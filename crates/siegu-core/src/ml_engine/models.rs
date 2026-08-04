//! Model loading and management for all ML pipelines.
//!
//! Each ONNX model is wrapped in `Arc<Mutex<Session>>` so it can be shared
//! across threads. Models are conditionally loaded based on user config flags
//! (`model_enabled_<name>` in the app config). Missing models are silently
//! skipped (returning `None`) so the app degrades gracefully.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use ndarray::Array2;
use ort::session::Session;

/// Thread-safe handle to an ORT inference session.
pub type ModelEngine = Arc<Mutex<Session>>;

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
    super::ep::build_session(&path)
        .ok()
        .map(|s| Arc::new(Mutex::new(s)))
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
                    let mut lock = text_model.lock().unwrap_or_else(|e| e.into_inner());
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
