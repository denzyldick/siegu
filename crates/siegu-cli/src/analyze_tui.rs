use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

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
    timings: Vec<(String, f64)>,
    objects: Vec<(String, String)>,
    face_count: usize,
    nsfw: Option<String>,
    ocr: Option<String>,
}

struct App {
    photo_lines: Vec<PhotoLine>,
    completed: usize,
    total: usize,
    ep: String,
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
            completed: 0,
            total,
            ep: "CPU".to_string(),
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

                let mut timings: Vec<(String, f64)> = result
                    .model_timings
                    .iter()
                    .map(|(k, v)| (k.clone(), v * 1000.0))
                    .collect();
                timings.sort_by(|a, b| a.0.cmp(&b.0));

                let objects = result.objects.iter().take(5).cloned().collect();

                self.photo_lines.push(PhotoLine {
                    short_name,
                    timings,
                    objects,
                    face_count: result.face_count,
                    nsfw: result.nsfw.clone(),
                    ocr: result.ocr.clone(),
                });

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
            TuiEvent::Log(_) => {}
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

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(size);

    render_header(f, app, chunks[0]);
    render_photos(f, app, chunks[1]);
    render_progress(f, app, chunks[2]);
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
            format!("{}/{} photos", app.completed, app.total),
            Style::default().fg(Color::White),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.0}ms/photo", app.last_avg_ms),
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

fn render_photos(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let inner_height = area.height as usize;
    if inner_height == 0 {
        return;
    }

    let available_lines = inner_height.saturating_sub(2);

    let mut lines: Vec<Line> = Vec::new();

    if app.photo_lines.is_empty() && !app.done {
        lines.push(Line::from(Span::styled(
            "  Waiting for first photo...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    for photo in app.photo_lines.iter().rev().take(available_lines / 3).rev() {
        lines.push(Line::from(Span::styled(
            format!("  {}", photo.short_name),
            Style::default()
                .fg(Color::Yellow)
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
        format!("{}/{}", app.completed, app.total)
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

fn run_tui(ml_context: MlContext, total: usize) {
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(total);

    loop {
        terminal.draw(|f| ui(f, &app)).unwrap();

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
                terminal.draw(|f| ui(f, &app)).unwrap();
                let _ = event::poll(Duration::from_secs(3));
            }
            break;
        }
    }

    disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
    terminal.show_cursor().unwrap();
}

pub fn run_analyze_all(config_dir: &Path) {
    let db_path = config_dir.join("siegu.db");
    if !db_path.exists() {
        eprintln!("Error: no database found at {}", db_path.display());
        std::process::exit(1);
    }

    let db = Database::new(&config_dir.display().to_string());
    let unindexed = db.get_unindexed_photos_batch(0, 100000);
    let total = unindexed.len();

    if total == 0 {
        println!("All photos already analyzed.");
        return;
    }

    let config_path = config_dir.display().to_string();
    let (tx, _rx) = mpsc::channel();
    let callbacks = TuiCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context.tx.send(siegu_core::ml_worker::Job::ProcessAll);

    run_tui(ml_context, total);
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
    let (tx, _rx) = mpsc::channel();
    let callbacks = TuiCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context
        .tx
        .send(siegu_core::ml_worker::Job::AnalyzeSingle(
            photo_id.to_string(),
        ));

    run_tui(ml_context, 1);
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
        println!("No photos need '{model_id}' analysis.");
        return;
    }

    let config_path = config_dir.display().to_string();
    let (tx, _rx) = mpsc::channel();
    let callbacks = TuiCallbacks { tx };
    let ml_context = start_worker(callbacks, config_path, 32);

    let _ = ml_context.tx.send(siegu_core::ml_worker::Job::ProcessModel(
        model_id.to_string(),
    ));

    run_tui(ml_context, total);
}
