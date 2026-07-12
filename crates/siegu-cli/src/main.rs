use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use siegu_core::database::Database;
use siegu_core::scanner::ScanGuard;

mod analyze_tui;

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
    println!("Remote sync is available through the Tauri GUI app.");
    println!("The CLI does not currently support WebRTC peer connections.");
    println!("Server URL: {server}");
    println!("Use `siegu serve` to start a LAN server instead.");
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
