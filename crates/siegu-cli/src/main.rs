use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use siegu_core::database::{Database, PhotoSyncInfo};
use siegu_core::mesh_transport::MeshTransport;
use siegu_core::scanner::ScanGuard;
use siegu_core::{PeerDevice, SavedSession, SyncEvent, SyncMessage, SyncProgress};

mod analyze_tui;
mod web;

/// Wire shape of one RPC reply: (request id, ok, result, error).
pub type RpcReply = (u64, bool, Option<serde_json::Value>, Option<String>);
pub type SharedRpcSlot = Arc<tokio::sync::Mutex<Option<RpcReply>>>;
pub type SharedSyncTx =
    Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<SyncMessage>>>>;

pub struct CliSyncEvent {
    pub config_path: String,
    pub sync_tx: SharedSyncTx,
    /// Signalled when the WebRTC data channel is ready (#19 e2e driver).
    pub ready: Arc<tokio::sync::Notify>,
    /// Last completed view-only manifest (#9 e2e driver).
    pub view_manifest: Arc<tokio::sync::Mutex<Vec<PhotoSyncInfo>>>,
    /// Signalled when `view_manifest` is complete.
    pub view_notify: Arc<tokio::sync::Notify>,
    /// Latest CommandResponse from the peer (#19 e2e driver).
    pub rpc_slot: SharedRpcSlot,
    /// Signalled whenever `rpc_slot` is updated.
    pub rpc_notify: Arc<tokio::sync::Notify>,
}

impl CliSyncEvent {
    pub fn new(config_path: &str) -> (Self, SharedSyncTx) {
        let sync_tx = Arc::new(tokio::sync::Mutex::new(None));
        let event = Self {
            config_path: config_path.to_string(),
            sync_tx: Arc::clone(&sync_tx),
            ready: Arc::new(tokio::sync::Notify::new()),
            view_manifest: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            view_notify: Arc::new(tokio::sync::Notify::new()),
            rpc_slot: Arc::new(tokio::sync::Mutex::new(None)),
            rpc_notify: Arc::new(tokio::sync::Notify::new()),
        };
        (event, sync_tx)
    }
}

impl SyncEvent for CliSyncEvent {
    fn on_state_change(&self, state: &str) {
        println!("[sync] {state}");
        // Initiators print "Secure Data Channel Ready"; receivers only get
        // the plain "Connected" peer-state line (exact match - never the
        // earlier "Connected to signaling...").
        if state == "Secure Data Channel Ready" || state == "Connected" {
            // notify_one parks a permit until a waiter shows up, so the
            // driver never misses the signal even if it registers late.
            self.ready.notify_one();
        }
    }

    fn on_view_manifest(&self, photos: &[PhotoSyncInfo]) {
        if let Ok(mut slot) = self.view_manifest.try_lock() {
            *slot = photos.to_vec();
        }
        self.view_notify.notify_one();
    }

    fn on_command_response(
        &self,
        id: u64,
        ok: bool,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) {
        if let Ok(mut slot) = self.rpc_slot.try_lock() {
            *slot = Some((id, ok, result.cloned(), error.map(str::to_string)));
        }
        self.rpc_notify.notify_one();
    }

    fn on_log(&self, message: &str) {
        println!("[sync] {message}");
    }

    fn on_sync_progress(&self, progress: SyncProgress) {
        println!(
            "[sync] {}: {}/{} ({:.0}%) - {}",
            progress.device_id,
            progress.items_completed,
            progress.items_total,
            progress.progress * 100.0,
            progress.status,
        );
    }

    fn on_photo_received(&self, _photo_id: String, _path: String) {}

    fn on_sync_error(&self, error: String) {
        eprintln!("[sync] Error: {error}");
    }

    fn on_peer_connected(
        &self,
        device_id: String,
        device_name: String,
        peer_os: String,
        models_enabled: Vec<String>,
        protocol_version: u8,
    ) {
        let db = Database::new(&self.config_path);
        db.upsert_peer_device(&PeerDevice {
            device_id,
            name: device_name,
            ip: String::new(),
            port: 0,
            device_type: String::new(),
            os: peer_os,
            models_enabled,
            protocol_version,
            storage_used: 0,
            storage_capacity: 0,
            last_seen: String::new(),
            photo_count: 0,
            video_count: 0,
            remote_photo_count: 0,
            remote_video_count: 0,
        });
    }

    fn on_peer_disconnected(&self, peer_id: String) {
        let db = Database::new(&self.config_path);
        db.update_peer_device_seen(&peer_id);
    }

    fn on_device_registered(&self, _db: &Database) {}

    fn on_metadata_updated(
        &self,
        _photo_id: &str,
        _caption: Option<&str>,
        _aesthetics_score: Option<f64>,
    ) {
        let msg = siegu_core::SyncMessage::MetadataUpdate {
            photo_id: _photo_id.to_string(),
            caption: _caption.map(|c| c.to_string()),
            aesthetics_score: _aesthetics_score,
            indexed: 2,
            deleted_at: None,
        };
        self.on_log(&format!("Metadata updated for {_photo_id}"));
        if let Ok(g) = self.sync_tx.try_lock() {
            if let Some(tx) = g.as_ref() {
                let _ = tx.send(msg);
            }
        }
    }

    fn get_config_path(&self) -> String {
        self.config_path.clone()
    }

    fn get_sync_path(&self) -> Option<String> {
        let db = Database::new(&self.config_path);
        db.get_state().get("sync_path").cloned()
    }

    fn get_directories(&self) -> Vec<String> {
        let db = Database::new(&self.config_path);
        db.list_directories()
    }
}

#[derive(Parser)]
#[command(name = "siegu", version, about = "Privacy-first media management CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true)]
    config_dir: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a folder for media
    Scan {
        /// Folder path to scan (uses configured directories if omitted)
        folder: Option<String>,
        #[arg(short, long)]
        config: Option<String>,
    },
    /// ML analysis commands
    Analyze {
        #[command(subcommand)]
        action: AnalyzeAction,
    },
    /// Manage ML models
    Models {
        #[command(subcommand)]
        action: ModelsAction,
    },
    /// Show app status
    Status {
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Start LAN signaling server
    Serve {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Start remote sync
    Sync {
        /// Signaling server URL
        server: String,
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Mesh sync commands
    Mesh {
        #[command(subcommand)]
        action: MeshAction,
    },
    /// Share this library as a view-only gallery in a browser (#11)
    Web {
        /// Port for the static web client (signalling picks its own port)
        #[arg(short, long, default_value = "8787")]
        port: u16,
        #[arg(short, long)]
        config: Option<String>,
        /// Permission level for connected browsers (#19): ro (default) allows
        /// browsing only; rw also allows favorites/trash mutations.
        #[arg(long, default_value = "ro")]
        share_mode: String,
    },
}

#[derive(Subcommand)]
enum MeshAction {
    /// Create a manual album from the first N photos and print its ID (#16).
    /// Test helper for scripts/e2e-view-only.sh.
    SeedAlbum {
        /// Album name
        #[arg(long, default_value = "E2E Shared")]
        name: String,
        /// Put the first N photos of the library into the album
        #[arg(long, default_value = "1")]
        take_first: usize,
    },
    /// Start a LAN mesh host and wait for peers
    Host {
        #[arg(short, long, default_value = "0")]
        port: u16,
        /// Connect to an existing signaling server instead of starting a local one
        #[arg(long)]
        server: Option<String>,
        /// Room ID to use when connecting via --server (required with --server)
        #[arg(long)]
        room: Option<String>,
        /// Permission level for connected peers (#19): ro (default) allows
        /// browsing only; rw also allows favorites/trash mutations.
        #[arg(long, default_value = "ro")]
        share_mode: String,
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Join a mesh room via signaling server
    Join {
        /// Room ID
        room: String,
        /// Signaling server URL (defaults to ws://127.0.0.1:8080)
        #[arg(long)]
        server: Option<String>,
        /// This peer creates the WebRTC offer (needed when joining a --server host)
        #[arg(long)]
        initiator: bool,
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Browse a peer's library read-only (#9): enter view-only mode, verify
    /// manifest + thumbnail + restore pull, then stay alive. Prints greppable
    /// VIEWONLY markers for scripts/e2e-view-only.sh.
    Browse {
        /// Room ID
        room: String,
        /// Signaling server URL (defaults to ws://127.0.0.1:8080)
        #[arg(long)]
        server: Option<String>,
        /// This peer creates the WebRTC offer (needed when joining a --server host)
        #[arg(long)]
        initiator: bool,
        #[arg(short, long)]
        config: Option<String>,
        /// Enter album-share mode (#16): send EnterAlbumShare for this album
        /// instead of EnterViewOnly and verify the host enforces membership.
        #[arg(long)]
        album: Option<String>,
        /// Device name announced to the room (must be unique per guest).
        #[arg(long, default_value = "siegu-browser")]
        name: String,
    },
    /// Send a single CommandRequest to the peer and print its reply (#19).
    /// Used by scripts/e2e-view-only.sh to exercise the RPC surface.
    Rpc {
        /// Room ID
        room: String,
        /// Command name, e.g. list_files or toggle_favorite
        command: String,
        /// JSON payload for the command
        #[arg(default_value = "{}")]
        payload: String,
        /// Signaling server URL (defaults to ws://127.0.0.1:8080)
        #[arg(long)]
        server: Option<String>,
        /// This peer creates the WebRTC offer (needed when joining a --server host)
        #[arg(long)]
        initiator: bool,
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Show mesh/session status
    Status {
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Disconnect and clear saved session
    Disconnect {
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Show storage quota usage
    Quota {
        #[arg(short, long)]
        config: Option<String>,
    },
}

#[derive(Subcommand)]
enum AnalyzeAction {
    /// Run ML analysis on all unprocessed photos
    All {
        /// Non-interactive mode: print progress lines and an E2E summary
        #[arg(long)]
        headless: bool,
    },
    /// Run ML analysis on a single photo by ID
    Photo {
        id: String,
        /// Non-interactive mode: print progress lines and an E2E summary
        #[arg(long)]
        headless: bool,
    },
    /// Run a specific model on all photos
    Model {
        model_id: String,
        /// Non-interactive mode: print progress lines and an E2E summary
        #[arg(long)]
        headless: bool,
    },
}

#[derive(Subcommand)]
enum ModelsAction {
    /// List all models and their status
    List,
    /// Download missing models
    Download {
        /// Specific model names to download (all if omitted)
        #[arg(num_args = 1..)]
        names: Option<Vec<String>>,
    },
    /// Show disk usage of models
    Usage,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show all config
    Get,
    /// Get a specific config key
    GetKey { key: String },
    /// Set a config key-value pair
    Set { key: String, value: String },
    /// List all valid config keys
    Keys,
}

fn resolve_config_dir(cli_dir: &Option<String>, cmd_dir: &Option<String>) -> PathBuf {
    if let Some(d) = cmd_dir {
        return PathBuf::from(d);
    }
    if let Some(d) = cli_dir {
        return PathBuf::from(d);
    }
    siegu_core::config::default_config_dir()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Scan { folder, config } => {
            let config_dir = resolve_config_dir(&cli.config_dir, config);
            cmd_scan(&config_dir, folder.as_deref()).await;
        }
        Commands::Analyze { action } => {
            let config_dir = resolve_config_dir(&cli.config_dir, &None);
            match action {
                AnalyzeAction::All { headless } => {
                    if *headless {
                        analyze_tui::run_analyze_all_headless(&config_dir);
                    } else {
                        analyze_tui::run_analyze_all(&config_dir);
                    }
                }
                AnalyzeAction::Photo { id, headless } => {
                    if *headless {
                        analyze_tui::run_analyze_photo_headless(&config_dir, id);
                    } else {
                        analyze_tui::run_analyze_photo(&config_dir, id);
                    }
                }
                AnalyzeAction::Model { model_id, headless } => {
                    if *headless {
                        analyze_tui::run_analyze_model_headless(&config_dir, model_id);
                    } else {
                        analyze_tui::run_analyze_model(&config_dir, model_id);
                    }
                }
            }
        }
        Commands::Models { action } => {
            let config_dir = resolve_config_dir(&cli.config_dir, &None);
            match action {
                ModelsAction::List => cmd_models_list(&config_dir),
                ModelsAction::Download { names } => {
                    cmd_models_download(&config_dir, names.as_deref()).await
                }
                ModelsAction::Usage => cmd_models_usage(&config_dir),
            }
        }
        Commands::Status { config } => {
            let config_dir = resolve_config_dir(&cli.config_dir, config);
            cmd_status(&config_dir);
        }
        Commands::Serve { port } => {
            cmd_serve(*port).await;
        }
        Commands::Sync { server, config } => {
            let _config_dir = resolve_config_dir(&cli.config_dir, config);
            cmd_sync(server).await;
        }
        Commands::Config { action } => {
            let config_dir = resolve_config_dir(&cli.config_dir, &None);
            match action {
                ConfigAction::Get => cmd_config_get(&config_dir),
                ConfigAction::GetKey { key } => cmd_config_get_key(&config_dir, key),
                ConfigAction::Set { key, value } => cmd_config_set(&config_dir, key, value),
                ConfigAction::Keys => cmd_config_keys(),
            }
        }
        Commands::Mesh { action } => match action {
            MeshAction::SeedAlbum { name, take_first } => {
                cmd_seed_album(
                    &resolve_config_dir(&cli.config_dir, &None),
                    name,
                    *take_first,
                );
            }
            MeshAction::Host {
                port,
                server,
                room,
                share_mode,
                config,
            } => {
                let config_dir = resolve_config_dir(&cli.config_dir, config);
                cmd_mesh_host(
                    *port,
                    server.as_deref(),
                    room.as_deref(),
                    share_mode,
                    &config_dir,
                )
                .await;
            }
            MeshAction::Join {
                room,
                server,
                initiator,
                config,
            } => {
                let config_dir = resolve_config_dir(&cli.config_dir, config);
                cmd_mesh_join(room, server.as_deref(), *initiator, &config_dir).await;
            }
            MeshAction::Browse {
                room,
                server,
                initiator,
                config,
                album,
                name,
            } => {
                let config_dir = resolve_config_dir(&cli.config_dir, config);
                cmd_mesh_browse(
                    room,
                    server.as_deref(),
                    *initiator,
                    &config_dir,
                    album.clone(),
                    name,
                )
                .await;
            }
            MeshAction::Rpc {
                room,
                command,
                payload,
                server,
                initiator,
                config,
            } => {
                let config_dir = resolve_config_dir(&cli.config_dir, config);
                cmd_mesh_rpc(
                    room,
                    command,
                    payload,
                    server.as_deref(),
                    *initiator,
                    &config_dir,
                )
                .await;
            }
            MeshAction::Status { config } => {
                let config_dir = resolve_config_dir(&cli.config_dir, config);
                cmd_mesh_status(&config_dir);
            }
            MeshAction::Disconnect { config } => {
                let config_dir = resolve_config_dir(&cli.config_dir, config);
                cmd_mesh_disconnect(&config_dir);
            }
            MeshAction::Quota { config } => {
                let config_dir = resolve_config_dir(&cli.config_dir, config);
                cmd_mesh_quota(&config_dir);
            }
        },
        Commands::Web {
            port,
            config,
            share_mode,
        } => {
            let mode = siegu_core::rpc::ShareMode::parse(share_mode).unwrap_or_else(|| {
                eprintln!(
                    "invalid --share-mode '{share_mode}' (expected ro or rw), falling back to ro"
                );
                siegu_core::ShareMode::ReadOnly
            });
            let config_dir = resolve_config_dir(&cli.config_dir, config);
            if let Err(e) = web::run(web::WebOptions {
                http_port: *port,
                config: Some(config_dir.display().to_string()),
                share_mode: mode,
            })
            .await
            {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

async fn cmd_scan(config_dir: &Path, folder: Option<&str>) {
    let _ = std::fs::create_dir_all(config_dir);
    let mut db = Database::new(&config_dir.display().to_string());

    let folders: Vec<String> = if let Some(f) = folder {
        let folder_path = Path::new(f);
        if !folder_path.exists() {
            eprintln!("Error: folder does not exist: {f}");
            std::process::exit(1);
        }
        let canonical = folder_path
            .canonicalize()
            .unwrap_or_else(|_| folder_path.to_path_buf())
            .display()
            .to_string();
        let dirs = db.list_directories();
        if !dirs.contains(&canonical) {
            db.add_directory(&canonical);
            println!("Added {canonical} to watched directories");
        }
        vec![canonical]
    } else {
        let dirs = db.list_directories();
        if dirs.is_empty() {
            eprintln!("No directories configured. Run `siegu scan <folder>` to add one.");
            std::process::exit(1);
        }
        dirs
    };

    let guard = ScanGuard::new();
    let _session = match guard.try_start() {
        Some(s) => s,
        None => {
            eprintln!("Error: scan already in progress");
            std::process::exit(1);
        }
    };

    let existing = siegu_core::scanner::load_existing_paths(&config_dir.display().to_string());
    println!("Loaded {} existing paths from DB", existing.len());

    use rayon::prelude::*;

    let pb = ProgressBar::new_spinner();
    let style = match ProgressStyle::with_template("{spinner:.green} {msg}") {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("failed to build spinner progress style, falling back to default: {e}");
            ProgressStyle::default_spinner()
        }
    };
    pb.set_style(style);

    let mut total_new_all = 0usize;
    let mut total_scanned_all = 0usize;

    for dir in &folders {
        let folder_path = Path::new(dir);
        println!("Scanning: {dir}");

        pb.set_message(format!("Scanning {dir}..."));

        let entries: Vec<_> = jwalk::WalkDir::new(folder_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| siegu_core::scanner::is_media_file(&e.path()))
            .collect();

        let total_scanned = entries.len();
        total_scanned_all += total_scanned;

        let new_photos: Vec<_> = entries
            .par_iter()
            .filter_map(|entry| {
                let file_path = entry.path();
                let path_str = file_path.display().to_string();
                if existing.contains(&path_str) {
                    return None;
                }
                let meta = siegu_core::scanner::extract_photo_metadata(&file_path);
                Some(siegu_core::scanner::photo_from_metadata(&path_str, &meta))
            })
            .collect();

        let total_new = new_photos.len();
        total_new_all += total_new;

        if total_new > 0 {
            pb.set_message(format!("Writing {total_new} new photos from {dir}..."));
            for batch in new_photos.chunks(500) {
                let _ = db.store_photo_batch(batch);
            }
        }
    }

    pb.finish_with_message("Done");
    println!(
        "\nScan complete: {total_new_all} new photos found ({total_scanned_all} total scanned)"
    );
}

fn cmd_models_list(config_dir: &Path) {
    let models_dir = config_dir.join("models");

    println!("Models directory: {}", models_dir.display());
    println!();

    for entry in siegu_core::model_manager::MODEL_REGISTRY {
        let path = models_dir.join(entry.filename);
        let exists = path.exists();
        let size = if exists {
            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let status = if exists {
            format!("OK ({:.1} MB)", size as f64 / 1_048_576.0)
        } else {
            "MISSING".to_string()
        };

        println!(
            "  {:20} {:>16}  {}",
            entry.model_name, status, entry.filename
        );
    }

    let statuses = siegu_core::model_manager::all_model_status(&models_dir);
    let missing: Vec<_> = statuses.iter().filter(|s| !s.downloaded).collect();
    if !missing.is_empty() {
        println!(
            "\n{} model(s) missing. Run: siegu models download",
            missing.len()
        );
    } else {
        println!("\nAll models downloaded.");
    }
}

async fn cmd_models_download(config_dir: &Path, names: Option<&[String]>) {
    let models_dir = config_dir.join("models");
    let _ = std::fs::create_dir_all(&models_dir);

    let to_download: Vec<_> = if let Some(names) = names {
        siegu_core::model_manager::MODEL_REGISTRY
            .iter()
            .filter(|e| {
                names.iter().any(|n| {
                    let normalized = match n.as_str() {
                        "ultraface" | "arcface" => "face",
                        other => other,
                    };
                    normalized == e.model_name
                })
            })
            .collect()
    } else {
        siegu_core::model_manager::MODEL_REGISTRY.iter().collect()
    };

    let model_names: Vec<String> = to_download
        .iter()
        .map(|e| e.model_name.to_string())
        .collect();
    let needed = siegu_core::model_manager::needed_download_bytes(&models_dir, &model_names);
    if needed > 0 {
        let free = siegu_core::model_manager::available_disk_bytes(&models_dir);
        if free > 0 && free < needed {
            eprintln!(
                "Error: not enough disk space: {} MB needed, only {} MB free",
                needed / (1024 * 1024),
                free / (1024 * 1024)
            );
            return;
        }
    }

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36")
        .timeout(std::time::Duration::from_secs(600))
        .connect_timeout(std::time::Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .unwrap_or_else(|e| {
            eprintln!("Error: failed to create HTTP client: {e}");
            std::process::exit(1);
        });

    for entry in &to_download {
        let path = models_dir.join(entry.filename);
        if path.exists() {
            println!("{}: already downloaded, skipping", entry.model_name);
            continue;
        }

        println!("Downloading: {} from {}", entry.model_name, entry.url);

        let total_size = entry.expected_size;
        let pb = ProgressBar::new(total_size);
        let style = match ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    "failed to build download progress style, falling back to default: {e}"
                );
                ProgressStyle::default_bar()
            }
        };
        pb.set_style(style);
        pb.set_message(entry.model_name.to_string());

        let result = siegu_core::model_manager::download_file(
            &client,
            entry.url,
            entry.filename,
            entry.expected_size,
            &models_dir,
            |downloaded, total| {
                if let Some(total) = total {
                    pb.set_length(total);
                }
                pb.set_position(downloaded);
            },
        )
        .await;

        match result {
            Ok(siegu_core::model_manager::DownloadOutcome::Skipped) => {
                println!("{}: already downloaded, skipping", entry.model_name);
            }
            Ok(siegu_core::model_manager::DownloadOutcome::Completed) => {
                pb.finish_with_message(format!("{}: done", entry.model_name));
            }
            Err(e) => {
                eprintln!("  ERROR: {e}");
                pb.abandon();
            }
        }

        if path.exists() {
            match siegu_core::model_manager::verify_sha256(&path, entry.sha256) {
                Ok(true) => println!("{}: SHA-256 verified", entry.model_name),
                Ok(false) => {
                    eprintln!(
                        "  WARNING: SHA-256 mismatch for {}, deleting",
                        entry.filename
                    );
                    let _ = tokio::fs::remove_file(&path).await;
                }
                Err(e) => eprintln!("  WARNING: could not verify hash: {e}"),
            }
        }
    }

    println!("\nDone.");
}

fn cmd_models_usage(config_dir: &Path) {
    let models_dir = config_dir.join("models");
    let model_names: Vec<String> = siegu_core::model_manager::MODEL_REGISTRY
        .iter()
        .map(|e| e.model_name.to_string())
        .collect();
    let usage = siegu_core::model_manager::model_sizes_on_disk(&models_dir, &model_names);
    let total: u64 = usage.iter().map(|(_, s)| s).sum();

    println!("Model disk usage:");
    for (name, size) in &usage {
        println!("  {:20} {:>8.1} MB", name, *size as f64 / 1_048_576.0);
    }
    println!("  {:20} {:>8.1} MB", "TOTAL", total as f64 / 1_048_576.0);
}

fn cmd_status(config_dir: &Path) {
    let db_path = config_dir.join("siegu.db");
    println!("Config directory: {}", config_dir.display());
    println!(
        "Database: {}",
        if db_path.exists() {
            "found"
        } else {
            "not found"
        }
    );

    if db_path.exists() {
        let db = Database::new(&config_dir.display().to_string());
        let (photo_count, video_count) = db.get_media_counts();
        let folders = db.list_directories();
        let config = db.get_state();

        println!("\nMedia:");
        println!("  Photos: {photo_count}");
        println!("  Videos: {video_count}");
        println!("  Folders: {}", folders.len());
        for f in &folders {
            println!("    - {f}");
        }

        println!("\nConfiguration:");
        let mut keys: Vec<_> = config.keys().collect();
        keys.sort();
        for key in keys {
            println!("  {key}: {}", config[key]);
        }
    }

    let models_dir = config_dir.join("models");
    if models_dir.exists() {
        let total = siegu_core::model_manager::total_model_disk_usage(&models_dir);
        println!("\nModels: {:.1} MB on disk", total as f64 / 1_048_576.0);
    } else {
        println!("\nModels: not downloaded");
    }

    println!(
        "\nMemory: {:.1} MB available",
        siegu_core::model_manager::available_memory_bytes() as f64 / 1_048_576.0
    );
}

async fn cmd_serve(port: u16) {
    println!("Starting LAN signaling server on port {port}...");
    println!("Press Ctrl+C to stop.");

    let _ = siegu_core::lan_server::start(port).await;
    let _ = tokio::signal::ctrl_c().await;
    println!("Shutting down...");
}

async fn cmd_sync(server: &str) {
    println!("Remote sync requires a config directory and session setup.");
    println!("Use `siegu mesh join <room>` for LAN sync, or pass a signaling URL.");
    println!("Server URL: {server}");
}

fn cmd_config_get(config_dir: &Path) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        println!("{{}}");
        return;
    }
    let db = Database::new(&config_dir.display().to_string());
    let config = db.get_state();
    println!(
        "{}",
        serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string())
    );
}

fn cmd_config_get_key(config_dir: &Path, key: &str) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("No database found");
        return;
    }
    let db = Database::new(&config_dir.display().to_string());
    let config = db.get_state();
    match config.get(key) {
        Some(v) => println!("{v}"),
        None => eprintln!("Key '{key}' not set"),
    }
}

fn cmd_config_set(config_dir: &Path, key: &str, value: &str) {
    if let Err(e) = siegu_core::config::validate_config_value(key, value) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    let _ = std::fs::create_dir_all(config_dir);
    let db = Database::new(&config_dir.display().to_string());
    let mut state = std::collections::HashMap::new();
    state.insert(key.to_string(), value.to_string());
    db.set_state(state);
    println!("Set {key} = {value}");
}

fn cmd_config_keys() {
    println!("Valid config keys:");
    for key in siegu_core::config::ALLOWED_CONFIG_KEYS {
        println!("  {key}");
    }
}

async fn cmd_mesh_host(
    port: u16,
    server: Option<&str>,
    room: Option<&str>,
    share_mode: &str,
    config_dir: &Path,
) {
    let share_mode = match siegu_core::ShareMode::parse(share_mode) {
        Some(m) => m,
        None => {
            eprintln!("warning: unknown --share-mode '{share_mode}', defaulting to read-only");
            siegu_core::ShareMode::ReadOnly
        }
    };
    let config_path = config_dir.display().to_string();
    let _ = std::fs::create_dir_all(config_dir);
    let db = Database::new(&config_path);

    let room_id = room
        .map(|r| r.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-host".to_string());

    let (signaling_url, actual_port, daemon) = if let Some(server_url) = server {
        let url = server_url.trim_end_matches('/').to_string();
        println!("Connecting to signaling server: {url}");
        println!("Room ID: {room_id}");
        (url, 0, None)
    } else {
        println!("Starting LAN mesh host...");
        println!("Room ID: {room_id}");

        let server = match MeshTransport::start_lan_server(port).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error: failed to start LAN signaling server: {e}");
                std::process::exit(1);
            }
        };
        let actual_port = server.port;
        let signaling_url = format!("ws://127.0.0.1:{actual_port}");
        println!("Signaling server on port {actual_port}");

        let daemon = match siegu_core::mdns::create_daemon() {
            Ok(d) => {
                if let Err(e) = siegu_core::mdns::register_service(&d, &hostname, actual_port) {
                    eprintln!("mDNS registration failed: {e}");
                } else {
                    println!("mDNS registered as {hostname}");
                }
                Some(d)
            }
            Err(e) => {
                eprintln!("mDNS init failed: {e}");
                None
            }
        };

        (signaling_url, actual_port, daemon)
    };

    db.save_session(&SavedSession {
        room_id: room_id.clone(),
        signaling_url: signaling_url.clone(),
        port: actual_port,
        is_initiator: true,
        passphrase: String::new(),
    });

    let sync_tx = Arc::new(tokio::sync::Mutex::new(None));
    let event = Arc::new(CliSyncEvent {
        config_path: config_path.clone(),
        sync_tx: Arc::clone(&sync_tx),
        ready: Arc::new(tokio::sync::Notify::new()),
        view_manifest: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        view_notify: Arc::new(tokio::sync::Notify::new()),
        rpc_slot: Arc::new(tokio::sync::Mutex::new(None)),
        rpc_notify: Arc::new(tokio::sync::Notify::new()),
    });

    let transport = MeshTransport::new(
        room_id.clone(),
        true,
        signaling_url.clone(),
        config_path.clone(),
        uuid::Uuid::new_v4().to_string(),
        hostname.clone(),
        Vec::new(),
        event,
    )
    .with_share_mode(share_mode);

    println!("Waiting for peers... Press Ctrl+C to stop.");

    let transport_handle = tokio::spawn(async move {
        if let Err(e) = transport.start().await {
            eprintln!("WebRTC transport stopped: {e}");
        }
    });

    let _ = tokio::signal::ctrl_c().await;
    println!("Shutting down...");

    if let Some(ref d) = daemon {
        siegu_core::mdns::unregister_service(d, &hostname);
    }
    transport_handle.abort();
    println!("Shutting down...");
    transport_handle.abort();
}

async fn cmd_mesh_join(room: &str, server: Option<&str>, initiator: bool, config_dir: &Path) {
    let config_path = config_dir.display().to_string();
    let _ = std::fs::create_dir_all(config_dir);
    let db = Database::new(&config_path);

    let device_name = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-device".to_string());

    let signaling_url = server
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_else(|| "ws://127.0.0.1:8080".to_string());

    println!("Joining mesh room: {room}");
    println!("Signaling: {signaling_url}");

    let sync_tx2 = Arc::new(tokio::sync::Mutex::new(None));
    let event = Arc::new(CliSyncEvent {
        config_path: config_path.clone(),
        sync_tx: Arc::clone(&sync_tx2),
        ready: Arc::new(tokio::sync::Notify::new()),
        view_manifest: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        view_notify: Arc::new(tokio::sync::Notify::new()),
        rpc_slot: Arc::new(tokio::sync::Mutex::new(None)),
        rpc_notify: Arc::new(tokio::sync::Notify::new()),
    });

    let transport = MeshTransport::new(
        room.to_string(),
        initiator,
        signaling_url.clone(),
        config_path.clone(),
        uuid::Uuid::new_v4().to_string(),
        device_name,
        Vec::new(),
        event,
    );

    db.save_session(&SavedSession {
        room_id: room.to_string(),
        signaling_url,
        port: 0,
        is_initiator: initiator,
        passphrase: String::new(),
    });

    println!("Connecting... Press Ctrl+C to stop.");

    let transport_handle = tokio::spawn(async move {
        if let Err(e) = transport.start().await {
            eprintln!("WebRTC transport stopped: {e}");
        }
    });

    let _ = tokio::signal::ctrl_c().await;
    println!("Shutting down...");
    transport_handle.abort();
}

/// Handles for the one-shot mesh drivers (`mesh browse`, `mesh rpc`):
/// everything needed to observe peer traffic from outside the transport.
struct MeshDriver {
    sync_tx: SharedSyncTx,
    ready: Arc<tokio::sync::Notify>,
    view_manifest: Arc<tokio::sync::Mutex<Vec<PhotoSyncInfo>>>,
    view_notify: Arc<tokio::sync::Notify>,
    rpc_slot: SharedRpcSlot,
    rpc_notify: Arc<tokio::sync::Notify>,
}

impl MeshDriver {
    /// Build the event sink, start the transport and wait until the WebRTC
    /// channel reports "Secure Data Channel Ready".
    async fn connect(
        room: &str,
        server: Option<&str>,
        initiator: bool,
        config_dir: &Path,
        device_name: &str,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let config_path = config_dir.display().to_string();
        let signaling_url = server
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "ws://127.0.0.1:8080".to_string());

        let (event, sync_tx) = CliSyncEvent::new(&config_path);
        let driver = Self {
            ready: Arc::clone(&event.ready),
            view_manifest: Arc::clone(&event.view_manifest),
            view_notify: Arc::clone(&event.view_notify),
            rpc_slot: Arc::clone(&event.rpc_slot),
            rpc_notify: Arc::clone(&event.rpc_notify),
            sync_tx: Arc::clone(&sync_tx),
        };

        let transport = MeshTransport::new(
            room.to_string(),
            initiator,
            signaling_url.clone(),
            config_path.clone(),
            uuid::Uuid::new_v4().to_string(),
            device_name.to_string(),
            Vec::new(),
            Arc::new(event),
        )
        .with_view_only_client(true)
        .with_external_tx(Arc::clone(&sync_tx));

        println!("Signaling: {signaling_url}");
        let handle = tokio::spawn(async move {
            if let Err(e) = transport.start().await {
                eprintln!("WebRTC transport stopped: {e}");
            }
        });

        if tokio::time::timeout(std::time::Duration::from_secs(90), driver.ready.notified())
            .await
            .is_err()
        {
            eprintln!("FAIL: data channel not ready within 90s");
            handle.abort();
            std::process::exit(1);
        }
        // Give VersionNegotiate/ManifestRequest a beat to settle on both sides.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        (driver, handle)
    }

    async fn sender(&self) -> tokio::sync::mpsc::UnboundedSender<SyncMessage> {
        match self.sync_tx.lock().await.as_ref() {
            Some(tx) => tx.clone(),
            None => {
                eprintln!("FAIL: session sender not bound");
                std::process::exit(1);
            }
        }
    }
}

/// View-only e2e driver (#9/#10/#19): connect to a host, enter view-only
/// mode and exercise the whole read-only surface — manifest, thumbnail
/// fetch, sync-guard probe and a restore pull. Each stage prints a greppable
/// `VIEWONLY ...` marker for scripts/e2e-view-only.sh; any stage failure
/// exits non-zero so CI only needs the markers plus exit code.
async fn cmd_mesh_browse(
    room: &str,
    server: Option<&str>,
    initiator: bool,
    config_dir: &Path,
    album_id: Option<String>,
    device_name: &str,
) {
    use std::time::Duration;

    let _ = std::fs::create_dir_all(config_dir);
    // Ensure schema exists so restore persistence has somewhere to land.
    let _schema = Database::new(&config_dir.display().to_string());

    println!("Browsing mesh room: {room}");
    let (driver, handle) =
        MeshDriver::connect(room, server, initiator, config_dir, device_name).await;
    let tx = driver.sender().await;

    // ── stage 1: EnterViewOnly / EnterAlbumShare → chunked manifest ───────
    let entered = match &album_id {
        Some(id) => tx.send(SyncMessage::EnterAlbumShare {
            album_id: id.clone(),
        }),
        None => tx.send(SyncMessage::EnterViewOnly),
    };
    if entered.is_err() {
        eprintln!("FAIL: could not send view-only/album-share entry");
        std::process::exit(1);
    }
    if tokio::time::timeout(Duration::from_secs(60), driver.view_notify.notified())
        .await
        .is_err()
    {
        eprintln!("FAIL: view-only manifest did not arrive within 60s");
        std::process::exit(1);
    }
    let photos = driver.view_manifest.lock().await.clone();
    if photos.is_empty() {
        eprintln!("FAIL: view-only manifest is empty (host library not scanned?)");
        std::process::exit(1);
    }
    println!("VIEWONLY MANIFEST OK count={}", photos.len());
    if album_id.is_some() {
        println!("VIEWONLY ALBUM SCOPE OK count={}", photos.len());
    }
    let first_id = photos[0].id.clone();
    let first_name = std::path::Path::new(&photos[0].location)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // ── stage 2: thumbnail round-trip via the view-only cache ─────────────
    let view = siegu_core::view_only::state();
    if !view.request_media(&first_id, true) {
        eprintln!("FAIL: could not request thumbnail");
        std::process::exit(1);
    }
    match view
        .wait_for(&format!("thumb:{first_id}"), Duration::from_secs(30))
        .await
    {
        Some(media) => println!(
            "VIEWONLY THUMB OK bytes={} mime={}",
            media.bytes.len(),
            media.mime
        ),
        None => {
            eprintln!("FAIL: thumbnail for {first_id} did not arrive within 30s");
            std::process::exit(1);
        }
    }

    // ── stage 3: sync-guard probe — the sharer must IGNORE StartSync ──────
    let _ = tx.send(SyncMessage::StartSync);
    println!("VIEWONLY SYNC-GUARD PROBE SENT");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // ── stage 3b (album share): membership enforcement (#16) ──────────────
    // A photo outside the shared album must NOT be served; the host drops
    // the FetchMediaRequest, so no media ever arrives for the bogus id.
    if album_id.is_some() {
        let bogus = "siegu-e2e-nonmember".to_string();
        let _ = tx.send(SyncMessage::FetchMediaRequest {
            id: bogus.clone(),
            thumbnail: true,
            restore: false,
        });
        println!("VIEWONLY ALBUM DENY PROBE SENT id={bogus}");
        match view
            .wait_for(&format!("thumb:{bogus}"), Duration::from_secs(5))
            .await
        {
            Some(_) => {
                eprintln!("FAIL: host served media for a photo outside the shared album");
                std::process::exit(1);
            }
            None => println!("VIEWONLY ALBUM DENY OK id={bogus}"),
        }
    }

    // ── stage 4: restore pull (#10) re-materializes the original locally ──
    if tx
        .send(SyncMessage::FetchMediaRequest {
            id: first_id.clone(),
            thumbnail: false,
            restore: true,
        })
        .is_err()
    {
        eprintln!("FAIL: could not send restore FetchMediaRequest");
        std::process::exit(1);
    }
    println!("VIEWONLY RESTORE REQUESTED id={first_id}");

    let expected = config_dir.join("Siegu").join("siegu").join(&first_name);
    let mut restored = false;
    for _ in 0..120 {
        if let Ok(meta) = std::fs::metadata(&expected) {
            if meta.len() > 0 {
                restored = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    if !restored {
        eprintln!(
            "FAIL: restored original not found at {}",
            expected.display()
        );
        std::process::exit(1);
    }
    println!("VIEWONLY RESTORE OK path={}", expected.display());
    println!("VIEWONLY DONE");

    handle.abort();
}

/// Test helper for scripts/e2e-view-only.sh (#16): build a manual album from
/// the first N photos in this library and print its ID on stdout.
fn cmd_seed_album(config_dir: &Path, name: &str, take_first: usize) {
    let db = Database::new(&config_dir.display().to_string());
    let ids: Vec<String> = db
        .list_photos("", 0, take_first.max(1), false, false)
        .iter()
        .map(|p| p.id.clone())
        .collect();
    if ids.is_empty() {
        eprintln!("FAIL: no photos in library to seed the album with");
        std::process::exit(1);
    }
    let album = match db.create_album(name) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("FAIL: could not create album: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = db.add_album_items(&album.id, &ids) {
        eprintln!("FAIL: could not fill album: {e}");
        std::process::exit(1);
    }
    println!("ALBUM ID {} photos={}", album.id, ids.len());
}

/// One-shot RPC driver (#19): connect, send a single CommandRequest and
/// print the peer's reply as `RPC RESULT ok=<bool> <json>` / `RPC ERROR ...`.
/// Exit code 0 on success, 3 when the command itself failed (ok=false).
async fn cmd_mesh_rpc(
    room: &str,
    command: &str,
    payload: &str,
    server: Option<&str>,
    initiator: bool,
    config_dir: &Path,
) {
    use std::time::Duration;

    let _ = std::fs::create_dir_all(config_dir);
    let payload_value: serde_json::Value = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FAIL: payload is not valid JSON: {e}");
            std::process::exit(1);
        }
    };

    println!("RPC mesh room: {room} command: {command}");
    let (driver, handle) =
        MeshDriver::connect(room, server, initiator, config_dir, "siegu-rpc-client").await;
    let tx = driver.sender().await;

    if tx
        .send(SyncMessage::CommandRequest {
            id: 1,
            name: command.to_string(),
            payload: payload_value,
        })
        .is_err()
    {
        eprintln!("FAIL: could not send CommandRequest");
        std::process::exit(1);
    }

    if tokio::time::timeout(Duration::from_secs(45), driver.rpc_notify.notified())
        .await
        .is_err()
    {
        eprintln!("FAIL: no CommandResponse within 45s");
        std::process::exit(1);
    }
    let (_id, ok, result, error) = match driver.rpc_slot.lock().await.take() {
        Some(entry) => entry,
        None => {
            eprintln!("FAIL: response slot empty");
            std::process::exit(1);
        }
    };
    handle.abort();

    if ok {
        println!(
            "RPC RESULT ok=true result={}",
            result
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".into())
        );
    } else {
        println!(
            "RPC ERROR ok=false error={}",
            error.unwrap_or_else(|| "unknown".into())
        );
        std::process::exit(3);
    }
}

fn cmd_mesh_status(config_dir: &Path) {
    let config_path = config_dir.display().to_string();
    let db = Database::new(&config_path);
    match db.load_session() {
        Some(session) => {
            println!("Saved session:");
            println!("  Room:         {}", session.room_id);
            println!("  Signaling:    {}", session.signaling_url);
            println!("  Port:         {}", session.port);
            println!("  Initiator:    {}", session.is_initiator);
        }
        None => {
            println!("No saved session.");
        }
    }
    let peers = db.list_peer_devices();
    if !peers.is_empty() {
        println!("\nKnown peer devices:");
        for p in &peers {
            println!(
                "  {} ({}) - last seen: {}",
                p.name, p.device_id, p.last_seen
            );
        }
    }
    let state = db.get_state();
    if let Some(max_mb) = state.get("max_storage_mb") {
        println!("  Storage quota: {max_mb} MB");
    } else {
        println!("  Storage quota: 10240 MB (default)");
    }
}

fn cmd_mesh_disconnect(config_dir: &Path) {
    let config_path = config_dir.display().to_string();
    let db = Database::new(&config_path);
    db.clear_session();
    println!("Session cleared.");
}

fn cmd_mesh_quota(config_dir: &Path) {
    let config_path = config_dir.display().to_string();
    let used = siegu_core::MeshManager::get_storage_used(&config_path);
    let quota = siegu_core::MeshManager::get_storage_quota(&config_path);
    let pct = if quota > 0 {
        (used as f64 / quota as f64) * 100.0
    } else {
        0.0
    };
    println!("Storage usage:");
    println!(
        "  Used:  {} bytes ({:.2} MB)",
        used,
        used as f64 / 1_048_576.0
    );
    println!(
        "  Quota: {} bytes ({:.2} MB)",
        quota,
        quota as f64 / 1_048_576.0
    );
    println!("  Usage: {:.1}%", pct);

    if pct > 90.0 {
        println!("  WARNING: Storage nearly full!");
    }
}
