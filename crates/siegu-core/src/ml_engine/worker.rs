use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::database::Database;
use crate::ml_worker::{self, decrement_pending_count, increment_pending_count, Job, MlContext};

use super::models::LoadedModels;
use super::pipeline::{self, PhotoResult};

/// Cached rayon pool state: `(configured_thread_count, pool)`.
type ScanPool = Arc<Mutex<Option<(usize, Arc<rayon::ThreadPool>)>>>;

/// Number of analysis results accumulated before DB writes are flushed in a
/// single transaction (see [`pipeline::flush_results_batch_to_db`]).
const FLUSH_BATCH_SIZE: usize = 32;

/// Cosine-similarity gate for re-merging oversplit anonymous people after a
/// bulk analysis job. Averaged group centroids are reliable enough that 0.5
/// separates the same person's photos while keeping distinct people apart.
const FACE_SIM_MERGE_THRESHOLD: f32 = 0.5;

/// Upper bound on session-created anonymous people considered by the
/// post-analysis merge pass; recombination is O(P²), so huge fresh imports
/// simply skip it rather than stall the worker.
const MAX_MERGE_CANDIDATES: usize = 4096;

/// Pacing delay (in milliseconds) applied between batches of a bulk analysis
/// job. Parsed from the `batch_delay_ms` config value; clamped to 0..=2000.
pub fn batch_delay_ms_from_config(config: &HashMap<String, String>) -> u64 {
    config
        .get("batch_delay_ms")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
        .min(2000)
}

/// Share of the latest sample weight in the running EWMA of per-photo time.
/// 10% keeps the estimate responsive to current machine load while damping
/// single-photo spikes.
const EWMA_ALPHA: f64 = 0.1;

/// Fold a freshly measured per-photo wall time (microseconds) into the shared
/// EWMA stored as `f64` bits in `slot`. Lock-free: retries on CAS contention
/// between the rayon threads. Falls back to the current value if the arithmetic
/// ever produces a non-finite result.
fn update_avg_photo_time(slot: &AtomicU64, sample_us: u64) {
    let sample = sample_us as f64;
    loop {
        let cur = f64::from_bits(slot.load(Ordering::Relaxed));
        let next = cur * (1.0 - EWMA_ALPHA) + sample * EWMA_ALPHA;
        if !next.is_finite() || next <= 0.0 {
            return;
        }
        if slot
            .compare_exchange_weak(
                cur.to_bits(),
                next.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            return;
        }
    }
}

/// Waits for the next analysis job, unloading the loaded models (freeing their
/// memory) once the channel has been idle for `unload_idle` and no analysis is
/// in flight. When `unload_idle` is zero or no models are loaded, this simply
/// blocks on the channel.
fn recv_next_job(
    rx: &mut tokio::sync::mpsc::Receiver<Job>,
    models: &Arc<Mutex<Option<LoadedModels>>>,
    pending_count: &AtomicUsize,
    unload_idle: Duration,
) -> Option<Job> {
    let models_loaded = models.lock().map(|m| m.is_some()).unwrap_or(false);
    if unload_idle.is_zero() || !models_loaded {
        return rx.blocking_recv();
    }

    let deadline = Instant::now() + unload_idle;
    loop {
        if let Ok(job) = rx.try_recv() {
            return Some(job);
        }
        if rx.is_closed() {
            return None;
        }
        if Instant::now() >= deadline && pending_count.load(Ordering::SeqCst) == 0 {
            if let Ok(mut m) = models.lock() {
                if m.take().is_some() {
                    tracing::info!("Unloaded AI models after idle timeout");
                }
            }
            return rx.blocking_recv();
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub trait AnalysisCallbacks: Send + Sync {
    fn on_photo_complete(
        &self,
        photo_id: &str,
        location: &str,
        result: &PhotoResult,
        remaining: usize,
        progress_model: Option<&str>,
        is_bulk: bool,
    );
    fn on_metadata_updated(
        &self,
        photo_id: &str,
        caption: Option<&str>,
        aesthetics_score: Option<f64>,
    );
    fn on_scan_complete(&self);
    fn on_progress(&self, completed: usize, total: usize, avg_ms: f64);
    fn on_model_status(&self, model: &str, status: &str, pending: usize, total: usize);
    /// Status report that can also carry a machine-readable reason code (e.g.
    /// "low_ram", "load_failed") explaining why a model won't run. Defaults to
    /// the plain [`Self::on_model_status`].
    fn on_model_status_with_reason(
        &self,
        model: &str,
        status: &str,
        pending: usize,
        total: usize,
        _reason: Option<&str>,
    ) {
        self.on_model_status(model, status, pending, total)
    }
    fn on_ep_selected(&self, ep: &str);
    fn on_log(&self, msg: &str);
    fn should_abort(&self) -> bool;
}

pub struct NoopCallbacks;

impl AnalysisCallbacks for NoopCallbacks {
    fn on_photo_complete(
        &self,
        _photo_id: &str,
        _location: &str,
        _result: &PhotoResult,
        _remaining: usize,
        _progress_model: Option<&str>,
        _is_bulk: bool,
    ) {
    }
    fn on_metadata_updated(
        &self,
        _photo_id: &str,
        _caption: Option<&str>,
        _aesthetics_score: Option<f64>,
    ) {
    }
    fn on_scan_complete(&self) {}
    fn on_progress(&self, _completed: usize, _total: usize, _avg_ms: f64) {}
    fn on_model_status(&self, _model: &str, _status: &str, _pending: usize, _total: usize) {}
    fn on_ep_selected(&self, _ep: &str) {}
    fn on_log(&self, _msg: &str) {}
    fn should_abort(&self) -> bool {
        false
    }
}

/// Reports enabled-but-unrunnable models via `on_model_status_with_reason` so
/// the UI can explain why a model won't run on this device (low RAM, a memory
/// budget that drops it, or an ONNX session that failed to build).
///
/// "Not downloaded" models are skipped here — that's a normal pre-download
/// state the UI already surfaces.
fn report_unavailable_models<C: AnalysisCallbacks>(
    db_path: &str,
    config: &HashMap<String, String>,
    loaded: &LoadedModels,
    callbacks: &C,
) {
    let models_dir = std::path::Path::new(db_path).join("models");
    let caps = super::models::model_feasibility(&models_dir, config, &|msg| callbacks.on_log(msg));
    let caps: HashMap<&str, &super::models::ModelFeasibility> =
        caps.iter().map(|c| (c.model.as_str(), c)).collect();

    for &name in super::models::FEASIBILITY_MODELS {
        let enabled = config
            .get(&format!("model_enabled_{name}"))
            .is_none_or(|v| v == "true");
        if !enabled {
            continue;
        }
        match caps.get(name) {
            Some(cap) if !cap.runnable => {
                let reason = cap.reason.as_deref().unwrap_or("");
                if reason == super::models::REASON_NOT_DOWNLOADED {
                    continue;
                }
                callbacks.on_model_status_with_reason(name, "unavailable", 0, 0, Some(reason));
            }
            Some(_) => {
                // Feasibility can't see a session build failing on a corrupt or
                // unsupported model, so check the loaded engines directly.
                if !loaded.engine_loaded(name) {
                    callbacks.on_model_status_with_reason(
                        name,
                        "unavailable",
                        0,
                        0,
                        Some(super::models::REASON_LOAD_FAILED),
                    );
                }
            }
            None => {
                callbacks.on_log(&format!("Model {name}: no feasibility verdict"));
            }
        }
    }
}

pub fn start_worker<C: AnalysisCallbacks + 'static>(
    callbacks: C,
    config_path: String,
    batch_size: usize,
) -> MlContext {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Job>(ml_worker::JOB_CHANNEL_CAPACITY);
    let pending_count = Arc::new(AtomicUsize::new(0));
    let pending_count_clone = Arc::clone(&pending_count);
    let abort = Arc::new(AtomicBool::new(false));
    let abort_clone = Arc::clone(&abort);
    let paused = Arc::new(AtomicBool::new(false));
    let paused_clone = Arc::clone(&paused);
    let db_path = config_path.clone();
    let callbacks = Arc::new(callbacks);
    let models: Arc<Mutex<Option<LoadedModels>>> = Arc::new(Mutex::new(None));
    let models_thread = Arc::clone(&models);

    std::thread::spawn(move || {
        let faces_dir = format!("{db_path}/faces");
        let _ = std::fs::create_dir_all(&faces_dir);

        let db = Arc::new(Mutex::new(Database::new(&db_path)));
        let config = db.lock().unwrap_or_else(|e| e.into_inner()).get_state();
        let unload_idle = Duration::from_secs(
            config
                .get("ml_unload_idle_seconds")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
        );

        // Cached rayon pool for parallel photo analysis. Rebuilt lazily whenever
        // the scan_threads config value changes so the setting applies to the
        // next analysis without requiring an app restart.
        let scan_pool: ScanPool = Arc::new(Mutex::new(None));

        // Exponential moving average of measured per-photo wall time, stored as
        // f64 microseconds bits so the rayon closure can update it lock-free.
        // Initialised to a conservative 1000 ms so the first ETA shown is never
        // zero, then converges to the real measurement after a few photos.
        let avg_photo_time_us = Arc::new(AtomicU64::new(1_000_000f64.to_bits()));
        let total_processed = Arc::new(AtomicUsize::new(0));
        let mut last_auto_job: Option<Instant> = None;
        let models = models_thread;

        while let Some(job) = recv_next_job(&mut rx, &models, &pending_count_clone, unload_idle) {
            let is_reload = matches!(job, Job::ReloadModels);

            if abort_clone.load(Ordering::SeqCst) && !job.is_single() && !is_reload {
                continue;
            }

            if is_reload {
                // Force the lazy-load block below to rebuild every model from the
                // latest config. No photos are processed for this job.
                let mut m = models.lock().unwrap_or_else(|e| e.into_inner());
                *m = None;
            }

            let is_auto = matches!(job, Job::ProcessAll);
            if is_auto {
                let mode = db
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_state()
                    .get("indexing_mode")
                    .cloned()
                    .unwrap_or("immediate".to_string());
                if mode == "manual" {
                    continue;
                }
                if mode == "idle" {
                    if let Some(last) = last_auto_job {
                        let elapsed = last.elapsed();
                        if elapsed < Duration::from_secs(30) {
                            std::thread::sleep(Duration::from_secs(30) - elapsed);
                        }
                    }
                }
                last_auto_job = Some(Instant::now());
            }

            {
                let mut m = models.lock().unwrap_or_else(|e| e.into_inner());
                if m.is_none() {
                    callbacks.on_log("Loading AI models...");
                    let lock = db.lock().unwrap_or_else(|e| e.into_inner());
                    let config = lock.get_state();
                    let known_people = lock.get_all_people_with_embeddings();
                    drop(lock);
                    let loaded =
                        super::models::load_models(&db_path, &config, known_people, &|msg| {
                            callbacks.on_log(msg)
                        });
                    callbacks.on_ep_selected(&loaded.selected_ep);
                    report_unavailable_models(&db_path, &config, &loaded, &*callbacks);
                    *m = Some(loaded);
                    callbacks.on_log("Models ready.");
                }
            }

            let (photo_ids, target_model, progress_model) = match &job {
                Job::ReloadModels => (Vec::new(), None, None),
                Job::AnalyzeSingle(id) => (vec![id.clone()], None, None),
                Job::AnalyzeSingleWithModel(id, model_id) => {
                    let status_model = ml_worker::job_status_model(model_id).unwrap_or(model_id);
                    (vec![id.clone()], Some(status_model.to_string()), None)
                }
                Job::ProcessModel(model_id) => {
                    if let Some(status_model) = ml_worker::job_status_model(model_id) {
                        let lock = db.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            lock.get_photos_missing_model(status_model, None, None),
                            Some(status_model.to_string()),
                            Some(model_id.clone()),
                        )
                    } else {
                        callbacks.on_log(&format!("Unknown model: {model_id}"));
                        callbacks.on_model_status(model_id, "error", 0, 0);
                        (Vec::new(), None, None)
                    }
                }
                Job::ProcessAll => {
                    let lock = db.lock().unwrap_or_else(|e| e.into_inner());
                    // "Skip existing library": when enabled, only photos added
                    // after the cutoff rowid are analyzed; the pre-existing
                    // backlog is left untouched until the option is turned off.
                    let state = lock.get_state();
                    let cutoff = if state
                        .get("analysis_skip_existing")
                        .is_some_and(|v| v == "true")
                    {
                        state
                            .get("analysis_cutoff_rowid")
                            .and_then(|v| v.parse::<i64>().ok())
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    (
                        lock.get_unindexed_photo_ids_after(cutoff, 10000),
                        None,
                        None,
                    )
                }
            };

            if job.is_single() {
                abort_clone.store(false, Ordering::SeqCst);
            }

            if photo_ids.is_empty() {
                if let Some(ref model) = progress_model {
                    callbacks.on_log(&format!("No photos need {model} analysis."));
                    callbacks.on_model_status(model, "up_to_date", 0, 0);
                }
                continue;
            }

            if let Some(ref model) = progress_model {
                callbacks.on_log(&format!("Running {model} on {} photos.", photo_ids.len()));
                callbacks.on_model_status(model, "running", photo_ids.len(), photo_ids.len());
            }

            let config = db.lock().unwrap_or_else(|e| e.into_inner()).get_state();
            let has_enabled_model = [
                "clip",
                "face",
                "ocr",
                "nsfw",
                "aesthetics",
                "yolo",
                "blip",
                "arcface",
                "midas",
                "whisper",
            ]
            .iter()
            .any(|m| {
                config
                    .get(&format!("model_enabled_{m}"))
                    .is_none_or(|v| v == "true")
            });
            if !has_enabled_model && target_model.is_none() {
                let lock = db.lock().unwrap_or_else(|e| e.into_inner());
                for pid in &photo_ids {
                    lock.update_photo_indexed(pid, 1);
                }
                continue;
            }

            let total_pending = increment_pending_count(&pending_count_clone, photo_ids.len());
            let processed = total_processed.load(Ordering::Relaxed);
            let avg_ms = f64::from_bits(avg_photo_time_us.load(Ordering::Relaxed)) / 1000.0;
            callbacks.on_progress(processed, processed + total_pending, avg_ms);

            let is_bulk = !job.is_single();
            let batch_delay_ms = batch_delay_ms_from_config(&config);

            let abort_flag = Arc::clone(&abort_clone);
            let paused_flag = Arc::clone(&paused_clone);
            let db_ref = Arc::clone(&db);
            let pending_count_ref = Arc::clone(&pending_count_clone);
            let faces_dir_ref = faces_dir.clone();
            let target_model_ref = target_model.clone();
            let progress_model_ref = progress_model.clone();
            let config_ref = config.clone();
            let models_ref = Arc::clone(&models);
            let callbacks_ref = Arc::clone(&callbacks);
            let total_processed_ref = Arc::clone(&total_processed);
            let avg_photo_time_us_ref = Arc::clone(&avg_photo_time_us);

            let scan_threads: usize = config
                .get("scan_threads")
                .and_then(|s| s.parse().ok())
                .unwrap_or(4);
            let pool = {
                let mut pool_cell = scan_pool.lock().unwrap_or_else(|e| e.into_inner());
                match pool_cell.as_ref() {
                    Some((n, p)) if *n == scan_threads => Arc::clone(p),
                    _ => match rayon::ThreadPoolBuilder::new()
                        .num_threads(scan_threads)
                        .build()
                    {
                        Ok(p) => {
                            callbacks.on_log(&format!("Scan threads set to {scan_threads}."));
                            let wrapped = Arc::new(p);
                            *pool_cell = Some((scan_threads, Arc::clone(&wrapped)));
                            wrapped
                        }
                        Err(e) => {
                            tracing::error!(
                                "failed to build scan thread pool with {scan_threads} threads: {e}"
                            );
                            continue;
                        }
                    },
                }
            };

            pool.spawn(move || {
                let callbacks = callbacks_ref;
                let photo_ids_batches: Vec<Vec<String>> =
                    photo_ids.chunks(batch_size).map(|c| c.to_vec()).collect();

                let flush_pending =
                    |pending_flush: &mut Vec<(String, PhotoResult)>,
                     pending_people: &mut Vec<(String, Vec<f32>)>| {
                        if !pending_people.is_empty() {
                            let db = db_ref.lock().unwrap_or_else(|e| e.into_inner());
                            db.create_anonymous_people(pending_people);
                            pending_people.clear();
                        }
                        if !pending_flush.is_empty() {
                            let db = db_ref.lock().unwrap_or_else(|e| e.into_inner());
                            pipeline::flush_results_batch_to_db(
                                &db,
                                pending_flush,
                                target_model_ref.as_deref(),
                            );
                            pending_flush.clear();
                        }
                    };

                // Anonymous people created during this job. The streaming
                // per-photo match can oversplit one person into several groups
                // when a face lands just under/over the similarity threshold,
                // so after the job drains we merge those groups again (see
                // merge_similar_anonymous_people).
                let mut session_people_ids: Vec<String> = Vec::new();

                for batch in photo_ids_batches {
                    if abort_flag.load(Ordering::SeqCst) {
                        break;
                    }
                    while paused_flag.load(Ordering::SeqCst) && !abort_flag.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(500));
                    }
                    if abort_flag.load(Ordering::SeqCst) {
                        break;
                    }

                    let mut pending_flush: Vec<(String, PhotoResult)> = Vec::new();
                    let mut pending_people: Vec<(String, Vec<f32>)> = Vec::new();

                    for photo_id in &batch {
                        if abort_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        while paused_flag.load(Ordering::SeqCst)
                            && !abort_flag.load(Ordering::SeqCst)
                        {
                            std::thread::sleep(Duration::from_millis(500));
                        }
                        if abort_flag.load(Ordering::SeqCst) {
                            break;
                        }

                        let photo_entry = {
                            let lock = db_ref.lock().unwrap_or_else(|e| e.into_inner());
                            lock.get_photo_for_indexing(photo_id)
                        };

                        if let Some(photo_entry) = photo_entry {
                            let short_name = std::path::Path::new(&photo_entry.location)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| photo_entry.id.clone());
                            // Per-photo "Analyzing..." is only surfaced in
                            // single-photo mode; during bulk indexing the scan
                            // feed would be flooded with one line per photo.
                            if !is_bulk {
                                callbacks.on_log(&format!("Analyzing {short_name}..."));
                            }

                            let is_video = pipeline::is_video_file(&photo_entry.location);

                            let model_snapshot = {
                                let m = models_ref.lock().unwrap_or_else(|e| e.into_inner());
                                let Some(models) = m.as_ref() else {
                                    tracing::error!(
                                        "AI models not loaded; skipping analysis for {}",
                                        photo_entry.id
                                    );
                                    decrement_pending_count(&pending_count_ref);
                                    continue;
                                };
                                models.clone()
                            };

                            let photo_start = Instant::now();
                            let mut result = if is_video {
                                pipeline::analyze_video(
                                    &photo_entry.id,
                                    &photo_entry.location,
                                    &photo_entry.ai_status,
                                    &model_snapshot,
                                    &config_ref,
                                    target_model_ref.as_deref(),
                                    &faces_dir_ref,
                                )
                            } else {
                                pipeline::analyze_photo(
                                    &photo_entry.id,
                                    &photo_entry.location,
                                    &photo_entry.ai_status,
                                    &model_snapshot,
                                    &config_ref,
                                    target_model_ref.as_deref(),
                                    &faces_dir_ref,
                                )
                            };

                            // Update the running EWMA of per-photo wall time.
                            let elapsed_us = photo_start.elapsed().as_micros() as u64;
                            update_avg_photo_time(&avg_photo_time_us_ref, elapsed_us);

                            let new_people: Vec<(String, Vec<f32>)> = {
                                let mut new_people = Vec::new();
                                for face in &mut result.faces {
                                    if face.person_id.is_none() {
                                        let new_id = uuid::Uuid::new_v4().to_string();
                                        if !face.embedding.is_empty() {
                                            new_people
                                                .push((new_id.clone(), face.embedding.clone()));
                                        } else {
                                            new_people.push((new_id.clone(), Vec::new()));
                                        }
                                        face.person_id = Some(new_id);
                                    }
                                }
                                new_people
                            };
                            for (id, _) in &new_people {
                                session_people_ids.push(id.clone());
                            }
                            if !new_people.is_empty() {
                                let mut m = models_ref.lock().unwrap_or_else(|e| e.into_inner());
                                if let Some(models) = m.as_mut() {
                                    models.known_people.extend(new_people.iter().cloned());
                                    let recent_max: usize = config_ref
                                        .get("ml_known_people_max")
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(64);
                                    super::models::LoadedModels::trim_known_people_recent(
                                        &mut models.known_people,
                                        models.known_people_named,
                                        recent_max,
                                    );
                                }
                                pending_people.extend(new_people);
                            }

                            if result.caption.is_some() || result.aesthetics.is_some() {
                                callbacks.on_metadata_updated(
                                    &photo_entry.id,
                                    result.caption.as_deref(),
                                    result.aesthetics,
                                );
                            }

                            let remaining = decrement_pending_count(&pending_count_ref);
                            callbacks.on_photo_complete(
                                &photo_entry.id,
                                &photo_entry.location,
                                &result,
                                remaining,
                                progress_model_ref.as_deref(),
                                is_bulk,
                            );

                            pending_flush.push((photo_entry.id.clone(), result));
                            total_processed_ref.fetch_add(1, Ordering::Relaxed);
                            if pending_flush.len() >= FLUSH_BATCH_SIZE {
                                flush_pending(&mut pending_flush, &mut pending_people);
                            }
                        } else {
                            decrement_pending_count(&pending_count_ref);
                        }
                    }

                    flush_pending(&mut pending_flush, &mut pending_people);

                    // Refresh the live progress/ETA every batch: completed count
                    // and the measured EWMA average are pushed to the callbacks so
                    // the frontend ETA stays accurate while a job drains.
                    let processed = total_processed_ref.load(Ordering::Relaxed);
                    let pending = pending_count_ref.load(Ordering::Relaxed);
                    let avg_ms =
                        f64::from_bits(avg_photo_time_us_ref.load(Ordering::Relaxed)) / 1000.0;
                    callbacks.on_progress(processed, processed + pending, avg_ms);

                    if batch_delay_ms > 0 {
                        std::thread::sleep(Duration::from_millis(batch_delay_ms));
                    }
                }

                if !session_people_ids.is_empty()
                    && session_people_ids.len() <= MAX_MERGE_CANDIDATES
                {
                    let (kept, dropped) = {
                        let db = db_ref.lock().unwrap_or_else(|e| e.into_inner());
                        db.merge_similar_anonymous_people(
                            &session_people_ids,
                            FACE_SIM_MERGE_THRESHOLD,
                        )
                    };
                    if !dropped.is_empty() {
                        let mut m = models_ref.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(models) = m.as_mut() {
                            models.known_people.retain(|(id, _)| !dropped.contains(id));
                            for (id, centroid) in kept {
                                if let Some(entry) =
                                    models.known_people.iter_mut().find(|(kid, _)| *kid == id)
                                {
                                    entry.1 = centroid;
                                }
                            }
                        }
                    }
                }

                callbacks.on_scan_complete();
            });
        }
    });

    MlContext {
        tx,
        pending_count,
        abort,
        paused,
        models,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn batch_delay_defaults_to_zero_when_missing() {
        let config = HashMap::new();
        assert_eq!(batch_delay_ms_from_config(&config), 0);
    }

    #[test]
    fn batch_delay_reads_valid_value() {
        let mut config = HashMap::new();
        config.insert("batch_delay_ms".to_string(), "500".to_string());
        assert_eq!(batch_delay_ms_from_config(&config), 500);
    }

    #[test]
    fn batch_delay_clamps_to_max() {
        let mut config = HashMap::new();
        config.insert("batch_delay_ms".to_string(), "99999".to_string());
        assert_eq!(batch_delay_ms_from_config(&config), 2000);
    }

    #[test]
    fn batch_delay_ignores_garbage() {
        let mut config = HashMap::new();
        config.insert("batch_delay_ms".to_string(), "soon".to_string());
        assert_eq!(batch_delay_ms_from_config(&config), 0);
    }

    #[test]
    fn ewma_starts_at_1s_default_then_converges() {
        let slot = AtomicU64::new(1_000_000f64.to_bits());
        // A single fast sample barely moves the initial 1s default.
        update_avg_photo_time(&slot, 10_000); // 10ms
        let ms = f64::from_bits(slot.load(Ordering::Relaxed)) / 1000.0;
        assert!(ms > 100.0, "default should dominate the first sample: {ms}");
        // Many fast samples pull it down toward ~10ms.
        for _ in 0..200 {
            update_avg_photo_time(&slot, 10_000);
        }
        let ms = f64::from_bits(slot.load(Ordering::Relaxed)) / 1000.0;
        assert!(ms < 50.0, "EWMA should converge toward the sample: {ms}ms");
    }

    #[test]
    fn ewma_tracks_a_faster_rate() {
        let slot = AtomicU64::new(1_000_000f64.to_bits());
        for _ in 0..5000 {
            update_avg_photo_time(&slot, 5_000); // 5ms
        }
        let ms = f64::from_bits(slot.load(Ordering::Relaxed)) / 1000.0;
        assert!(ms < 10.0, "should track a 5ms rate, got {ms}ms");
    }

    #[test]
    fn ewma_folds_instant_sample_toward_zero() {
        let slot = AtomicU64::new(50_000f64.to_bits()); // 50ms
        for _ in 0..2000 {
            update_avg_photo_time(&slot, 0); // instantaneous photo
        }
        let us = f64::from_bits(slot.load(Ordering::Relaxed));
        assert!(
            us < 50.0,
            "instant photos should pull the average toward 0, got {us}us"
        );
    }
}
