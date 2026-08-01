use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use siegu_core::database::Database;
use siegu_core::mesh_transport::MeshTransport;
use siegu_core::scanner::ScanGuard;
use siegu_core::{MeshManager, PeerDevice, SavedSession, SyncEvent, SyncProgress};

mod analyze_tui;

pub struct CliSyncEvent {
    pub config_path: String,
    pub sync_tx: Arc<
        tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<siegu_core::SyncMessage>>>,
    >,
}

impl SyncEvent for CliSyncEvent {
    fn on_state_change(&self, state: &str) {
        println!("[sync] {state}");
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
}

#[derive(Subcommand)]
enum MeshAction {
    /// Start a LAN mesh host and wait for peers
    Host {
        #[arg(short, long, default_value = "0")]
        port: u16,
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Join a mesh room via signaling server
    Join {
        /// Room ID or signaling URL
        room: String,
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
    All,
    /// Run ML analysis on a single photo by ID
    Photo { id: String },
    /// Run a specific model on all photos
    Model { model_id: String },
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
                AnalyzeAction::All => cmd_analyze_all(&config_dir),
                AnalyzeAction::Photo { id } => cmd_analyze_photo(&config_dir, id),
                AnalyzeAction::Model { model_id } => cmd_analyze_model(&config_dir, model_id),
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
        Commands::Mesh { action } => {
            let config_dir = resolve_config_dir(&cli.config_dir, &None);
            match action {
                MeshAction::Host { port, config } => {
                    let config_dir = resolve_config_dir(&cli.config_dir, config);
                    cmd_mesh_host(*port, &config_dir).await;
                }
                MeshAction::Join { room, config } => {
                    let config_dir = resolve_config_dir(&cli.config_dir, config);
                    cmd_mesh_join(room, &config_dir).await;
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
    pb.set_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());

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

fn cmd_analyze_all(config_dir: &Path) {
    analyze_tui::run_analyze_all(config_dir);
}

fn cmd_analyze_photo(config_dir: &Path, id: &str) {
    analyze_tui::run_analyze_photo(config_dir, id);
}

fn cmd_analyze_model(config_dir: &Path, model_id: &str) {
    analyze_tui::run_analyze_model(config_dir, model_id);
}

fn cmd_models_list(config_dir: &Path) {
    let models_dir = config_dir.join("models");

    println!("Models directory: {}", models_dir.display());
    println!();

    for entry in siegu_core::model_manager::MODEL_REGISTRY {
        let path = models_dir.join(&entry.filename);
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
            .filter(|e| names.iter().any(|n| n == &e.model_name))
            .collect()
    } else {
        siegu_core::model_manager::MODEL_REGISTRY.iter().collect()
    };

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36")
        .timeout(std::time::Duration::from_secs(600))
        .connect_timeout(std::time::Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .expect("Failed to create HTTP client");

    for entry in &to_download {
        let path = models_dir.join(&entry.filename);
        if path.exists() {
            println!("{}: already downloaded, skipping", entry.model_name);
            continue;
        }

        println!("Downloading: {} from {}", entry.model_name, entry.url);

        let response = match client.get(entry.url).send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  ERROR: request failed: {e}");
                continue;
            }
        };

        if !response.status().is_success() {
            eprintln!("  ERROR: status {}", response.status());
            continue;
        }

        let total_size = response.content_length();
        let pb = ProgressBar::new(total_size.unwrap_or(0));
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .unwrap(),
        );
        pb.set_message(entry.model_name.to_string());

        let tmp_path = path.with_extension("tmp");
        let mut file = tokio::fs::File::create(&tmp_path).await.unwrap();
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("  ERROR: stream error: {e}");
                    break;
                }
            };
            tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                .await
                .unwrap();
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        drop(file);
        tokio::fs::rename(&tmp_path, &path).await.unwrap();
        pb.finish_with_message(format!("{}: done", entry.model_name));

        if !entry.sha256.is_empty() {
            match siegu_core::model_manager::verify_sha256(&path, &entry.sha256) {
                Ok(true) => println!("  SHA-256 verified"),
                Ok(false) => {
                    eprintln!("  WARNING: SHA-256 mismatch! File may be corrupted.");
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

    match siegu_core::lan_server::start(port).await {
        _ => {}
    }
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

async fn cmd_mesh_host(port: u16, config_dir: &Path) {
    let config_path = config_dir.display().to_string();
    let _ = std::fs::create_dir_all(config_dir);
    let db = Database::new(&config_path);

    let room_id = uuid::Uuid::new_v4().to_string();
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-host".to_string());

    println!("Starting LAN mesh host...");
    println!("Room ID: {room_id}");

    let server = MeshTransport::start_lan_server(port)
        .await
        .expect("Failed to start LAN signaling server");
    let actual_port = server.port;
    println!("Signaling server on port {actual_port}");

    let sync_tx = Arc::new(tokio::sync::Mutex::new(None));
    let event = Arc::new(CliSyncEvent {
        config_path: config_path.clone(),
        sync_tx: Arc::clone(&sync_tx),
    });

    let transport = MeshTransport::new(
        room_id.clone(),
        true,
        format!("ws://127.0.0.1:{actual_port}"),
        config_path.clone(),
        uuid::Uuid::new_v4().to_string(),
        hostname.clone(),
        Vec::new(),
        event,
    );

    let daemon = match siegu_core::mdns::create_daemon() {
        Ok(d) => {
            if let Err(e) = siegu_core::mdns::register_service(&d, &hostname, actual_port, &room_id)
            {
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

    db.save_session(&SavedSession {
        room_id: room_id.clone(),
        signaling_url: format!("ws://127.0.0.1:{actual_port}"),
        port: actual_port,
        is_initiator: true,
        passphrase: String::new(),
    });

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

async fn cmd_mesh_join(room: &str, config_dir: &Path) {
    let config_path = config_dir.display().to_string();
    let _ = std::fs::create_dir_all(config_dir);
    let db = Database::new(&config_path);

    let device_name = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "siegu-device".to_string());

    let signaling_url = if room.starts_with("ws://") || room.starts_with("wss://") {
        room.to_string()
    } else {
        format!("ws://127.0.0.1:8080/{}", room)
    };

    println!("Joining mesh room: {room}");
    println!("Signaling: {signaling_url}");

    let sync_tx2 = Arc::new(tokio::sync::Mutex::new(None));
    let event = Arc::new(CliSyncEvent {
        config_path: config_path.clone(),
        sync_tx: Arc::clone(&sync_tx2),
    });

    let transport = MeshTransport::new(
        room.to_string(),
        false,
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
        is_initiator: false,
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
