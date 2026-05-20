use crate::database::{Database, Face};
use crate::emit_log;
use base64::Engine;
use ndarray::{Array2, Array4};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Emitter;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

// Conditional imports for AI Engines
#[cfg(not(target_os = "android"))]
use ort::{session::builder::GraphOptimizationLevel, session::Session};

#[cfg(target_os = "android")]
use tract_onnx::prelude::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Job {
    ProcessAll,
    AutoAnalyzeSingle(String),
    AnalyzeSingle(String),
    AnalyzeSingleWithModel(String, String),
    ProcessModel(String),
}

pub struct MlContext {
    pub tx: UnboundedSender<Job>,
    pub pending_count: Arc<AtomicUsize>,
    pub abort: Arc<std::sync::atomic::AtomicBool>,
}

const MAX_REASONABLE_PENDING: usize = 1_000_000;
type FaceEmbeddingStore = Arc<Mutex<Vec<(String, Vec<f32>)>>>;

fn job_status_model(model_id: &str) -> Option<&'static str> {
    match model_id {
        "clip" => Some("clip"),
        "ultraface" | "face" => Some("face"),
        "ocr" => Some("ocr"),
        "nsfw" => Some("nsfw"),
        "aesthetics" => Some("aesthetics"),
        "yolo" => Some("yolo"),
        "blip" => Some("blip"),
        "arcface" => Some("arcface"),
        "midas" => Some("midas"),
        "whisper" => Some("whisper"),
        _ => None,
    }
}

fn should_run_model(target_model: Option<&str>, model: &str, config: Option<&std::collections::HashMap<String, String>>) -> bool {
    if let Some(config) = config {
        let key = format!("model_enabled_{}", model);
        if let Some(val) = config.get(&key) {
            if val != "true" {
                return false;
            }
        }
    }
    target_model.is_none_or(|target| target == model)
}

// The pending counter is shared by UI commands, discovery, and background model work.
// Clamp obviously invalid values so stale underflows do not leave the UI stuck indexing forever.
fn decrement_pending_count(counter: &AtomicUsize) -> usize {
    counter
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            if current > MAX_REASONABLE_PENDING {
                Some(0)
            } else {
                Some(current.saturating_sub(1))
            }
        })
        .map(|previous| {
            if previous > MAX_REASONABLE_PENDING {
                0
            } else {
                previous.saturating_sub(1)
            }
        })
        .unwrap_or(0)
}

fn increment_pending_count(counter: &AtomicUsize, amount: usize) -> usize {
    counter
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            let base = if current > MAX_REASONABLE_PENDING {
                0
            } else {
                current
            };
            Some(base.saturating_add(amount))
        })
        .map(|previous| {
            let base = if previous > MAX_REASONABLE_PENDING {
                0
            } else {
                previous
            };
            base.saturating_add(amount)
        })
        .unwrap_or(amount)
}

// Model wrappers to handle different engine types
#[derive(Clone)]
pub(crate) enum ModelEngine {
    #[cfg(not(target_os = "android"))]
    Ort(Arc<Mutex<Session>>),
    #[cfg(target_os = "android")]
    Tract(Arc<SimplePlan<TypedFact, Box<dyn TypedOp>, TypedModel>>),
}

impl ModelEngine {
    pub(crate) fn run(&self, input: Array4<f32>, _input_name: &str) -> Result<Vec<f32>, String> {
        match self {
            #[cfg(not(target_os = "android"))]
            ModelEngine::Ort(session) => {
                let shape = input.shape().to_vec();
                let data = input.into_raw_vec_and_offset().0;
                let tensor =
                    ort::value::Value::from_array((shape, data)).map_err(|e| e.to_string())?;
                let mut lock = session.lock().unwrap();
                let outputs = lock
                    .run(ort::inputs![_input_name => &tensor])
                    .map_err(|e| e.to_string())?;
                let mut results = Vec::new();
                for i in 0..outputs.len() {
                    if let Ok((_shape, data)) = outputs[i].try_extract_tensor::<f32>() {
                        results.extend_from_slice(data);
                    }
                }
                Ok(results)
            }
            #[cfg(target_os = "android")]
            ModelEngine::Tract(plan) => {
                let tract_tensor: tract_onnx::prelude::Tensor = input.into();
                let mut inputs = vec![];
                let input_count = plan.model().input_outlets().unwrap().len();
                for _ in 0..input_count {
                    inputs.push(tract_tensor.clone().into());
                }
                let result = plan.run(inputs.into()).map_err(|e| e.to_string())?;
                let mut results = Vec::new();
                for output in result.iter() {
                    if let Ok(output) = output.as_slice::<f32>() {
                        results.extend_from_slice(output);
                    }
                }
                Ok(results)
            }
        }
    }
}

fn compute_text_embeddings(
    #[cfg(not(target_os = "android"))] text_model: &mut Session,
    #[cfg(target_os = "android")] text_model: &SimplePlan<TypedFact, Box<dyn TypedOp>, TypedModel>,
    tokenizer: &tokenizers::Tokenizer,
) -> Vec<(String, Vec<f32>)> {
    let search_vocabulary = vec![
        "a passport",
        "a driver's license",
        "an id card",
        "a document",
        "a receipt",
        "a screenshot",
        "a meme",
        "a text message",
        "a cat",
        "a dog",
        "a pet",
        "an animal",
        "a car",
        "a vehicle",
        "a motorcycle",
        "a bicycle",
        "a person",
        "a selfie",
        "a group of people",
        "a crowd",
        "a building",
        "a house",
        "architecture",
        "a city",
        "a landscape",
        "nature",
        "a mountain",
        "a beach",
        "water",
        "food",
        "a meal",
        "a drink",
        "coffee",
        "a laptop",
        "a computer",
        "a phone",
        "a screen",
        "electronics",
        "a piece of furniture",
        "a room interior",
        "a sunset",
        "the sky",
        "clouds",
        "art",
        "a drawing",
        "a painting",
    ];

    let mut embeddings = Vec::new();
    for text_label in search_vocabulary {
        if let Ok(encoding) = tokenizer.encode(format!("a photo of {text_label}"), true) {
            #[cfg(not(target_os = "android"))]
            let mut ids = encoding
                .get_ids()
                .iter()
                .map(|&x| x as i64)
                .collect::<Vec<i64>>();
            #[cfg(target_os = "android")]
            let mut ids = encoding
                .get_ids()
                .iter()
                .map(|&x| x as i32)
                .collect::<Vec<i32>>();

            if ids.len() > 77 {
                ids.truncate(77);
            } else {
                while ids.len() < 77 {
                    ids.push(0);
                }
            }

            #[cfg(not(target_os = "android"))]
            let arr = Array2::from_shape_vec((1, 77), ids).unwrap();
            #[cfg(target_os = "android")]
            let arr = Array2::from_shape_vec((1, 77), ids).unwrap();

            #[cfg(not(target_os = "android"))]
            {
                let shape = arr.shape().to_vec();
                let data = arr.into_raw_vec_and_offset().0;
                if let Ok(id_tensor) = ort::value::Value::from_array((shape, data)) {
                    if let Ok(outputs) = text_model.run(ort::inputs!["input_ids" => &id_tensor]) {
                        if let Ok((_shape, text_emb_tensor)) =
                            outputs[0].try_extract_tensor::<f32>()
                        {
                            let mut text_embedding = vec![0.0; 512];
                            text_embedding.copy_from_slice(text_emb_tensor);
                            let text_norm: f32 =
                                text_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                            if text_norm > 0.0 {
                                for v in text_embedding.iter_mut() {
                                    *v /= text_norm;
                                }
                            }
                            embeddings.push((text_label.to_string(), text_embedding));
                        }
                    }
                }
            }

            #[cfg(target_os = "android")]
            {
                let tract_tensor: tract_onnx::prelude::Tensor = arr.into();
                let mut inputs = vec![];
                let input_count = text_model.model().input_outlets().unwrap().len();
                for _ in 0..input_count {
                    inputs.push(tract_tensor.clone().into());
                }

                if let Ok(result) = text_model.run(inputs.into()) {
                    if let Some(output) = result[0].as_slice::<f32>().ok() {
                        let mut text_embedding = output.to_vec();
                        if text_embedding.len() > 512 {
                            text_embedding.truncate(512);
                        }
                        let text_norm: f32 =
                            text_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                        if text_norm > 0.0 {
                            for v in text_embedding.iter_mut() {
                                *v /= text_norm;
                            }
                        }
                        embeddings.push((text_label.to_string(), text_embedding));
                    }
                }
            }
        }
    }
    embeddings
}

pub fn start_background_worker(
    app: &AppHandle,
    config_path: String,
) -> (
    UnboundedSender<Job>,
    Arc<AtomicUsize>,
    Arc<std::sync::atomic::AtomicBool>,
) {
    let (tx, mut rx) = unbounded_channel::<Job>();
    let pending_count = Arc::new(AtomicUsize::new(0));
    let pending_count_clone = Arc::clone(&pending_count);
    let abort = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let abort_clone = Arc::clone(&abort);
    let app_handle = app.clone();
    let db_path = config_path.clone();

    std::thread::spawn(move || {
        let models_dir = format!("{db_path}/models");
        let faces_dir = format!("{db_path}/faces");
        let _ = fs::create_dir_all(&models_dir);
        let _ = fs::create_dir_all(&faces_dir);

        let clip_visual_path = Path::new(&models_dir).join("clip-vit-base-patch32-visual.onnx");
        let clip_text_path = Path::new(&models_dir).join("clip-vit-base-patch32-text.onnx");
        let clip_tokenizer_path = Path::new(&models_dir).join("tokenizer.json");
        let ultraface_path = Path::new(&models_dir).join("version-RFB-320.onnx");
        let ocr_det_path = Path::new(&models_dir).join("ocr_det.onnx");
        let ocr_rec_path = Path::new(&models_dir).join("ocr_rec.onnx");
        let nsfw_path = Path::new(&models_dir).join("nsfw.onnx");
        let aesthetics_path = Path::new(&models_dir).join("aesthetics.onnx");
        let yolo_path = Path::new(&models_dir).join("yolov8.onnx");
        let blip_path = Path::new(&models_dir).join("blip.onnx");
        let arcface_path = Path::new(&models_dir).join("arcface.onnx");
        let midas_path = Path::new(&models_dir).join("midas.onnx");
        let whisper_path = Path::new(&models_dir).join("whisper.onnx");
        let ocr_dict_path = Path::new(&models_dir).join("en_dict.txt");

        let mut clip_visual: Option<ModelEngine> = None;
        let mut face_detector: Option<ModelEngine> = None;
        let mut ocr_det: Option<ModelEngine> = None;
        let mut ocr_rec: Option<ModelEngine> = None;
        let mut nsfw_model: Option<ModelEngine> = None;
        let mut aesthetics_model: Option<ModelEngine> = None;
        let mut yolo_model: Option<ModelEngine> = None;
        let mut blip_model: Option<ModelEngine> = None;
        let mut arcface_model: Option<ModelEngine> = None;
        let mut midas_model: Option<ModelEngine> = None;
        let mut whisper_model: Option<ModelEngine> = None;
        let mut text_embeddings: Arc<Vec<(String, Vec<f32>)>> = Arc::new(Vec::new());
        let known_people: FaceEmbeddingStore = Arc::new(Mutex::new(Vec::new()));
        let mut ocr_alphabet: Arc<Vec<String>> = Arc::new(Vec::new());
        let mut engine_initialized = false;

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
        let mut avg_photo_time_ms = 1000f64;
        let mut last_auto_job: Option<Instant> = None;

        while let Some(job) = rx.blocking_recv() {
            if abort_clone.load(Ordering::SeqCst) && !matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_)) {
                continue;
            }

            // Check indexing mode for auto-triggered jobs
            let is_auto = matches!(job, Job::AutoAnalyzeSingle(_) | Job::ProcessAll);
            if is_auto {
                let mode = db.lock().unwrap().get_state().get("indexing_mode").cloned().unwrap_or("immediate".to_string());
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

            if !engine_initialized {
                emit_log(
                    &app_handle,
                    "ML Worker: Initializing AI Engines...".to_string(),
                );
                #[cfg(not(target_os = "android"))]
                {
                    let _ = ort::init().with_name("siegu").commit();
                }
                engine_initialized = true;

                let is_ok = |p: &Path| {
                    p.exists() && p.metadata().map(|m| m.len()).unwrap_or(0) > 1024 * 1024
                };
                let tokenizer = tokenizers::Tokenizer::from_file(&clip_tokenizer_path)
                    .ok()
                    .map(Arc::new);

                if is_ok(&ultraface_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&ultraface_path)
                    {
                        face_detector = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&clip_visual_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&clip_visual_path)
                    {
                        clip_visual = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&clip_text_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Some(tokenizer) = tokenizer.as_ref() {
                        if let Ok(mut s) = Session::builder()
                            .unwrap()
                            .with_optimization_level(GraphOptimizationLevel::Disable)
                            .unwrap()
                            .commit_from_file(&clip_text_path)
                        {
                            text_embeddings = Arc::new(compute_text_embeddings(&mut s, tokenizer));
                        }
                    }
                }
                if is_ok(&ocr_det_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&ocr_det_path)
                    {
                        ocr_det = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&ocr_rec_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&ocr_rec_path)
                    {
                        ocr_rec = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&nsfw_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&nsfw_path)
                    {
                        nsfw_model = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&aesthetics_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&aesthetics_path)
                    {
                        aesthetics_model = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&yolo_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&yolo_path)
                    {
                        yolo_model = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&blip_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&blip_path)
                    {
                        blip_model = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&arcface_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&arcface_path)
                    {
                        arcface_model = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&midas_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&midas_path)
                    {
                        midas_model = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if is_ok(&whisper_path) {
                    #[cfg(not(target_os = "android"))]
                    if let Ok(s) = Session::builder()
                        .unwrap()
                        .with_optimization_level(GraphOptimizationLevel::Disable)
                        .unwrap()
                        .commit_from_file(&whisper_path)
                    {
                        whisper_model = Some(ModelEngine::Ort(Arc::new(Mutex::new(s))));
                    }
                }
                if ocr_dict_path.exists() {
                    let dict = fs::read_to_string(&ocr_dict_path).unwrap_or_default();
                    let mut alphabet = vec!["blank".to_string()];
                    alphabet.extend(dict.lines().map(|s| s.to_string()));
                    alphabet.push(" ".to_string());
                    ocr_alphabet = Arc::new(alphabet);
                }
                let people_vec = db.lock().unwrap().get_all_people_with_embeddings();
                if let Ok(mut lock) = known_people.lock() {
                    *lock = people_vec;
                }
                emit_log(&app_handle, "ML Worker: Engine Ready.".to_string());
            }

            let is_single = matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _));
            let (photo_ids, target_model, progress_model) = match job {
                Job::AnalyzeSingle(id) | Job::AutoAnalyzeSingle(id) => (vec![id], None, None),
                Job::AnalyzeSingleWithModel(id, model_id) => {
                    let status_model = job_status_model(&model_id).unwrap_or(&model_id);
                    eprintln!("[siegu-bench] AnalyzeSingleWithModel: photo={id} model={model_id} status_model={status_model}");
                    (vec![id], Some(status_model.to_string()), None)
                }
                Job::ProcessModel(model_id) => {
                    if model_id == "whisper" {
                        emit_log(
                            &app_handle,
                            "ML Worker: Audio Search is downloaded but video transcription is not wired into the worker yet.".to_string(),
                        );
                        let _ = app_handle.emit(
                            "model-progress",
                            serde_json::json!({
                                "model": model_id,
                                "pending": 0,
                                "total": 0,
                                "status": "unavailable",
                                "message": "Video transcription is not wired yet"
                            }),
                        );
                        (Vec::new(), None, None)
                    } else if let Some(status_model) = job_status_model(&model_id) {
                        let lock = db.lock().unwrap();
                        (
                            lock.get_photos_missing_model(status_model),
                            Some(status_model.to_string()),
                            Some(model_id),
                        )
                    } else {
                        emit_log(
                            &app_handle,
                            format!("ERROR: Unknown AI model requested: {model_id}"),
                        );
                        let _ = app_handle.emit(
                            "model-progress",
                            serde_json::json!({
                                "model": model_id,
                                "pending": 0,
                                "total": 0,
                                "status": "error",
                                "message": "Unknown AI model"
                            }),
                        );
                        (Vec::new(), None, None)
                    }
                }
                Job::ProcessAll => {
                    let lock = db.lock().unwrap();
                    (
                        lock.get_unindexed_photos()
                            .iter()
                            .map(|p| p.id.clone())
                            .collect(),
                        None,
                        None,
                    )
                }
            };

            // Single-photo jobs should always run — clear the abort flag that
            // the calling command may have set to interrupt a running batch.
            if is_single {
                abort_clone.store(false, std::sync::atomic::Ordering::SeqCst);
            }

            if photo_ids.is_empty() {
                if let Some(ref model) = progress_model {
                    emit_log(
                        &app_handle,
                        format!("ML Worker: No photos need {model} analysis."),
                    );
                    let _ = app_handle.emit(
                        "model-progress",
                        serde_json::json!({
                            "model": model,
                            "pending": 0,
                            "total": 0,
                            "status": "up_to_date",
                            "message": "No photos need this model"
                        }),
                    );
                }
                continue;
            }

            if let Some(ref model) = progress_model {
                emit_log(
                    &app_handle,
                    format!("ML Worker: Running {model} on {} photos.", photo_ids.len()),
                );
                let _ = app_handle.emit(
                    "model-progress",
                    serde_json::json!({
                        "model": model,
                        "pending": photo_ids.len(),
                        "total": photo_ids.len(),
                        "status": "running"
                    }),
                );
            }

            let total_pending = increment_pending_count(&pending_count_clone, photo_ids.len());
            let _ = app_handle.emit("indexing-progress", total_pending);

            for photo_id in photo_ids {
                if abort_clone.load(Ordering::SeqCst) {
                    break;
                }
                let start_time = std::time::Instant::now();
                let target_model_task = target_model.clone();
                let progress_model_task = progress_model.clone();

                let photo_entry = {
                    let lock = db.lock().unwrap();
                    let sql = "SELECT p.id, p.location, p.encoded, p.latitude, p.longitude, p.created, p.indexed, p.caption, p.aesthetics_score, 
                        s.clip, s.face, s.ocr, s.nsfw, s.aesthetics, s.yolo, s.blip, s.arcface, s.midas, s.whisper, s.sam, s.superres 
                        FROM photo p LEFT JOIN ai_status s ON p.id = s.photo_id WHERE p.id = ?1";
                    lock.connection
                        .query_row(sql, [&photo_id], |row| {
                            Ok(crate::database::Photo {
                                id: row.get(0)?,
                                location: row.get(1)?,
                                encoded: row.get(2)?,
                                created: row.get(5).unwrap_or_default(),
                                objects: std::collections::HashMap::new(),
                                properties: std::collections::HashMap::new(),
                                latitude: row.get(3).unwrap_or(0.0),
                                longitude: row.get(4).unwrap_or(0.0),
                                favorite: false,
                                indexed: row.get(6).unwrap_or(0),
                                caption: row.get(7).ok(),
                                aesthetics_score: row.get(8).ok(),
                                ai_status: crate::database::AiStatus {
                                    clip: row.get(9).unwrap_or(0),
                                    face: row.get(10).unwrap_or(0),
                                    ocr: row.get(11).unwrap_or(0),
                                    nsfw: row.get(12).unwrap_or(0),
                                    aesthetics: row.get(13).unwrap_or(0),
                                    yolo: row.get(14).unwrap_or(0),
                                    blip: row.get(15).unwrap_or(0),
                                    arcface: row.get(16).unwrap_or(0),
                                    midas: row.get(17).unwrap_or(0),
                                    whisper: row.get(18).unwrap_or(0),
                                    sam: row.get(19).unwrap_or(0),
                                    superres: row.get(20).unwrap_or(0),
                                },
                            })
                        })
                        .ok()
                };

                if let Some(photo_entry) = photo_entry {
                    let photo_id_task = photo_entry.id.clone();
                    let photo_loc_actual = photo_entry.location.clone();
                    let app_handle_task = app_handle.clone();
                    let pending_count_task = Arc::clone(&pending_count_clone);

                    let _ = app_handle.emit(
                        "current-ai-job",
                        serde_json::json!({
                            "id": photo_id,
                            "filename": Path::new(&photo_entry.location)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(""),
                            "model": target_model.clone().unwrap_or_default(),
                        }),
                    );
                    let db_task = Arc::clone(&db);

                    let clip_visual_task = clip_visual.clone();
                    let face_detector_task = face_detector.clone();
                    let ocr_det_task = ocr_det.clone();
                    let ocr_rec_task = ocr_rec.clone();
                    let nsfw_task = nsfw_model.clone();
                    let aesthetics_task = aesthetics_model.clone();
                    let yolo_task = yolo_model.clone();
                    let blip_task = blip_model.clone();
                    let arcface_task = arcface_model.clone();
                    let midas_task = midas_model.clone();
                    let _whisper_task = whisper_model.clone();
                    let ocr_alphabet_task = ocr_alphabet.clone();
                    let text_embeddings_task = text_embeddings.clone();
                    let known_people_task = known_people.clone();
                    let faces_dir_task = faces_dir.clone();
                    let abort_task = Arc::clone(&abort_clone);
                    let target_model_inner = target_model_task.clone();
                    let progress_model_inner = progress_model_task.clone();

                    pool.spawn(move || {
                        let mut model_timings = std::collections::HashMap::new();
                        let config = {
                            let lock = db_task.lock().unwrap();
                            lock.get_state()
                        };
                        let image_res = image::open(&photo_loc_actual);
                        if abort_task.load(Ordering::SeqCst) {
                            // aborted — skip processing, still emit event below
                        } else {
                            if let Ok(dynamic_img) = image_res {
                                let img = dynamic_img.to_rgb8();

                            // Tier 2: CLIP Visual
                            if should_run_model(target_model_inner.as_deref(), "clip", Some(&config)) && photo_entry.ai_status.clip == 0 {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref visual_model) = clip_visual_task {
                                    let resized = image::imageops::resize(&img, 224, 224, image::imageops::FilterType::Triangle);
                                    let mut input_img = Array4::<f32>::zeros((1, 3, 224, 224));
                                    for (x, y, pixel) in resized.enumerate_pixels() {
                                        input_img[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.48145466) / 0.26862954;
                                        input_img[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.4578275) / 0.2613026;
                                        input_img[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.40821073) / 0.2757771;
                                    }
                                    if let Ok(data) = visual_model.run(input_img, "pixel_values") {
                                        let mut visual_embedding = data;
                                        let visual_norm: f32 = visual_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                                        if visual_norm > 0.0 { for v in visual_embedding.iter_mut() { *v /= visual_norm; } }
                                        let mut similarities = Vec::new();
                                        for (text_label, text_embedding) in text_embeddings_task.iter() {
                                            let dot_product: f32 = visual_embedding.iter().zip(text_embedding.iter()).map(|(a, b)| a * b).sum();
                                            similarities.push((text_label, dot_product));
                                        }
                                        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                        let top_matches = similarities
                                            .iter()
                                            .take(5)
                                            .map(|(class_name, score)| format!("{class_name} ({score:.2})"))
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        let lock = db_task.lock().unwrap();
                                        for (class_name, score) in similarities.iter().take(5) {
                                            let _ = lock.connection.execute("INSERT INTO object (photo_id, class, probability) VALUES(?1, ?2, ?3)", (&photo_id_task, class_name, &score.to_string()));
                                        }
                                        lock.update_ai_status(&photo_id_task, "clip", 1);
                                        model_timings.insert("clip".to_string(), __start.elapsed().as_secs_f64());
                                        emit_log(&app_handle_task, format!("ML Worker: CLIP tags for {photo_id_task}: {top_matches}"));
                                    }
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: CLIP model is not loaded. Download or update Smart Search first.".to_string());
                                }
                            }

                            // Tier 1: Aesthetics
                            if should_run_model(target_model_inner.as_deref(), "aesthetics", Some(&config)) && photo_entry.ai_status.aesthetics == 0 {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref model) = aesthetics_task {
                                    let resized = image::imageops::resize(&img, 224, 224, image::imageops::FilterType::Triangle);
                                    let mut input = Array4::<f32>::zeros((1, 3, 224, 224));
                                    for (x, y, pixel) in resized.enumerate_pixels() {
                                        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.48145466) / 0.26862954;
                                        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.4578275) / 0.2613026;
                                        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.40821073) / 0.2757771;
                                    }
                                    if let Ok(data) = model.run(input, "input") {
                                        let score = data[0];
                                        let lock = db_task.lock().unwrap();
                                        let _ = lock.connection.execute("UPDATE photo SET aesthetics_score = ?1 WHERE id = ?2", (score as f64, &photo_id_task));
                                        lock.update_ai_status(&photo_id_task, "aesthetics", 1);
                                        model_timings.insert("aesthetics".to_string(), __start.elapsed().as_secs_f64());
                                        emit_log(&app_handle_task, format!("ML Worker: Aesthetic score for {photo_id_task}: {score:.2}"));
                                    }
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: Aesthetics model is not loaded. Download or update Quality Scorer first.".to_string());
                                }
                            }

                            // Tier 1: NSFW
                            if should_run_model(target_model_inner.as_deref(), "nsfw", Some(&config)) && photo_entry.ai_status.nsfw == 0 {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref model) = nsfw_task {
                                    let resized = image::imageops::resize(&img, 224, 224, image::imageops::FilterType::Triangle);
                                    let mut input = Array4::<f32>::zeros((1, 3, 224, 224));
                                    for (x, y, pixel) in resized.enumerate_pixels() {
                                        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.48145466) / 0.26862954;
                                        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.4578275) / 0.2613026;
                                        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.40821073) / 0.2757771;
                                    }
                                    if let Ok(data) = model.run(input, "pixel_values") {
                                        let nsfw_score = if data.len() >= 2 { let e0 = data[0].exp(); let e1 = data[1].exp(); e1 / (e0 + e1) } else { data[0] };
                                        let lock = db_task.lock().unwrap();
                                        let _ = lock.connection.execute("INSERT INTO properties (photo_id, key, value) VALUES(?1, 'nsfw', ?2)", (&photo_id_task, &nsfw_score.to_string()));
                                        lock.update_ai_status(&photo_id_task, "nsfw", 1);
                                        model_timings.insert("nsfw".to_string(), __start.elapsed().as_secs_f64());
                                        emit_log(&app_handle_task, format!("ML Worker: NSFW score for {photo_id_task}: {nsfw_score:.2}"));
                                    }
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: NSFW model is not loaded. Download or update Safe Mode first.".to_string());
                                }
                            }

                            // Tier 2: OCR
                            if should_run_model(target_model_inner.as_deref(), "ocr", Some(&config)) && photo_entry.ai_status.ocr == 0 {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref _det_model) = ocr_det_task {
                                    if let Some(ref rec_model) = ocr_rec_task {
                                        let rec_h = 48; let rec_w = 320;
                                        let resized_rec = image::imageops::resize(&img, rec_w, rec_h, image::imageops::FilterType::Triangle);
                                        let mut input_rec = Array4::<f32>::zeros((1, 3, rec_h as usize, rec_w as usize));
                                        for (x, y, pixel) in resized_rec.enumerate_pixels() {
                                            input_rec[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5;
                                            input_rec[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5;
                                            input_rec[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5;
                                        }
                                        if let Ok(rec_logits) = rec_model.run(input_rec, "x") {
                                            if !ocr_alphabet_task.is_empty() {
                                                let seq_len = rec_logits.len() / ocr_alphabet_task.len();
                                                let num_classes = ocr_alphabet_task.len();
                                                let mut recognized_text = String::new();
                                                let mut last_char_idx = 0;
                                                for i in 0..seq_len {
                                                    let chunk = &rec_logits[i * num_classes..(i + 1) * num_classes];
                                                    let mut max_val = -f32::INFINITY; let mut max_idx = 0;
                                                    for (idx, &val) in chunk.iter().enumerate() { if val > max_val { max_val = val; max_idx = idx; } }
                                                    if max_idx != 0 && max_idx != last_char_idx { if let Some(c) = ocr_alphabet_task.get(max_idx) { recognized_text.push_str(c); } }
                                                    last_char_idx = max_idx;
                                                }
                                                if !recognized_text.trim().is_empty() {
                                                    let lock = db_task.lock().unwrap();
                                                    let _ = lock.connection.execute("INSERT INTO ocr (photo_id, text) VALUES(?1, ?2)", (&photo_id_task, &recognized_text));
                                                    emit_log(&app_handle_task, format!("ML Worker: OCR text for {photo_id_task}: {}", recognized_text.trim()));
                                                } else {
                                                    emit_log(&app_handle_task, format!("ML Worker: OCR found no text for {photo_id_task}."));
                                                }
                                            }
                                        }
                                        let lock = db_task.lock().unwrap();
                                        lock.update_ai_status(&photo_id_task, "ocr", 1);
                                        model_timings.insert("ocr".to_string(), __start.elapsed().as_secs_f64());
                                    }
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: OCR model is not loaded. Download or update Text Finder first.".to_string());
                                }
                            }

                            // Tier 2: YOLOv8
                            if should_run_model(target_model_inner.as_deref(), "yolo", Some(&config)) && photo_entry.ai_status.yolo == 0 {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref model) = yolo_task {
                                    let resized = image::imageops::resize(&img, 640, 640, image::imageops::FilterType::Triangle);
                                    let mut input = Array4::<f32>::zeros((1, 3, 640, 640));
                                    for (x, y, pixel) in resized.enumerate_pixels() {
                                        input[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
                                        input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
                                        input[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
                                    }
                                    if let Ok(data) = model.run(input, "images") {
                                        // YOLOv8n output is [1, 84, 8400]
                                        // We just check for high-confidence classes to tag
                                        let num_classes = 80;
                                        let num_anchors = 8400;
                                        let mut found_classes: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
                                        for a in 0..num_anchors {
                                            let mut max_conf = 0.0f32;
                                            let mut max_cls = 0;
                                            for c in 0..num_classes {
                                                let conf = data[num_anchors * (4 + c) + a];
                                                if conf > max_conf { max_conf = conf; max_cls = c; }
                                            }
                                            if max_conf > 0.6 {
                                                found_classes.entry(max_cls).and_modify(|e| *e = e.max(max_conf)).or_insert(max_conf);
                                            }
                                        }
                                        const COCO_CLASSES: &[&str] = &["person","bicycle","car","motorcycle","airplane","bus","train","truck","boat","traffic light","fire hydrant","stop sign","parking meter","bench","bird","cat","dog","horse","sheep","cow","elephant","bear","zebra","giraffe","backpack","umbrella","handbag","tie","suitcase","frisbee","skis","snowboard","sports ball","kite","baseball bat","baseball glove","skateboard","surfboard","tennis racket","bottle","wine glass","cup","fork","knife","spoon","bowl","banana","apple","sandwich","orange","broccoli","carrot","hot dog","pizza","donut","cake","chair","couch","potted plant","bed","dining table","toilet","tv","laptop","mouse","remote","keyboard","cell phone","microwave","oven","toaster","sink","refrigerator","book","clock","vase","scissors","teddy bear","hair drier","toothbrush"];
                                        let lock = db_task.lock().unwrap();
                                        let found_count = found_classes.len();
                                        for (cls_idx, conf) in found_classes {
                                            let name = COCO_CLASSES.get(cls_idx).copied().unwrap_or("unknown");
                                            let _ = lock.connection.execute("INSERT INTO object (photo_id, class, probability) VALUES(?1, ?2, ?3)", (&photo_id_task, name, &conf.to_string()));
                                        }
                                        lock.update_ai_status(&photo_id_task, "yolo", 1);
                                        model_timings.insert("yolo".to_string(), __start.elapsed().as_secs_f64());
                                        emit_log(&app_handle_task, format!("ML Worker: YOLO found {found_count} object classes for {photo_id_task}."));
                                    }
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: YOLO model is not loaded. Download or update Object Pro first.".to_string());
                                }
                            }

                            // Tier 1: Face Detection
                            let should_run_face = should_run_model(target_model_inner.as_deref(), "face", Some(&config))
                                && photo_entry.ai_status.face == 0;
                            let should_run_arcface = should_run_model(target_model_inner.as_deref(), "arcface", Some(&config))
                                && photo_entry.ai_status.arcface == 0;
                            if should_run_face || should_run_arcface {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref face_model) = face_detector_task {
                                    let mut face_count = 0usize;
                                    let (orig_w, orig_h) = (img.width() as f32, img.height() as f32);
                                    let resized = image::imageops::resize(&img, 320, 240, image::imageops::FilterType::Triangle);
                                    let mut input = Array4::<f32>::zeros((1, 3, 240, 320));
                                    for (x, y, pixel) in resized.enumerate_pixels() {
                                        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 - 127.0) / 128.0;
                                        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 - 127.0) / 128.0;
                                        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 - 127.0) / 128.0;
                                    }
                                    if let Ok(data) = face_model.run(input, "input") {
                                        if data.len() >= 4420 * 6 {
                                            let scores = &data[..4420 * 2]; let boxes = &data[4420 * 2..];
                                            let anchors = crate::face_detector::generate_anchors();
                                            let mut proposals = Vec::new();
                                            for i in 0..anchors.len() {
                                                let score = scores[i * 2 + 1];
                                                if score > 0.6 {
                                                    let loc = [boxes[i * 4], boxes[i * 4 + 1], boxes[i * 4 + 2], boxes[i * 4 + 3]];
                                                    let decoded = crate::face_detector::decode(&loc, &anchors[i]);
                                                    proposals.push((decoded, score));
                                                }
                                            }
                                            let keep = crate::face_detector::nms(&mut proposals, 0.3);
                                            for &idx in &keep {
                                                let bbox = proposals[idx].0;
                                                let xmin = (bbox[0] * orig_w).max(0.0) as u32; let ymin = (bbox[1] * orig_h).max(0.0) as u32;
                                                let xmax = (bbox[2] * orig_w).min(orig_w) as u32; let ymax = (bbox[3] * orig_h).min(orig_h) as u32;
                                                if xmax > xmin && ymax > ymin {
                                                    let (w, h) = (xmax - xmin, ymax - ymin);
                                                    if w > 20 && h > 20 {
                                                        let face_crop = image::imageops::crop_imm(&img, xmin, ymin, w, h).to_image();
                                                        let face_id = format!("{photo_id_task}_face_{xmin}_{ymin}");
                                                        let crop_path = format!("{faces_dir_task}/{face_id}.jpg");
                                                        if face_crop.save(&crop_path).is_ok() {
                                                            let mut face_embedding = Vec::new();

                                                            // Use ArcFace if available (higher accuracy)
                                                            if let Some(ref model) = arcface_task {
                                                                let f_resized = image::imageops::resize(&face_crop, 112, 112, image::imageops::FilterType::Triangle);
                                                                let mut f_input = Array4::<f32>::zeros((1, 3, 112, 112));
                                                                for (x, y, pixel) in f_resized.enumerate_pixels() {
                                                                    f_input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 - 127.5) / 128.0;
                                                                    f_input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 - 127.5) / 128.0;
                                                                    f_input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 - 127.5) / 128.0;
                                                                }
                                                                if let Ok(emb) = model.run(f_input, "input.1") {
                                                                    let mut e = emb;
                                                                    let norm: f32 = e.iter().map(|x| x * x).sum::<f32>().sqrt();
                                                                    if norm > 0.0 { for v in e.iter_mut() { *v /= norm; } }
                                                                    face_embedding = e;
                                                                }
                                                            } else if let Some(ref visual_model) = clip_visual_task {
                                                                let face_resized = image::imageops::resize(&face_crop, 224, 224, image::imageops::FilterType::Triangle);
                                                                let mut face_input = Array4::<f32>::zeros((1, 3, 224, 224));
                                                                for (x, y, pixel) in face_resized.enumerate_pixels() {
                                                                    face_input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.48145466) / 0.26862954;
                                                                    face_input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.4578275) / 0.2613026;
                                                                    face_input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.40821073) / 0.2757771;
                                                                }
                                                                if let Ok(emb) = visual_model.run(face_input, "pixel_values") {
                                                                    let mut e = emb; let norm: f32 = e.iter().map(|x| x * x).sum::<f32>().sqrt();
                                                                    if norm > 0.0 { for v in e.iter_mut() { *v /= norm; } }
                                                                    face_embedding = e;
                                                                }
                                                            }
                                                            let mut assigned_person_id = None;
                                                            if !face_embedding.is_empty() {
                                                                if let Ok(mut lock) = known_people_task.lock() {
                                                                    let mut highest_similarity = 0.0f32; let mut best_match_id = None;
                                                                    for (person_id, person_centroid) in lock.iter() {
                                                                        let dot_product: f32 = face_embedding.iter().zip(person_centroid.iter()).map(|(a, b)| a * b).sum();
                                                                        if dot_product > highest_similarity { highest_similarity = dot_product; best_match_id = Some(person_id.clone()); }
                                                                    }
                                                                    if highest_similarity > 0.75 { assigned_person_id = best_match_id; }
                                                                    else {
                                                                        let lock_db = db_task.lock().unwrap();
                                                                        let new_id = lock_db.create_anonymous_person(&face_embedding);
                                                                        lock.push((new_id.clone(), face_embedding.clone()));
                                                                        assigned_person_id = Some(new_id);
                                                                    }
                                                                }
                                                            } else {
                                                                let lock_db = db_task.lock().unwrap();
                                                                let new_id = lock_db.create_anonymous_person(&[]);
                                                                assigned_person_id = Some(new_id);
                                                            }
                                                            let mut buffer = std::io::Cursor::new(Vec::new());
                                                            let _ = face_crop.write_to(&mut buffer, image::ImageOutputFormat::Jpeg(80));
                                                            let encoded = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(buffer.get_ref()));
                                                            let lock = db_task.lock().unwrap();
                                                            lock.store_face(Face { photo_id: photo_id_task.clone(), face_id: face_id.clone(), crop_path, encoded, embedding: face_embedding, person_id: assigned_person_id });
                                                            face_count += 1;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    let lock = db_task.lock().unwrap();
                                    let _ = lock.connection.execute("INSERT OR REPLACE INTO properties (photo_id, key, value) VALUES(?1, 'face_count', ?2)", (&photo_id_task, &face_count.to_string()));
                                    lock.update_ai_status(&photo_id_task, "face", 1);
                                    if arcface_task.is_some() {
                                        lock.update_ai_status(&photo_id_task, "arcface", 1);
                                    }
                                    model_timings.insert("face".to_string(), __start.elapsed().as_secs_f64());
                                    emit_log(&app_handle_task, format!("ML Worker: Face analysis found {face_count} faces for {photo_id_task}."));
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: Face model is not loaded. Download or update Face Grouping first.".to_string());
                                }
                            }

                            // Tier 3: BLIP (Captioning)
                            if should_run_model(target_model_inner.as_deref(), "blip", Some(&config)) && photo_entry.ai_status.blip == 0 {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref model) = blip_task {
                                    let resized = image::imageops::resize(&img, 384, 384, image::imageops::FilterType::Triangle);
                                    let mut input = Array4::<f32>::zeros((1, 3, 384, 384));
                                    for (x, y, pixel) in resized.enumerate_pixels() {
                                        input[[0, 0, y as usize, x as usize]] = (pixel[0] as f32 / 255.0 - 0.48145466) / 0.26862954;
                                        input[[0, 1, y as usize, x as usize]] = (pixel[1] as f32 / 255.0 - 0.4578275) / 0.2613026;
                                        input[[0, 2, y as usize, x as usize]] = (pixel[2] as f32 / 255.0 - 0.40821073) / 0.2757771;
                                    }
                                    if let Ok(_data) = model.run(input, "pixel_values") {
                                        let lock = db_task.lock().unwrap();
                                        lock.update_ai_status(&photo_id_task, "blip", 1);
                                        model_timings.insert("blip".to_string(), __start.elapsed().as_secs_f64());
                                        emit_log(&app_handle_task, format!("ML Worker: BLIP caption model ran for {photo_id_task}."));
                                    }
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: BLIP model is not loaded. Download or update Photo Describer first.".to_string());
                                }
                            }

                            // Tier 3: MiDaS (Depth)
                            if should_run_model(target_model_inner.as_deref(), "midas", Some(&config)) && photo_entry.ai_status.midas == 0 {
                                if abort_task.load(Ordering::SeqCst) { return; }
                                let __start = std::time::Instant::now();
                                if let Some(ref model) = midas_task {
                                    let resized = image::imageops::resize(&img, 256, 256, image::imageops::FilterType::Triangle);
                                    let mut input = Array4::<f32>::zeros((1, 3, 256, 256));
                                    for (x, y, pixel) in resized.enumerate_pixels() {
                                        input[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
                                        input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
                                        input[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
                                    }
                                    if let Ok(_data) = model.run(input, "pixel_values") {
                                        let lock = db_task.lock().unwrap();
                                        lock.update_ai_status(&photo_id_task, "midas", 1);
                                        model_timings.insert("midas".to_string(), __start.elapsed().as_secs_f64());
                                        emit_log(&app_handle_task, format!("ML Worker: MiDaS depth analysis completed for {photo_id_task}."));
                                    }
                                } else if progress_model_inner.is_some() {
                                    emit_log(&app_handle_task, "ERROR: MiDaS model is not loaded. Download or update Depth Vision first.".to_string());
                                }
                            }
                        } else {
                            emit_log(&app_handle_task, format!("ERROR: Could not open image for AI analysis: {photo_loc_actual}"));
                        }
                        }

                        // Finalize
                        let lock = db_task.lock().unwrap();
                        if target_model_inner.is_none() {
                            lock.update_photo_indexed(&photo_id_task, 2);
                        }
                        let _ = lock.connection.execute("UPDATE photo SET sync_needed = 1 WHERE id = ?1", [&photo_id_task]);

                        if let Some(state) = app_handle_task.try_state::<crate::WebRtcState>() {
                            let mut tx_lock = state.sync_tx.blocking_lock();
                            if let Some(tx) = tx_lock.as_mut() {
                                if let Ok(info) = lock.get_photo_sync_info_by_id(&photo_id_task) {
                                    let _ = tx.send(crate::transport::SyncMessage::SyncFile { photo: info });
                                }
                            }
                        }

                        let _ = app_handle_task.emit("photo-updated", serde_json::json!({
                            "id": photo_id_task,
                        }));

                        {
                            let lock = db_task.lock().unwrap();
                            let object_count: i32 = lock.connection
                                .query_row("SELECT COUNT(*) FROM object WHERE photo_id = ?1", [&photo_id_task], |r| r.get(0))
                                .unwrap_or(0);
                            let face_count: i32 = lock.connection
                                .query_row("SELECT COUNT(*) FROM faces WHERE photo_id = ?1", [&photo_id_task], |r| r.get(0))
                                .unwrap_or(0);
                            let has_caption: bool = lock.connection
                                .query_row("SELECT caption FROM photo WHERE id = ?1", [&photo_id_task], |r| r.get::<_, Option<String>>(0))
                                .unwrap_or(None)
                                .is_some();
                            eprintln!("[ml] emitting photo-analysis-result for {}", photo_id_task);
                            let _ = app_handle_task.emit("photo-analysis-result", serde_json::json!({
                                "id": photo_id_task,
                                "object_count": object_count,
                                "face_count": face_count,
                                "has_caption": has_caption,
                                "indexed": true,
                                "model_timings": model_timings,
                            }));
                            eprintln!("[siegu-bench] photo={photo_id_task} timings={:?}", model_timings);
                        }

                        let remaining = decrement_pending_count(&pending_count_task);
                        let _ = app_handle_task.emit("indexing-progress", remaining);
                        let _ = app_handle_task.emit("indexing-eta", (remaining as f64) * avg_photo_time_ms);

                        if remaining == 0 {
                            let _ = app_handle_task.emit("scan-progress", serde_json::json!({
                                "status": "complete",
                                "progress": 100,
                            }));
                        }

                        if let Some(ref model) = progress_model_inner {
                            let _ = app_handle_task.emit("model-progress", serde_json::json!({
                                "model": model,
                                "pending": remaining,
                                "status": if remaining == 0 { "completed" } else { "running" },
                            }));
                        }
                    });
                }
                avg_photo_time_ms =
                    (avg_photo_time_ms * 0.9) + (start_time.elapsed().as_millis() as f64 * 0.1);
            }

            if is_single {
                abort_clone.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
    });
    (tx, pending_count, abort)
}
