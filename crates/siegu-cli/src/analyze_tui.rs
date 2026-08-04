use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tracing::error;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::Terminal;

use siegu_core::database::Database;
use siegu_core::ml_engine::worker::{start_worker, AnalysisCallbacks};
use siegu_core::ml_engine::PhotoResult;
use siegu_core::ml_worker::MlContext;

#[allow(clippy::large_enum_variant)]
enum TuiEvent {
    PhotoComplete {
        photo_id: String,
        location: String,
        result: PhotoResult,
        remaining: usize,
    },
    Progress {
        completed: usize,
        total: usize,
    },
    ModelStatus {
        model: String,
        status: String,
    },
    EpSelected(String),
    Log(String),
    ScanComplete,
}

struct TuiCallbacks {
    tx: mpsc::Sender<TuiEvent>,
}

impl AnalysisCallbacks for TuiCallbacks {
    fn on_photo_complete(
        &self,
        photo_id: &str,
        location: &str,
        result: &PhotoResult,
        remaining: usize,
        _progress_model: Option<&str>,
    ) {
        let _ = self.tx.send(TuiEvent::PhotoComplete {
            photo_id: photo_id.to_string(),
            location: location.to_string(),
            result: result.clone(),
            remaining,
        });
    }

    fn on_metadata_updated(
        &self,
        _photo_id: &str,
        _caption: Option<&str>,
        _aesthetics_score: Option<f64>,
    ) {
    }

    fn on_scan_complete(&self) {
        let _ = self.tx.send(TuiEvent::ScanComplete);
    }

    fn on_progress(&self, completed: usize, total: usize, _avg_ms: f64) {
        let _ = self.tx.send(TuiEvent::Progress { completed, total });
    }

    fn on_model_status(&self, model: &str, status: &str, _pending: usize, _total: usize) {
        let _ = self.tx.send(TuiEvent::ModelStatus {
            model: model.to_string(),
            status: status.to_string(),
        });
    }

    fn on_ep_selected(&self, ep: &str) {
        let _ = self.tx.send(TuiEvent::EpSelected(ep.to_string()));
    }

    fn on_log(&self, msg: &str) {
        let _ = self.tx.send(TuiEvent::Log(msg.to_string()));
    }

    fn should_abort(&self) -> bool {
        false
    }
}

struct PhotoLine {
    short_name: String,
    is_video: bool,
    timings: Vec<(String, f64)>,
    objects: Vec<(String, String)>,
    face_count: usize,
    nsfw: Option<String>,
    ocr: Option<String>,
    transcript: Option<String>,
    aesthetics: Option<f64>,
}

struct Findings {
    object_counts: std::collections::HashMap<String, usize>,
    total_faces: usize,
    nsfw_items: usize,
    ocr_texts: Vec<String>,
    transcripts: Vec<String>,
    aesthetics_sum: f64,
    aesthetics_count: usize,
    total_photos: usize,
    total_videos: usize,
}

impl Findings {
    fn new() -> Self {
        Self {
            object_counts: std::collections::HashMap::new(),
            total_faces: 0,
            nsfw_items: 0,
            ocr_texts: Vec::new(),
            transcripts: Vec::new(),
            aesthetics_sum: 0.0,
            aesthetics_count: 0,
            total_photos: 0,
            total_videos: 0,
        }
    }

    fn update(&mut self, line: &PhotoLine) {
        for (cls, _) in &line.objects {
            *self.object_counts.entry(cls.clone()).or_insert(0) += 1;
        }
        self.total_faces += line.face_count;
        if line.nsfw.is_some() {
            self.nsfw_items += 1;
        }
        if let Some(ref ocr) = line.ocr {
            if !ocr.trim().is_empty() && !self.ocr_texts.contains(ocr) {
                self.ocr_texts.push(ocr.clone());
            }
        }
        if let Some(ref t) = line.transcript {
            if !t.trim().is_empty() && !self.transcripts.contains(t) {
                self.transcripts.push(t.clone());
            }
        }
        if let Some(a) = line.aesthetics {
            self.aesthetics_sum += a;
            self.aesthetics_count += 1;
        }
    }
}

struct App {
    photo_lines: Vec<PhotoLine>,
    findings: Findings,
    completed: usize,
    total: usize,
    ep: String,
    status: String,
    active_model: Option<String>,
    should_quit: bool,
    done: bool,
    start_time: Instant,
    last_avg_ms: f64,
}

impl App {
    fn new(total: usize) -> Self {
        Self {
            photo_lines: Vec::new(),
            findings: Findings::new(),
            completed: 0,
            total,
            ep: "CPU".to_string(),
            status: "Starting...".to_string(),
            active_model: None,
            should_quit: false,
            done: false,
            start_time: Instant::now(),
            last_avg_ms: 1000.0,
        }
    }

    fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::PhotoComplete {
                location,
                result,
                remaining,
                ..
            } => {
                let short_name = std::path::Path::new(&location)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let is_video = siegu_core::ml_engine::pipeline::is_video_file(&location);

                let mut timings: Vec<(String, f64)> = result
                    .model_timings
                    .iter()
                    .map(|(k, v)| (k.clone(), v * 1000.0))
                    .collect();
                timings.sort_by(|a, b| a.0.cmp(&b.0));

                let objects = result.objects.iter().take(5).cloned().collect();

                if is_video {
                    self.findings.total_videos += 1;
                } else {
                    self.findings.total_photos += 1;
                }

                let line = PhotoLine {
                    short_name,
                    is_video,
                    timings,
                    objects,
                    face_count: result.face_count,
                    nsfw: result.nsfw.clone(),
                    ocr: result.ocr.clone(),
                    transcript: result.transcript.clone(),
                    aesthetics: result.aesthetics,
                };
                self.findings.update(&line);
                self.photo_lines.push(line);

                self.completed = self.total.saturating_sub(remaining);
                if self.completed > 0 {
                    let elapsed = self.start_time.elapsed().as_secs_f64();
                    self.last_avg_ms = (elapsed / self.completed as f64) * 1000.0;
                }
            }
            TuiEvent::Progress { total, .. } => {
                if self.total == 0 {
                    self.total = total;
                }
            }
            TuiEvent::ModelStatus { model, status } => {
                if status == "running" {
                    self.active_model = Some(model);
                } else {
                    self.active_model = None;
                }
            }
            TuiEvent::EpSelected(ep) => self.ep = ep,
            TuiEvent::Log(msg) => {
                if msg.contains("Loading") || msg.contains("Models ready") {
                    self.status = msg;
                }
            }
            TuiEvent::ScanComplete => self.done = true,
        }
    }

    fn eta_string(&self) -> String {
        if self.completed == 0 || self.total == 0 {
            return "...".to_string();
        }
        let remaining = self.total - self.completed;
        let secs = (self.last_avg_ms / 1000.0) * remaining as f64;
        if secs < 60.0 {
            format!("{}s", secs as u64)
        } else if secs < 3600.0 {
            format!("{}m", (secs / 60.0) as u64)
        } else {
            format!("{:.1}h", secs / 3600.0)
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    let size = f.area();

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(size);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(vertical[1]);

    render_header(f, app, vertical[0]);
    render_findings(f, app, horizontal[0]);
    render_photos(f, app, horizontal[1]);
    render_progress(f, app, vertical[2]);
}

fn render_header(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let pct = if app.total > 0 {
        (app.completed as f64 / app.total as f64 * 100.0) as u32
    } else {
        0
    };

    let status_line = Line::from(vec![
        Span::styled(" EP: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            app.ep.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{}", app.completed, app.total),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}p", app.findings.total_photos),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(" ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}v", app.findings.total_videos),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.0}ms", app.last_avg_ms),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("ETA: {}", app.eta_string()),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{pct}%"), Style::default().fg(Color::Cyan)),
    ]);

    let title = if app.done {
        "siegu analyze — complete"
    } else if let Some(ref model) = app.active_model {
        &format!("siegu analyze — running {model}")
    } else {
        "siegu analyze"
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(status_line).block(block);
    f.render_widget(paragraph, area);
}

fn render_findings(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let inner_height = area.height as usize;
    if inner_height == 0 {
        return;
    }
    let available = inner_height.saturating_sub(2);
    let mut lines: Vec<Line> = Vec::new();

    let title = if app.done {
        " findings — done"
    } else {
        " findings"
    };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if app.findings.object_counts.is_empty()
        && app.findings.total_faces == 0
        && app.findings.transcripts.is_empty()
        && app.findings.ocr_texts.is_empty()
    {
        lines.push(Line::from(Span::styled(
            "  waiting for results...",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let mut sorted_objects: Vec<(&String, &usize)> =
            app.findings.object_counts.iter().collect();
        sorted_objects.sort_by(|a, b| b.1.cmp(a.1));

        if !sorted_objects.is_empty() {
            lines.push(Line::from(Span::styled(
                " Objects",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
            for (cls, count) in sorted_objects.iter().take(available.saturating_sub(1)) {
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(cls.to_string(), Style::default().fg(Color::Green)),
                    Span::styled(format!(" ×{count}"), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        if app.findings.total_faces > 0 {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(" Faces: {}", app.findings.total_faces),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
        }

        if app.findings.nsfw_items > 0 {
            lines.push(Line::from(Span::styled(
                format!(" NSFW:  {} items", app.findings.nsfw_items),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
        }

        if app.findings.aesthetics_count > 0 {
            let avg = app.findings.aesthetics_sum / app.findings.aesthetics_count as f64;
            lines.push(Line::from(Span::styled(
                format!(" Aesthetic: {avg:.2} avg"),
                Style::default().fg(Color::Magenta),
            )));
        }

        if !app.findings.ocr_texts.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " OCR",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));
            for text in app.findings.ocr_texts.iter().take(3) {
                let preview = if text.len() > 28 {
                    format!("  {}...", &text[..28])
                } else {
                    format!("  {text}")
                };
                lines.push(Line::from(Span::styled(
                    preview,
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        if !app.findings.transcripts.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Transcripts",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            for t in app.findings.transcripts.iter().take(3) {
                let preview = if t.len() > 28 {
                    format!("  {}...", &t[..28])
                } else {
                    format!("  {t}")
                };
                lines.push(Line::from(Span::styled(
                    preview,
                    Style::default().fg(Color::Cyan),
                )));
            }
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn render_photos(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let inner_height = area.height as usize;
    if inner_height == 0 {
        return;
    }

    let available_lines = inner_height.saturating_sub(2);

    let mut lines: Vec<Line> = Vec::new();

    if app.photo_lines.is_empty() && !app.done {
        lines.push(Line::from(Span::styled(
            format!("  {}", app.status),
            Style::default().fg(Color::DarkGray),
        )));
    }

    for photo in app.photo_lines.iter().rev().take(available_lines / 4).rev() {
        let name_prefix = if photo.is_video { "  [video] " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{}{}", name_prefix, photo.short_name),
            Style::default()
                .fg(if photo.is_video {
                    Color::Magenta
                } else {
                    Color::Yellow
                })
                .add_modifier(Modifier::BOLD),
        )));

        let timings_str: Vec<String> = photo
            .timings
            .iter()
            .map(|(model, ms)| format!("{}:{:.0}ms", model, ms))
            .collect();
        if !timings_str.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("    {}", timings_str.join(" ")),
                Style::default().fg(Color::Gray),
            )));
        }

        let mut details: Vec<String> = Vec::new();
        if !photo.objects.is_empty() {
            let obj_str: Vec<String> = photo
                .objects
                .iter()
                .map(|(name, score)| format!("{}({})", name, score))
                .collect();
            details.push(format!("→ {}", obj_str.join(" ")));
        }
        if let Some(ref nsfw) = photo.nsfw {
            details.push(format!("NSFW: {nsfw}"));
        }
        if photo.face_count > 0 {
            details.push(format!("{} faces", photo.face_count));
        }
        if let Some(ref ocr) = photo.ocr {
            let preview = if ocr.len() > 50 {
                format!("{}...", &ocr[..50])
            } else {
                ocr.clone()
            };
            details.push(format!("→ \"{}\"", preview));
        }

        if !details.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("    {}", details.join(" │ ")),
                Style::default().fg(Color::Green),
            )));
        }

        if let Some(ref transcript) = photo.transcript {
            let preview = if transcript.len() > 60 {
                format!("{}...", &transcript[..60])
            } else {
                transcript.clone()
            };
            lines.push(Line::from(Span::styled(
                format!("    🎤 \"{}\"", preview),
                Style::default().fg(Color::Cyan),
            )));
        }

        lines.push(Line::from(""));
    }

    let block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block).scroll((0, 0));
    f.render_widget(paragraph, area);
}

fn render_progress(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let pct = if app.total > 0 {
        app.completed as f64 / app.total as f64
    } else {
        0.0
    };

    let label = if app.done {
        "Done! Press Ctrl+C to exit".to_string()
    } else {
        format!(
            "{}/{} ({}p {}v)",
            app.completed, app.total, app.findings.total_photos, app.findings.total_videos
        )
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
        .ratio(pct)
        .label(Span::styled(label, Style::default().fg(Color::White)));

    f.render_widget(gauge, area);
}

fn run_tui(ml_context: MlContext, rx: mpsc::Receiver<TuiEvent>, total: usize) {
    // Enabling raw mode is required for the interactive TUI; without it the analysis display cannot run.
    #[allow(clippy::expect_used)]
    enable_raw_mode().expect("failed to enable raw mode: required for the analysis TUI");
    let mut stdout = io::stdout();
    // The alternate screen is required for the TUI to render; failure leaves the terminal unusable.
    #[allow(clippy::expect_used)]
    execute!(stdout, EnterAlternateScreen)
        .expect("failed to enter alternate screen: required for the analysis TUI");
    let backend = CrosstermBackend::new(stdout);
    // Terminal construction is required to drive the TUI; failure means rendering cannot proceed.
    #[allow(clippy::expect_used)]
    let mut terminal =
        Terminal::new(backend).expect("failed to initialize the analysis TUI terminal");

    let mut app = App::new(total);

    loop {
        if let Err(e) = terminal.draw(|f| ui(f, &app)) {
            error!("terminal draw failed, exiting analysis TUI: {e}");
            break;
        }

        while let Ok(event) = rx.try_recv() {
            app.handle_event(event);
        }

        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    ml_context
                        .abort
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    app.should_quit = true;
                }
            }
        }

        if app.should_quit || app.done {
            if app.done && !app.should_quit {
                if let Err(e) = terminal.draw(|f| ui(f, &app)) {
                    error!("final terminal draw failed: {e}");
                }
                let _ = event::poll(Duration::from_secs(3));
            }
            break;
        }
    }

    if let Err(e) = disable_raw_mode() {
        error!("failed to disable raw mode: {e}");
    }
    if let Err(e) = execute!(terminal.backend_mut(), LeaveAlternateScreen) {
        error!("failed to leave alternate screen: {e}");
    }
    if let Err(e) = terminal.show_cursor() {
        error!("failed to show cursor: {e}");
    }
}

pub fn run_analyze_all(config_dir: &Path) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("Error: no database found at {}", db_path.display());
        std::process::exit(1);
    }

    let db = Database::new(&config_dir.display().to_string());
    let config = db.get_state();
    if !siegu_core::ml_worker::any_model_enabled(&config) {
        eprintln!("Error: no ML models enabled.");
        eprintln!("Enable models first, e.g.:");
        eprintln!("  siegu config set model_enabled_clip true");
        eprintln!("  siegu config set model_enabled_face true");
        std::process::exit(1);
    }

    let unindexed = db.get_unindexed_photos_batch(0, 100000);
    let total = unindexed.len();

    if total == 0 {
        println!("All media already analyzed.");
        return;
    }

    let config_path = config_dir.display().to_string();
    let (tx, rx) = mpsc::channel();
    let callbacks = TuiCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context.tx.send(siegu_core::ml_worker::Job::ProcessAll);

    run_tui(ml_context, rx, total);
}

pub fn run_analyze_photo(config_dir: &Path, photo_id: &str) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("Error: no database found");
        std::process::exit(1);
    }

    let db = Database::new(&config_dir.display().to_string());
    if db.get_photo_by_id(photo_id).is_none() {
        eprintln!("Error: photo not found: {photo_id}");
        std::process::exit(1);
    }

    let config_path = config_dir.display().to_string();
    let (tx, rx) = mpsc::channel();
    let callbacks = TuiCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context
        .tx
        .send(siegu_core::ml_worker::Job::AnalyzeSingle(
            photo_id.to_string(),
        ));

    run_tui(ml_context, rx, 1);
}

pub fn run_analyze_model(config_dir: &Path, model_id: &str) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("Error: no database found at {}", db_path.display());
        std::process::exit(1);
    }

    let status_model = siegu_core::ml_worker::job_status_model(model_id).unwrap_or(model_id);
    let db = Database::new(&config_dir.display().to_string());
    let missing = db.get_photos_missing_model(status_model);
    let total = missing.len();

    if total == 0 {
        println!("No media need '{model_id}' analysis.");
        return;
    }

    let config_path = config_dir.display().to_string();
    let (tx, rx) = mpsc::channel();
    let callbacks = TuiCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context.tx.send(siegu_core::ml_worker::Job::ProcessModel(
        model_id.to_string(),
    ));

    run_tui(ml_context, rx, total);
}

struct HeadlessCallbacks {
    tx: mpsc::Sender<TuiEvent>,
}

impl AnalysisCallbacks for HeadlessCallbacks {
    fn on_photo_complete(
        &self,
        photo_id: &str,
        location: &str,
        result: &PhotoResult,
        remaining: usize,
        _progress_model: Option<&str>,
    ) {
        let _ = self.tx.send(TuiEvent::PhotoComplete {
            photo_id: photo_id.to_string(),
            location: location.to_string(),
            result: result.clone(),
            remaining,
        });
    }

    fn on_progress(&self, completed: usize, total: usize, _avg_ms: f64) {
        let _ = self.tx.send(TuiEvent::Progress { completed, total });
    }

    fn on_model_status(&self, model: &str, status: &str, _pending: usize, _total: usize) {
        let _ = self.tx.send(TuiEvent::ModelStatus {
            model: model.to_string(),
            status: status.to_string(),
        });
    }

    fn on_log(&self, msg: &str) {
        let _ = self.tx.send(TuiEvent::Log(msg.to_string()));
    }

    fn on_scan_complete(&self) {
        let _ = self.tx.send(TuiEvent::ScanComplete);
    }

    fn on_metadata_updated(&self, _photo_id: &str, _caption: Option<&str>, _score: Option<f64>) {}

    fn on_ep_selected(&self, _ep: &str) {}

    fn should_abort(&self) -> bool {
        false
    }
}

fn print_e2e_summary(config_dir: &Path) {
    let db = Database::new(&config_dir.display().to_string());
    let named = db.get_people();
    let unnamed = db.get_anonymous_people_groups();
    let (photos, videos) = db.get_media_counts();

    println!("[e2e] media photos={photos} videos={videos}");
    println!(
        "[e2e] people_total={} named={} unnamed={}",
        named.len() + unnamed.len(),
        named.len(),
        unnamed.len()
    );
    for p in named {
        println!(
            "[e2e] person id={} name={} faces={}",
            p.id, p.name, p.face_count
        );
    }
    for p in unnamed {
        println!(
            "[e2e] person id={} name=unnamed faces={}",
            p.id, p.face_count
        );
    }
}

fn run_headless_loop(_ml_context: MlContext, rx: mpsc::Receiver<TuiEvent>, config_dir: &Path) {
    loop {
        match rx.recv() {
            Ok(TuiEvent::PhotoComplete {
                photo_id,
                location,
                result,
                ..
            }) => {
                let name = Path::new(&location)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or(photo_id.clone());
                let person_ids: Vec<&str> = result
                    .faces
                    .iter()
                    .filter_map(|f| f.person_id.as_deref())
                    .collect();
                println!(
                    "[e2e] photo={} faces={} people=[{}] nsfw={} aesthetics={} ocr_chars={}",
                    name,
                    result.face_count,
                    person_ids.join(","),
                    result.nsfw.as_deref().unwrap_or("-"),
                    result
                        .aesthetics
                        .map(|a| format!("{a:.3}"))
                        .unwrap_or_else(|| "-".to_string()),
                    result.ocr.as_ref().map(|o| o.chars().count()).unwrap_or(0),
                );
            }
            Ok(TuiEvent::Progress { completed, total }) => {
                println!("[e2e] progress {completed}/{total}");
            }
            Ok(TuiEvent::ModelStatus { model, status }) => {
                println!("[e2e] model {model} {status}");
            }
            Ok(TuiEvent::Log(msg)) => println!("[e2e] log {msg}"),
            Ok(TuiEvent::ScanComplete) => {
                print_e2e_summary(config_dir);
                break;
            }
            Ok(_) => {}
            Err(_) => {
                print_e2e_summary(config_dir);
                break;
            }
        }
    }
}

pub fn run_analyze_all_headless(config_dir: &Path) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("Error: no database found at {}", db_path.display());
        std::process::exit(1);
    }

    let db = Database::new(&config_dir.display().to_string());
    let config = db.get_state();
    if !siegu_core::ml_worker::any_model_enabled(&config) {
        eprintln!("Error: no ML models enabled.");
        std::process::exit(1);
    }

    let unindexed = db.get_unindexed_photos_batch(0, 100000);
    if unindexed.is_empty() {
        println!("All media already analyzed.");
        print_e2e_summary(config_dir);
        return;
    }

    let config_path = config_dir.display().to_string();
    let (tx, rx) = mpsc::channel();
    let callbacks = HeadlessCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context.tx.send(siegu_core::ml_worker::Job::ProcessAll);

    run_headless_loop(ml_context, rx, config_dir);
}

pub fn run_analyze_photo_headless(config_dir: &Path, photo_id: &str) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("Error: no database found");
        std::process::exit(1);
    }

    let db = Database::new(&config_dir.display().to_string());
    if db.get_photo_by_id(photo_id).is_none() {
        eprintln!("Error: photo not found: {photo_id}");
        std::process::exit(1);
    }

    let config_path = config_dir.display().to_string();
    let (tx, rx) = mpsc::channel();
    let callbacks = HeadlessCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context
        .tx
        .send(siegu_core::ml_worker::Job::AnalyzeSingle(
            photo_id.to_string(),
        ));

    run_headless_loop(ml_context, rx, config_dir);
}

pub fn run_analyze_model_headless(config_dir: &Path, model_id: &str) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("Error: no database found at {}", db_path.display());
        std::process::exit(1);
    }

    let status_model = siegu_core::ml_worker::job_status_model(model_id).unwrap_or(model_id);
    let db = Database::new(&config_dir.display().to_string());
    let missing = db.get_photos_missing_model(status_model);
    if missing.is_empty() {
        println!("No media need '{model_id}' analysis.");
        print_e2e_summary(config_dir);
        return;
    }

    let config_path = config_dir.display().to_string();
    let (tx, rx) = mpsc::channel();
    let callbacks = HeadlessCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context.tx.send(siegu_core::ml_worker::Job::ProcessModel(
        model_id.to_string(),
    ));

    run_headless_loop(ml_context, rx, config_dir);
}
