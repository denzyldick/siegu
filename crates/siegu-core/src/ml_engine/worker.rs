use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::database::Database;
use crate::ml_worker::{self, decrement_pending_count, increment_pending_count, Job, MlContext};

use super::models::LoadedModels;
use super::pipeline::{self, PhotoResult};

pub trait AnalysisCallbacks: Send + Sync {
    fn on_photo_complete(
        &self,
        photo_id: &str,
        result: &PhotoResult,
        remaining: usize,
        progress_model: Option<&str>,
    );
    fn on_scan_complete(&self);
    fn on_progress(&self, completed: usize, total: usize, avg_ms: f64);
    fn on_model_status(&self, model: &str, status: &str, pending: usize, total: usize);
    fn on_ep_selected(&self, ep: &str);
    fn on_log(&self, msg: &str);
    fn should_abort(&self) -> bool;
}

pub struct NoopCallbacks;

impl AnalysisCallbacks for NoopCallbacks {
    fn on_photo_complete(
        &self,
        _photo_id: &str,
        _result: &PhotoResult,
        _remaining: usize,
        _progress_model: Option<&str>,
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

pub fn start_worker<C: AnalysisCallbacks + 'static>(
    callbacks: C,
    config_path: String,
    batch_size: usize,
) -> MlContext {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Job>();
    let pending_count = Arc::new(AtomicUsize::new(0));
    let pending_count_clone = Arc::clone(&pending_count);
    let abort = Arc::new(AtomicBool::new(false));
    let abort_clone = Arc::clone(&abort);
    let db_path = config_path.clone();
    let callbacks = Arc::new(callbacks);

    std::thread::spawn(move || {
        let faces_dir = format!("{db_path}/faces");
        let _ = std::fs::create_dir_all(&faces_dir);

        let db = Arc::new(Mutex::new(Database::new(&db_path)));
        let config = db.lock().unwrap().get_state();
        let num_threads: usize = config
            .get("scan_threads")
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .unwrap();

        let avg_photo_time_ms = 1000f64;
        let mut last_auto_job: Option<Instant> = None;
        let models: Arc<Mutex<Option<LoadedModels>>> = Arc::new(Mutex::new(None));
        let total_processed = 0usize;

        while let Some(job) = rx.blocking_recv() {
            if abort_clone.load(Ordering::SeqCst) && !job.is_single() {
                continue;
            }

            let is_auto = matches!(job, Job::AutoAnalyzeSingle(_) | Job::ProcessAll);
            if is_auto {
                let mode = db
                    .lock()
                    .unwrap()
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
                let mut m = models.lock().unwrap();
                if m.is_none() {
                    callbacks.on_log("Loading AI models...");
                    let known_people = db.lock().unwrap().get_all_people_with_embeddings();
                    let loaded = super::models::load_models(&db_path, known_people);
                    callbacks.on_ep_selected(&loaded.selected_ep);
                    *m = Some(loaded);
                    callbacks.on_log("Models ready.");
                }
            }

            let (photo_ids, target_model, progress_model) = match &job {
                Job::AnalyzeSingle(id) | Job::AutoAnalyzeSingle(id) => {
                    (vec![id.clone()], None, None)
                }
                Job::AnalyzeSingleWithModel(id, model_id) => {
                    let status_model = ml_worker::job_status_model(model_id).unwrap_or(model_id);
                    (vec![id.clone()], Some(status_model.to_string()), None)
                }
                Job::ProcessModel(model_id) => {
                    if model_id == "whisper" {
                        callbacks.on_log("Video transcription not wired yet.");
                        callbacks.on_model_status(model_id, "unavailable", 0, 0);
                        (Vec::new(), None, None)
                    } else if let Some(status_model) = ml_worker::job_status_model(model_id) {
                        let lock = db.lock().unwrap();
                        (
                            lock.get_photos_missing_model(status_model),
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
                    let lock = db.lock().unwrap();
                    let photos = lock.get_unindexed_photos_batch(0, 10000);
                    (photos.iter().map(|p| p.id.clone()).collect(), None, None)
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

            let config = db.lock().unwrap().get_state();
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
                    .is_some_and(|v| v == "true")
            });
            if !has_enabled_model && target_model.is_none() {
                let lock = db.lock().unwrap();
                for pid in &photo_ids {
                    lock.update_photo_indexed(pid, 2);
                }
                continue;
            }

            let total_pending = increment_pending_count(&pending_count_clone, photo_ids.len());
            callbacks.on_progress(
                total_processed,
                total_processed + total_pending,
                avg_photo_time_ms,
            );

            let abort_flag = Arc::clone(&abort_clone);
            let db_ref = Arc::clone(&db);
            let pending_count_ref = Arc::clone(&pending_count_clone);
            let faces_dir_ref = faces_dir.clone();
            let target_model_ref = target_model.clone();
            let progress_model_ref = progress_model.clone();
            let config_ref = config.clone();
            let models_ref = Arc::clone(&models);
            let callbacks_ref = Arc::clone(&callbacks);

            pool.spawn(move || {
                let callbacks = callbacks_ref;
                let photo_ids_batches: Vec<Vec<String>> =
                    photo_ids.chunks(batch_size).map(|c| c.to_vec()).collect();

                for batch in photo_ids_batches {
                    if abort_flag.load(Ordering::SeqCst) {
                        break;
                    }

                    for photo_id in &batch {
                        if abort_flag.load(Ordering::SeqCst) {
                            break;
                        }

                        let photo_entry = {
                            let lock = db_ref.lock().unwrap();
                            lock.get_photo_by_id(photo_id)
                        };

                        if let Some(photo_entry) = photo_entry {
                            let mut result = {
                                let mut m = models_ref.lock().unwrap();
                                let models = m.as_mut().unwrap();
                                pipeline::analyze_photo(
                                    &photo_entry.id,
                                    &photo_entry.location,
                                    &photo_entry.ai_status,
                                    models,
                                    &config_ref,
                                    target_model_ref.as_deref(),
                                    &faces_dir_ref,
                                )
                            };

                            let new_people: Vec<(String, Vec<f32>)> = {
                                let lock = db_ref.lock().unwrap();
                                let mut new_people = Vec::new();
                                for face in &mut result.faces {
                                    if face.person_id.is_none() {
                                        if !face.embedding.is_empty() {
                                            let new_id =
                                                lock.create_anonymous_person(&face.embedding);
                                            new_people
                                                .push((new_id.clone(), face.embedding.clone()));
                                            face.person_id = Some(new_id);
                                        } else {
                                            let new_id = lock.create_anonymous_person(&[]);
                                            face.person_id = Some(new_id);
                                        }
                                    }
                                }
                                new_people
                            };
                            if !new_people.is_empty() {
                                let mut m = models_ref.lock().unwrap();
                                if let Some(models) = m.as_mut() {
                                    models.known_people.extend(new_people);
                                }
                            }

                            pipeline::flush_results_to_db(
                                &db_ref.lock().unwrap(),
                                &photo_entry.id,
                                &result,
                                target_model_ref.as_deref(),
                            );

                            let remaining = decrement_pending_count(&pending_count_ref);
                            callbacks.on_photo_complete(
                                &photo_entry.id,
                                &result,
                                remaining,
                                progress_model_ref.as_deref(),
                            );
                        } else {
                            decrement_pending_count(&pending_count_ref);
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
    }
}
