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
}

/// Container for all loaded ML models and their associated data.
///
/// Each field is `Option<ModelEngine>` — `None` means the model was
/// disabled in config, not found on disk, or failed to load.
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

    let clip_visual = if model_enabled(config, "clip") {
        log("Loading CLIP visual model...");
        let m = load_model(&models_dir, "clip-vit-base-patch32-visual.onnx");
        log("CLIP visual ready.");
        m
    } else {
        None
    };
    let face_detector = if model_enabled(config, "face") || model_enabled(config, "ultraface") {
        log("Loading face detector...");
        let m =
            load_model_with_min_size(&models_dir, "face_detection_yunet_2023mar.onnx", 100 * 1024);
        log("Face detector ready.");
        m
    } else {
        None
    };
    let arcface = if model_enabled(config, "arcface") {
        log("Loading ArcFace model...");
        let m = load_model(&models_dir, "arcface.onnx");
        log("ArcFace ready.");
        m
    } else {
        None
    };
    let ocr_det = if model_enabled(config, "ocr") {
        log("Loading OCR detection model...");
        let m = load_model(&models_dir, "ocr_det.onnx");
        log("OCR detection ready.");
        m
    } else {
        None
    };
    let ocr_rec = if model_enabled(config, "ocr") {
        log("Loading OCR recognition model...");
        let m = load_model(&models_dir, "ocr_rec.onnx");
        log("OCR recognition ready.");
        m
    } else {
        None
    };
    let nsfw = if model_enabled(config, "nsfw") {
        log("Loading NSFW model...");
        let m = load_model(&models_dir, "nsfw.onnx");
        log("NSFW ready.");
        m
    } else {
        None
    };
    let aesthetics = if model_enabled(config, "aesthetics") {
        log("Loading aesthetics model (1.6 GB)...");
        let m = load_model(&models_dir, "aesthetics.onnx");
        log("Aesthetics ready.");
        m
    } else {
        None
    };
    let yolo = if model_enabled(config, "yolo") {
        log("Loading YOLO model...");
        let m = load_model(&models_dir, "yolov8.onnx");
        log("YOLO ready.");
        m
    } else {
        None
    };
    let blip = if model_enabled(config, "blip") {
        log("Loading BLIP vision encoder...");
        let m = load_model(&models_dir, "blip.onnx");
        log("BLIP vision encoder ready.");
        m
    } else {
        None
    };
    let blip_decoder = if model_enabled(config, "blip") {
        log("Loading BLIP text decoder...");
        let m = load_model(&models_dir, "blip_decoder.onnx");
        log("BLIP text decoder ready.");
        m
    } else {
        None
    };
    let blip_tokenizer = if model_enabled(config, "blip") {
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
    let midas = if model_enabled(config, "midas") {
        log("Loading MiDaS depth model...");
        let m = load_model(&models_dir, "midas.onnx");
        log("MiDaS ready.");
        m
    } else {
        None
    };
    let (whisper_encoder, whisper_decoder, whisper_tokenizer) = if model_enabled(config, "whisper")
    {
        log("Loading Whisper encoder...");
        let enc = load_model(&models_dir, "whisper.onnx");
        log("Whisper encoder ready.");
        log("Loading Whisper decoder...");
        let dec = load_model(&models_dir, "whisper-decoder.onnx");
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
    let clip_text = if model_enabled(config, "clip") && clip_text_path.exists() {
        if let Ok(tokenizer) = tokenizers::Tokenizer::from_file(&tokenizer_path) {
            log("Loading CLIP text model & computing embeddings...");
            if let Some(mut text_model) = load_model(&models_dir, "clip-vit-base-patch32-text.onnx")
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
fn load_model(models_dir: &Path, filename: &str) -> Option<ModelEngine> {
    load_model_with_min_size(models_dir, filename, 1024 * 1024)
}

/// Like [`load_model`] but with an explicit minimum file size, used for
/// small models such as YuNet (~232KB) that are below the 1MB default gate.
fn load_model_with_min_size(
    models_dir: &Path,
    filename: &str,
    min_size: u64,
) -> Option<ModelEngine> {
    let path = models_dir.join(filename);
    let is_ok = path.exists() && path.metadata().map(|m| m.len()).unwrap_or(0) > min_size;
    if !is_ok {
        return None;
    }
    let pool_size = SessionPool::pool_size_for(filename);
    let mut sessions = Vec::with_capacity(pool_size);
    for _ in 0..pool_size {
        if let Ok(session) = super::ep::build_session(&path) {
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
