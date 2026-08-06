//! Model loading and management for all ML pipelines.
//!
//! Each ONNX model is wrapped in a pooled [`SessionPool`] (see [`ModelEngine`])
//! so several concurrent inference runs can proceed in parallel. Models are
//! conditionally loaded based on user config flags (`model_enabled_<name>` in
//! the app config). Missing models are silently skipped (returning `None`) so
//! the app degrades gracefully.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use ndarray::Array2;
use ort::session::Session;

/// Thread-safe handle to a pool of ORT inference sessions.
///
/// `ort::Session::run` requires `&mut self`, so a single shared session would
/// serialize all inference. Library indexing analyzes many photos in parallel
/// (the rayon `scan_threads` jobs); pooling a few sessions lets those runs
/// overlap instead of queueing on one global mutex.
pub type ModelEngine = Arc<SessionPool>;

/// Pool of interchangeable ONNX sessions for one model.
///
/// All sessions run the same graph, so a caller takes whichever slot is free.
/// A slot is released back to the pool when its guard is dropped. Each slot is
/// wrapped in its own `Mutex` to satisfy `Session::run(&mut self)`; the
/// free-list hands out an index to at most one caller at a time, so the slot
/// mutex never actually contends.
pub struct SessionPool {
    sessions: Vec<Mutex<Session>>,
    free: Mutex<Vec<usize>>,
    condvar: Condvar,
}

/// Exclusive access to one pooled session, returned by [`SessionPool::lock`].
/// Derefs to `&mut Session`; hands its slot back to the pool on drop.
pub struct SessionGuard<'a> {
    pool: &'a SessionPool,
    guard: MutexGuard<'a, Session>,
    idx: usize,
}

impl SessionPool {
    /// Builds a pool from the given sessions. The pool serves at most one run
    /// at a time per session (i.e., `sessions.len()` concurrent runs).
    pub fn new(sessions: Vec<Session>) -> Self {
        let sessions: Vec<Mutex<Session>> = sessions.into_iter().map(Mutex::new).collect();
        let free = Mutex::new((0..sessions.len()).collect());
        Self {
            sessions,
            free,
            condvar: Condvar::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Blocks until a session is free, then hands out exclusive access.
    pub fn lock(&self) -> Result<SessionGuard<'_>, String> {
        let mut free = self.free.lock().map_err(|e| e.to_string())?;
        if free.is_empty() && self.sessions.is_empty() {
            return Err("session pool is empty".to_string());
        }
        while free.is_empty() {
            free = self.condvar.wait(free).map_err(|e| e.to_string())?;
        }
        let idx = free
            .pop()
            .ok_or_else(|| "session pool free-list is empty".to_string())?;
        let guard = self.sessions[idx].lock().map_err(|e| e.to_string())?;
        Ok(SessionGuard {
            pool: self,
            guard,
            idx,
        })
    }

    /// Number of sessions to build for a model.
    ///
    /// Defaults to 2 so concurrent library-indexing jobs can run inference in
    /// parallel. Heavy models (BLIP captioning, Whisper transcription, the
    /// 1.6 GB aesthetics model) stay single by default to bound memory, since
    /// each extra session duplicates the model's weights. `SIEGU_ORT_POOL`
    /// overrides the default for every model.
    fn pool_size_for(filename: &str) -> usize {
        let env_pool = std::env::var("SIEGU_ORT_POOL")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n >= 1);
        match env_pool {
            Some(n) => n.min(8),
            None => {
                if filename.contains("whisper")
                    || filename.starts_with("blip")
                    || filename.starts_with("aesthetics")
                {
                    1
                } else {
                    2
                }
            }
        }
    }
}

impl Deref for SessionGuard<'_> {
    type Target = Session;

    fn deref(&self) -> &Session {
        &self.guard
    }
}

impl DerefMut for SessionGuard<'_> {
    fn deref_mut(&mut self) -> &mut Session {
        &mut self.guard
    }
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut free) = self.pool.free.lock() {
            free.push(self.idx);
            self.pool.condvar.notify_one();
        }
    }
}

/// Container for all loaded ML models and their associated data.
///
/// Each field is `Option<ModelEngine>` — `None` means the model was
/// disabled in config, not found on disk, or failed to load.
#[derive(Clone)]
pub struct LoadedModels {
    pub clip_visual: Option<ModelEngine>,
    pub clip_text: Option<ModelEngine>,
    pub text_embeddings: Vec<(String, Vec<f32>)>,
    pub face_detector: Option<ModelEngine>,
    pub arcface: Option<ModelEngine>,
    pub ocr_det: Option<ModelEngine>,
    pub ocr_rec: Option<ModelEngine>,
    pub ocr_alphabet: Vec<String>,
    pub nsfw: Option<ModelEngine>,
    pub aesthetics: Option<ModelEngine>,
    pub yolo: Option<ModelEngine>,
    pub blip: Option<ModelEngine>,
    pub blip_decoder: Option<ModelEngine>,
    pub blip_tokenizer: Option<tokenizers::Tokenizer>,
    pub midas: Option<ModelEngine>,
    pub whisper_encoder: Option<ModelEngine>,
    pub whisper_decoder: Option<ModelEngine>,
    pub whisper_tokenizer: Option<tokenizers::Tokenizer>,
    pub known_people: Vec<(String, Vec<f32>)>,
    pub selected_ep: String,
}

impl LoadedModels {
    /// Whether the given model's engines are actually in memory. Used to
    /// distinguish "disabled / dropped / not downloaded" (intentionally absent)
    /// from "failed to build a session" (broken on this device).
    pub fn engine_loaded(&self, name: &str) -> bool {
        match name {
            "clip" => self.clip_visual.is_some() && self.clip_text.is_some(),
            "face" => self.face_detector.is_some() && self.arcface.is_some(),
            "ocr" => self.ocr_det.is_some() && self.ocr_rec.is_some(),
            "nsfw" => self.nsfw.is_some(),
            "aesthetics" => self.aesthetics.is_some(),
            "yolo" => self.yolo.is_some(),
            "blip" => self.blip.is_some() && self.blip_decoder.is_some(),
            "midas" => self.midas.is_some(),
            "whisper" => self.whisper_encoder.is_some() && self.whisper_decoder.is_some(),
            _ => false,
        }
    }
}

/// Checks whether a model is enabled in the user's config.
/// Config keys follow the pattern `model_enabled_<name>` (e.g., `model_enabled_clip`).
///
/// A missing key means the model is enabled by default, mirroring
/// `should_run_model` and the app's UI toggle state (missing key => enabled).
fn model_enabled(config: &HashMap<String, String>, name: &str) -> bool {
    config
        .get(&format!("model_enabled_{name}"))
        .is_none_or(|v| v == "true")
}

/// Estimated resident size of each loadable model (sum of its registry
/// files), used to enforce the `ml_memory_budget_mb` cap.
const MODEL_SIZES: &[(&str, u64)] = &[
    ("clip", 351_685_709 + 254_058_553 + 2_224_119),
    ("face", 232_589),
    ("arcface", 174_383_860),
    ("ocr", 2_423_224 + 8_967_018 + 190),
    ("nsfw", 343_401_688),
    ("aesthetics", 1_718_811_155),
    ("yolo", 12_823_574),
    ("blip", 345_122_738 + 647_427_238 + 711_396),
    ("midas", 533_061_339),
    ("whisper", 32_883_618 + 118_505_132),
];

/// Whether a model should be loaded: enabled in config and not dropped by the
/// memory budget.
fn should_load(config: &HashMap<String, String>, dropped: &[&str], name: &str) -> bool {
    model_enabled(config, name) && !dropped.contains(&name)
}

/// Applies a byte-based RAM cap: returns the names of enabled models dropped
/// (heaviest first) so the total estimated size fits the cap. Returns an empty
/// list when the cap is `None` (no limit).
fn dropped_over_cap_bytes(
    config: &HashMap<String, String>,
    cap: Option<u64>,
    log: &dyn Fn(&str),
) -> Vec<&'static str> {
    let Some(budget) = cap else {
        return Vec::new();
    };

    let mut enabled: Vec<(u64, &'static str)> = MODEL_SIZES
        .iter()
        .filter(|(name, _)| model_enabled(config, name))
        .map(|(name, size)| (*size, *name))
        .collect();
    enabled.sort_by_key(|(size, _)| std::cmp::Reverse(*size));

    let mut total: u64 = enabled.iter().map(|(size, _)| *size).sum();
    let mut dropped: Vec<&'static str> = Vec::new();
    for (size, name) in &enabled {
        if total <= budget {
            break;
        }
        dropped.push(name);
        total = total.saturating_sub(*size);
    }

    if !dropped.is_empty() {
        log(&format!("Memory budget: skipped {}", dropped.join(", ")));
    }
    dropped
}

/// Applies the `ml_memory_budget_mb` cap: returns the names of enabled models
/// dropped (heaviest first) so the total estimated size fits the budget.
/// Returns an empty list when the budget is unset.
fn dropped_over_budget(config: &HashMap<String, String>, log: &dyn Fn(&str)) -> Vec<&'static str> {
    let budget_mb = config
        .get("ml_memory_budget_mb")
        .and_then(|s| s.parse::<u64>().ok());
    dropped_over_cap_bytes(
        config,
        budget_mb.map(|mb| mb.saturating_mul(1024 * 1024)),
        log,
    )
}

/// Machine-readable reasons a model can't run on this device. These codes are
/// emitted as-is so the frontend can map them to localized strings.
pub const REASON_MEMORY_BUDGET: &str = "memory_budget";
pub const REASON_LOW_RAM: &str = "low_ram";
pub const REASON_NOT_DOWNLOADED: &str = "not_downloaded";
pub const REASON_LOAD_FAILED: &str = "load_failed";

/// Feasibility verdict for one user-facing model.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelFeasibility {
    pub model: String,
    /// Whether the model can actually run on this device right now.
    pub runnable: bool,
    /// `None` when runnable; otherwise one of the `REASON_*` codes.
    pub reason: Option<String>,
}

/// Fraction of physical RAM the model weights may occupy. The rest is reserved
/// for the OS and app. Tuned so a 4 GB phone keeps ~2 GB for models, which is
/// realistic on low-end Android devices.
const RAM_USABLE_FRACTION: f64 = 0.5;

/// User-facing models checked for feasibility. Mirrors the app's model toggles
/// (the `face` toggle also gates the separate `arcface` model).
pub const FEASIBILITY_MODELS: &[&str] = &[
    "clip",
    "face",
    "ocr",
    "nsfw",
    "aesthetics",
    "yolo",
    "blip",
    "midas",
    "whisper",
];

/// Required files per model, used to decide whether a model is downloaded.
/// Matches the files each pipeline reads; an empty/absent file counts as
/// missing (e.g. an interrupted download).
fn model_files_ok(models_dir: &Path, name: &str) -> bool {
    let required: &[&str] = match name {
        "clip" => &[
            "clip-vit-base-patch32-visual.onnx",
            "clip-vit-base-patch32-text.onnx",
            "tokenizer.json",
        ],
        "face" => &["face_detection_yunet_2023mar.onnx", "arcface.onnx"],
        "ocr" => &["ocr_det.onnx", "ocr_rec.onnx", "en_dict.txt"],
        "nsfw" => &["nsfw.onnx"],
        "aesthetics" => &["aesthetics.onnx"],
        "yolo" => &["yolov8.onnx"],
        "blip" => &["blip.onnx", "blip_decoder.onnx", "blip_tokenizer.json"],
        "midas" => &["midas.onnx"],
        "whisper" => &[
            "whisper.onnx",
            "whisper-decoder.onnx",
            "whisper-tokenizer.json",
        ],
        _ => return false,
    };
    required.iter().all(|f| {
        let p = models_dir.join(f);
        p.exists() && p.metadata().map(|m| m.len()).unwrap_or(0) > 0
    })
}

/// Feasibility with an explicit device-RAM value (injectable for tests).
///
/// A model is reported as runnable only when it could actually load AND run
/// here: enabled, downloaded, not dropped by the user's memory budget, and not
/// over the device's physical RAM (heaviest enabled models are dropped first,
/// exactly like [`dropped_over_budget`]).
pub fn model_feasibility_with_ram(
    models_dir: &Path,
    config: &HashMap<String, String>,
    device_ram: Option<u64>,
    log: &dyn Fn(&str),
) -> Vec<ModelFeasibility> {
    let user_budget = config
        .get("ml_memory_budget_mb")
        .and_then(|s| s.parse::<u64>().ok())
        .map(|mb| mb.saturating_mul(1024 * 1024));
    let ram_cap = device_ram.map(|ram| (ram as f64 * RAM_USABLE_FRACTION) as u64);

    let mut out = Vec::new();
    for &name in FEASIBILITY_MODELS {
        if !model_enabled(config, name) {
            continue;
        }
        if !model_files_ok(models_dir, name) {
            out.push(ModelFeasibility {
                model: name.to_string(),
                runnable: false,
                reason: Some(REASON_NOT_DOWNLOADED.to_string()),
            });
            continue;
        }
        let cap = match (user_budget, ram_cap) {
            (Some(b), Some(r)) => Some(b.min(r)),
            (Some(b), None) => Some(b),
            (None, Some(r)) => Some(r),
            (None, None) => None,
        };
        let dropped = dropped_over_cap_bytes(config, cap, log);
        // `face` also depends on the separately-sized `arcface` engine.
        let is_dropped =
            dropped.contains(&name) || (name == "face" && dropped.contains(&"arcface"));
        if !is_dropped {
            out.push(ModelFeasibility {
                model: name.to_string(),
                runnable: true,
                reason: None,
            });
            continue;
        }
        // The binding cap is the smaller one: that's what actually caused the
        // drop, and it decides whether the fix is "raise the memory limit" or
        // "this device can't run it".
        let reason = match (user_budget, ram_cap) {
            (Some(b), Some(r)) if b > r => REASON_LOW_RAM,
            (Some(_), _) => REASON_MEMORY_BUDGET,
            (None, Some(_)) => REASON_LOW_RAM,
            (None, None) => unreachable!("cap is None, nothing can be dropped"),
        };
        out.push(ModelFeasibility {
            model: name.to_string(),
            runnable: false,
            reason: Some(reason.to_string()),
        });
    }
    out
}

/// Feasibility verdicts for every enabled model on this device.
pub fn model_feasibility(
    models_dir: &Path,
    config: &HashMap<String, String>,
    log: &dyn Fn(&str),
) -> Vec<ModelFeasibility> {
    model_feasibility_with_ram(
        models_dir,
        config,
        crate::model_manager::physical_memory_bytes(),
        log,
    )
}

/// Loads all enabled ONNX models from the models directory.
///
/// Models are loaded conditionally based on config flags. Each model is
/// validated to be >1MB (to reject corrupt/truncated files) before loading.
/// The `log` callback is used to report loading progress to the UI.
///
/// Returns a `LoadedModels` struct where each field is `Some` if the model
/// loaded successfully, or `None` if it was disabled/missing/failed.
pub fn load_models(
    config_path: &str,
    config: &HashMap<String, String>,
    known_people: Vec<(String, Vec<f32>)>,
    log: &dyn Fn(&str),
) -> LoadedModels {
    let models_dir = Path::new(config_path).join("models");

    let ml_threads: Option<usize> = config
        .get("ml_threads")
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| (1..=32).contains(&n));

    let dropped = dropped_over_budget(config, log);

    let clip_visual = if should_load(config, &dropped, "clip") {
        log("Loading CLIP visual model...");
        let m = load_model(&models_dir, "clip-vit-base-patch32-visual.onnx", ml_threads);
        log("CLIP visual ready.");
        m
    } else {
        None
    };
    let face_detector =
        if should_load(config, &dropped, "face") || model_enabled(config, "ultraface") {
            log("Loading face detector...");
            let m = load_model_with_min_size(
                &models_dir,
                "face_detection_yunet_2023mar.onnx",
                100 * 1024,
                ml_threads,
            );
            log("Face detector ready.");
            m
        } else {
            None
        };
    let arcface = if should_load(config, &dropped, "arcface") {
        log("Loading ArcFace model...");
        let m = load_model(&models_dir, "arcface.onnx", ml_threads);
        log("ArcFace ready.");
        m
    } else {
        None
    };
    let ocr_det = if should_load(config, &dropped, "ocr") {
        log("Loading OCR detection model...");
        let m = load_model(&models_dir, "ocr_det.onnx", ml_threads);
        log("OCR detection ready.");
        m
    } else {
        None
    };
    let ocr_rec = if should_load(config, &dropped, "ocr") {
        log("Loading OCR recognition model...");
        let m = load_model(&models_dir, "ocr_rec.onnx", ml_threads);
        log("OCR recognition ready.");
        m
    } else {
        None
    };
    let nsfw = if should_load(config, &dropped, "nsfw") {
        log("Loading NSFW model...");
        let m = load_model(&models_dir, "nsfw.onnx", ml_threads);
        log("NSFW ready.");
        m
    } else {
        None
    };
    let aesthetics = if should_load(config, &dropped, "aesthetics") {
        log("Loading aesthetics model (1.6 GB)...");
        let m = load_model(&models_dir, "aesthetics.onnx", ml_threads);
        log("Aesthetics ready.");
        m
    } else {
        None
    };
    let yolo = if should_load(config, &dropped, "yolo") {
        log("Loading YOLO model...");
        let m = load_model(&models_dir, "yolov8.onnx", ml_threads);
        log("YOLO ready.");
        m
    } else {
        None
    };
    let blip = if should_load(config, &dropped, "blip") {
        log("Loading BLIP vision encoder...");
        let m = load_model(&models_dir, "blip.onnx", ml_threads);
        log("BLIP vision encoder ready.");
        m
    } else {
        None
    };
    let blip_decoder = if should_load(config, &dropped, "blip") {
        log("Loading BLIP text decoder...");
        let m = load_model(&models_dir, "blip_decoder.onnx", ml_threads);
        log("BLIP text decoder ready.");
        m
    } else {
        None
    };
    let blip_tokenizer = if should_load(config, &dropped, "blip") {
        let tok_path = models_dir.join("blip_tokenizer.json");
        if tok_path.exists() {
            tokenizers::Tokenizer::from_file(&tok_path).ok()
        } else {
            // fall back to shared tokenizer.json
            let fallback = models_dir.join("tokenizer.json");
            if fallback.exists() {
                tokenizers::Tokenizer::from_file(&fallback).ok()
            } else {
                None
            }
        }
    } else {
        None
    };
    let midas = if should_load(config, &dropped, "midas") {
        log("Loading MiDaS depth model...");
        let m = load_model(&models_dir, "midas.onnx", ml_threads);
        log("MiDaS ready.");
        m
    } else {
        None
    };
    let (whisper_encoder, whisper_decoder, whisper_tokenizer) =
        if should_load(config, &dropped, "whisper") {
            log("Loading Whisper encoder...");
            let enc = load_model(&models_dir, "whisper.onnx", ml_threads);
            log("Whisper encoder ready.");
            log("Loading Whisper decoder...");
            let dec = load_model(&models_dir, "whisper-decoder.onnx", ml_threads);
            log("Whisper decoder ready.");
            let tok_path = models_dir.join("whisper-tokenizer.json");
            let tok = if tok_path.exists() {
                tokenizers::Tokenizer::from_file(&tok_path).ok()
            } else {
                None
            };
            (enc, dec, tok)
        } else {
            (None, None, None)
        };

    let clip_text_path = models_dir.join("clip-vit-base-patch32-text.onnx");
    let tokenizer_path = models_dir.join("tokenizer.json");
    let ocr_dict_path = models_dir.join("en_dict.txt");

    let mut text_embeddings = Vec::new();
    let clip_text = if should_load(config, &dropped, "clip") && clip_text_path.exists() {
        if let Ok(tokenizer) = tokenizers::Tokenizer::from_file(&tokenizer_path) {
            log("Loading CLIP text model & computing embeddings...");
            if let Some(mut text_model) =
                load_model(&models_dir, "clip-vit-base-patch32-text.onnx", ml_threads)
            {
                text_embeddings = compute_text_embeddings(&mut text_model, &tokenizer);
                log(&format!(
                    "CLIP text ready ({} embeddings).",
                    text_embeddings.len()
                ));
                Some(text_model)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let ocr_alphabet = if model_enabled(config, "ocr") && ocr_dict_path.exists() {
        let dict = std::fs::read_to_string(&ocr_dict_path).unwrap_or_default();
        let mut alphabet = vec!["blank".to_string()];
        alphabet.extend(dict.lines().map(|s| s.to_string()));
        alphabet.push(" ".to_string());
        alphabet
    } else {
        Vec::new()
    };

    let selected_ep = super::ep::selected_ep();

    LoadedModels {
        clip_visual,
        clip_text,
        text_embeddings,
        face_detector,
        arcface,
        ocr_det,
        ocr_rec,
        ocr_alphabet,
        nsfw,
        aesthetics,
        yolo,
        blip,
        blip_decoder,
        blip_tokenizer,
        midas,
        whisper_encoder,
        whisper_decoder,
        whisper_tokenizer,
        known_people,
        selected_ep,
    }
}

/// Loads a single ONNX model, returning `None` if the file doesn't exist,
/// is too small (<1MB, likely corrupt), or fails to build an ORT session.
fn load_model(models_dir: &Path, filename: &str, ml_threads: Option<usize>) -> Option<ModelEngine> {
    load_model_with_min_size(models_dir, filename, 1024 * 1024, ml_threads)
}

/// Like [`load_model`] but with an explicit minimum file size, used for
/// small models such as YuNet (~232KB) that are below the 1MB default gate.
fn load_model_with_min_size(
    models_dir: &Path,
    filename: &str,
    min_size: u64,
    ml_threads: Option<usize>,
) -> Option<ModelEngine> {
    let path = models_dir.join(filename);
    let is_ok = path.exists() && path.metadata().map(|m| m.len()).unwrap_or(0) > min_size;
    if !is_ok {
        return None;
    }
    let pool_size = SessionPool::pool_size_for(filename);
    let mut sessions = Vec::with_capacity(pool_size);
    for _ in 0..pool_size {
        if let Ok(session) = super::ep::build_session(&path, ml_threads) {
            sessions.push(session);
        }
    }
    if sessions.is_empty() {
        return None;
    }
    Some(Arc::new(SessionPool::new(sessions)))
}

/// Pre-computes CLIP text embeddings for a fixed vocabulary of common
/// photo categories (people, pets, vehicles, landscapes, etc.).
///
/// These embeddings are computed once at startup and reused for zero-shot
/// CLIP classification. Each embedding is L2-normalized so cosine similarity
/// can be computed via dot product.
fn compute_text_embeddings(
    text_model: &mut ModelEngine,
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
            let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
            if ids.len() > 77 {
                ids.truncate(77);
            } else {
                ids.resize(77, 0);
            }

            let arr = match Array2::from_shape_vec((1, 77), ids) {
                Ok(arr) => arr,
                Err(e) => {
                    tracing::error!("failed to build CLIP input tensor for '{text_label}': {e}");
                    continue;
                }
            };
            let shape = arr.shape().to_vec();
            let data = arr.into_raw_vec_and_offset().0;

            if let Ok(id_tensor) = ort::value::Value::from_array((shape, data)) {
                let extracted = {
                    let Some(mut lock) = text_model.lock().ok() else {
                        continue;
                    };
                    let outputs = lock.run(ort::inputs!["input_ids" => id_tensor]);
                    match outputs {
                        Ok(out) => {
                            if let Ok((_shape, text_emb_tensor)) =
                                out[0].try_extract_tensor::<f32>()
                            {
                                let mut text_embedding = vec![0.0; 512];
                                let len = text_emb_tensor.len().min(512);
                                text_embedding[..len].copy_from_slice(&text_emb_tensor[..len]);
                                Some(text_embedding)
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    }
                };
                if let Some(mut text_embedding) = extracted {
                    let text_norm: f32 = text_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
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
    embeddings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pool_size_keeps_heavy_models_single() {
        assert_eq!(SessionPool::pool_size_for("aesthetics.onnx"), 1);
        assert_eq!(SessionPool::pool_size_for("blip.onnx"), 1);
        assert_eq!(SessionPool::pool_size_for("blip_decoder.onnx"), 1);
        assert_eq!(SessionPool::pool_size_for("whisper.onnx"), 1);
        assert_eq!(SessionPool::pool_size_for("whisper-decoder.onnx"), 1);
        assert_eq!(
            SessionPool::pool_size_for("clip-vit-base-patch32-visual.onnx"),
            2
        );
        assert_eq!(SessionPool::pool_size_for("yolov8.onnx"), 2);
    }

    #[test]
    fn empty_pool_lock_fails() {
        let pool = SessionPool::new(Vec::new());
        assert!(pool.is_empty());
        assert!(pool.lock().is_err());
    }

    fn config_with(budget_mb: &str, enabled: &[&str]) -> HashMap<String, String> {
        let mut config = HashMap::new();
        config.insert("ml_memory_budget_mb".to_string(), budget_mb.to_string());
        for m in [
            "clip",
            "face",
            "arcface",
            "ocr",
            "nsfw",
            "aesthetics",
            "yolo",
            "blip",
            "midas",
            "whisper",
        ] {
            if !enabled.contains(&m) {
                config.insert(format!("model_enabled_{m}"), "false".to_string());
            }
        }
        config
    }

    fn noop_log(_msg: &str) {}

    #[test]
    fn budget_unset_returns_nothing() {
        let config = HashMap::new();
        assert!(dropped_over_budget(&config, &noop_log).is_empty());
    }

    #[test]
    fn budget_large_drops_nothing() {
        let config = config_with("32768", &["aesthetics", "blip", "clip"]);
        assert!(dropped_over_budget(&config, &noop_log).is_empty());
    }

    #[test]
    fn budget_drops_heaviest_model() {
        let config = config_with("2048", &["aesthetics", "blip", "yolo"]);
        let dropped = dropped_over_budget(&config, &noop_log);
        assert!(dropped.contains(&"aesthetics"));
        assert!(!dropped.contains(&"yolo"));
    }

    #[test]
    fn budget_ignores_disabled_models() {
        let config = config_with("1024", &["aesthetics"]);
        let dropped = dropped_over_budget(&config, &noop_log);
        assert!(dropped.contains(&"aesthetics"));
        assert!(!dropped.contains(&"blip"));
    }

    #[test]
    fn should_load_respects_budget_drop() {
        let config = config_with("1024", &["aesthetics", "yolo"]);
        let dropped = dropped_over_budget(&config, &noop_log);
        assert!(!should_load(&config, &dropped, "aesthetics"));
        assert!(should_load(&config, &dropped, "yolo"));
    }

    #[test]
    fn should_load_false_when_disabled() {
        let config = config_with("32768", &["clip"]);
        assert!(should_load(&config, &[], "clip"));
        assert!(!should_load(&config, &[], "midas"));
    }

    /// A models dir with every file `model_files_ok` requires (empty files are
    /// fine — feasibility only checks presence/size > 0).
    fn models_dir_with_files() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in [
            "clip-vit-base-patch32-visual.onnx",
            "clip-vit-base-patch32-text.onnx",
            "tokenizer.json",
            "face_detection_yunet_2023mar.onnx",
            "arcface.onnx",
            "ocr_det.onnx",
            "ocr_rec.onnx",
            "en_dict.txt",
            "nsfw.onnx",
            "aesthetics.onnx",
            "yolov8.onnx",
            "blip.onnx",
            "blip_decoder.onnx",
            "blip_tokenizer.json",
            "midas.onnx",
            "whisper.onnx",
            "whisper-decoder.onnx",
            "whisper-tokenizer.json",
        ] {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        dir
    }

    fn feasibility_for(
        models: &std::path::Path,
        config: &HashMap<String, String>,
        device_ram: Option<u64>,
    ) -> Vec<ModelFeasibility> {
        model_feasibility_with_ram(models, config, device_ram, &noop_log)
    }

    #[test]
    fn feasibility_all_runnable_when_ram_plentiful() {
        let models = models_dir_with_files();
        let config = HashMap::new(); // all enabled, no budget
        let out = feasibility_for(models.path(), &config, Some(64 * 1024 * 1024 * 1024));
        assert_eq!(out.len(), FEASIBILITY_MODELS.len());
        for v in &out {
            assert!(v.runnable, "{} should run: {:?}", v.model, v.reason);
        }
    }

    #[test]
    fn feasibility_drops_heaviest_models_on_4gb_device() {
        let models = models_dir_with_files();
        let config = HashMap::new(); // all enabled
                                     // 4 GB device -> 2 GB model cap. Enabled models total ~4.5 GB, so the
                                     // heaviest (aesthetics, then blip) are dropped first, like the loader.
        let out = feasibility_for(models.path(), &config, Some(4 * 1024 * 1024 * 1024));
        let aesthetics = out.iter().find(|v| v.model == "aesthetics").unwrap();
        assert!(!aesthetics.runnable);
        assert_eq!(aesthetics.reason.as_deref(), Some(REASON_LOW_RAM));
        let blip = out.iter().find(|v| v.model == "blip").unwrap();
        assert!(!blip.runnable);
        assert_eq!(blip.reason.as_deref(), Some(REASON_LOW_RAM));
        let yolo = out.iter().find(|v| v.model == "yolo").unwrap();
        assert!(yolo.runnable, "light models should still run");
    }

    #[test]
    fn feasibility_reports_memory_budget_when_user_cap_binds() {
        let models = models_dir_with_files();
        let config = config_with("2048", &["aesthetics", "blip", "yolo"]);
        // Device RAM is huge, so only the user's 2 GB budget can cause a drop.
        let out = feasibility_for(models.path(), &config, Some(64 * 1024 * 1024 * 1024));
        let aesthetics = out.iter().find(|v| v.model == "aesthetics").unwrap();
        assert!(!aesthetics.runnable);
        assert_eq!(aesthetics.reason.as_deref(), Some(REASON_MEMORY_BUDGET));
        let yolo = out.iter().find(|v| v.model == "yolo").unwrap();
        assert!(yolo.runnable);
    }

    #[test]
    fn feasibility_low_ram_binds_when_device_cap_smaller_than_user_budget() {
        let models = models_dir_with_files();
        let config = config_with("16384", &["aesthetics", "yolo"]); // 16 GB user budget
                                                                    // A 2 GB device caps models at 1 GB — tighter than the user's 16 GB.
        let out = feasibility_for(models.path(), &config, Some(2 * 1024 * 1024 * 1024));
        let aesthetics = out.iter().find(|v| v.model == "aesthetics").unwrap();
        assert_eq!(aesthetics.reason.as_deref(), Some(REASON_LOW_RAM));
        let yolo = out.iter().find(|v| v.model == "yolo").unwrap();
        assert!(yolo.runnable);
    }

    #[test]
    fn feasibility_reports_not_downloaded_for_missing_files() {
        let models = tempfile::tempdir().unwrap();
        let config = HashMap::new();
        let out = feasibility_for(models.path(), &config, Some(64 * 1024 * 1024 * 1024));
        assert_eq!(out.len(), FEASIBILITY_MODELS.len());
        for v in &out {
            assert!(!v.runnable);
            assert_eq!(v.reason.as_deref(), Some(REASON_NOT_DOWNLOADED));
        }
    }

    #[test]
    fn feasibility_skips_disabled_models() {
        let models = models_dir_with_files();
        let mut config = HashMap::new();
        config.insert("model_enabled_aesthetics".to_string(), "false".to_string());
        config.insert("model_enabled_blip".to_string(), "false".to_string());
        let out = feasibility_for(models.path(), &config, Some(2 * 1024 * 1024 * 1024));
        assert!(
            out.iter()
                .all(|v| v.model != "aesthetics" && v.model != "blip"),
            "disabled models must not be reported"
        );
    }

    /// Locate a directory containing at least the tiny YuNet face-detector
    /// model, used by [`session_pool_allows_concurrent_locks`]. Prefers the
    /// repo-local `test_models/` (CI), then the app's real models dir.
    fn test_models_dir() -> Option<std::path::PathBuf> {
        let candidates = [
            std::path::Path::new("test_models").to_path_buf(),
            crate::config::default_config_dir().join("models"),
        ];
        candidates
            .into_iter()
            .find(|dir| dir.join("face_detection_yunet_2023mar.onnx").exists())
    }

    /// Proves light models really get concurrent sessions: two threads each
    /// grab a slot from a 2-session pool simultaneously. A pool with a single
    /// session would block the second lock, which the channel handshake below
    /// detects deterministically (no timing-based assertions).
    #[test]
    #[ignore] // needs the ~230 KB YuNet model in test_models/ or the app dir
    fn session_pool_allows_concurrent_locks() {
        use crate::ml_engine::ep;
        use std::sync::mpsc;

        let Some(models_dir) = test_models_dir() else {
            println!("Skipping: face_detection_yunet_2023mar.onnx not present");
            return;
        };
        let model_path = models_dir.join("face_detection_yunet_2023mar.onnx");

        let mut sessions = Vec::new();
        for _ in 0..2 {
            match ep::build_session(&model_path, None) {
                Ok(session) => sessions.push(session),
                Err(e) => {
                    println!("Skipping: failed to load YuNet twice: {e}");
                    return;
                }
            }
        }

        let pool = Arc::new(SessionPool::new(sessions));

        let (a_idx_tx, a_idx_rx) = mpsc::channel();
        let (go_tx, go_rx) = mpsc::channel();
        let (b_locked_tx, b_locked_rx) = mpsc::channel();
        let (b_idx_tx, b_idx_rx) = mpsc::channel();

        let pool_a = Arc::clone(&pool);
        let pool_b = Arc::clone(&pool);

        std::thread::scope(|scope| {
            scope.spawn(move || {
                let guard = pool_a.lock().unwrap();
                let idx = guard.idx;
                a_idx_tx.send(idx).ok();
                go_tx.send(()).ok();
                // Hold this slot until B proves it acquired its own. If the
                // pool were single-session, B could never acquire while we
                // hold, so this receive would time out after 10s.
                let _ = b_locked_rx.recv_timeout(std::time::Duration::from_secs(10));
                drop(guard);
            });
            scope.spawn(move || {
                // Start only after A holds a slot.
                let _ = go_rx.recv_timeout(std::time::Duration::from_secs(10));
                let guard = pool_b.lock().unwrap();
                let idx = guard.idx;
                b_locked_tx.send(()).ok();
                b_idx_tx.send(idx).ok();
                drop(guard);
            });
        });

        let idx_a = a_idx_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("thread A should have locked a session");
        let idx_b = b_idx_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("thread B should have locked a second session");

        assert_ne!(
            idx_a, idx_b,
            "two concurrent locks must use distinct pooled sessions, got {idx_a} twice"
        );
    }
}
