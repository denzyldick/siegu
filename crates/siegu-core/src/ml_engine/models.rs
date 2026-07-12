use std::path::Path;
use std::sync::{Arc, Mutex};

use ndarray::Array2;
use ort::session::Session;

pub type ModelEngine = Arc<Mutex<Session>>;

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
    pub midas: Option<ModelEngine>,
    pub whisper: Option<ModelEngine>,
    pub known_people: Vec<(String, Vec<f32>)>,
    pub selected_ep: String,
}

pub fn load_models(config_path: &str, known_people: Vec<(String, Vec<f32>)>) -> LoadedModels {
    let models_dir = Path::new(config_path).join("models");
    let is_ok = |p: &Path| p.exists() && p.metadata().map(|m| m.len()).unwrap_or(0) > 1024 * 1024;

    let clip_visual = load_model(&models_dir, "clip-vit-base-patch32-visual.onnx");
    let face_detector = load_model(&models_dir, "version-RFB-320.onnx");
    let arcface = load_model(&models_dir, "arcface.onnx");
    let ocr_det = load_model(&models_dir, "ocr_det.onnx");
    let ocr_rec = load_model(&models_dir, "ocr_rec.onnx");
    let nsfw = load_model(&models_dir, "nsfw.onnx");
    let aesthetics = load_model(&models_dir, "aesthetics.onnx");
    let yolo = load_model(&models_dir, "yolov8.onnx");
    let blip = load_model(&models_dir, "blip.onnx");
    let midas = load_model(&models_dir, "midas.onnx");
    let whisper = load_model(&models_dir, "whisper.onnx");

    let clip_text_path = models_dir.join("clip-vit-base-patch32-text.onnx");
    let tokenizer_path = models_dir.join("tokenizer.json");
    let ocr_dict_path = models_dir.join("en_dict.txt");

    let mut text_embeddings = Vec::new();
    let clip_text = if is_ok(&clip_text_path) {
        if let Ok(tokenizer) = tokenizers::Tokenizer::from_file(&tokenizer_path) {
            if let Some(mut text_model) = load_model(&models_dir, "clip-vit-base-patch32-text.onnx")
            {
                text_embeddings = compute_text_embeddings(&mut text_model, &tokenizer);
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

    let ocr_alphabet = if ocr_dict_path.exists() {
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
        midas,
        whisper,
        known_people,
        selected_ep,
    }
}

fn load_model(models_dir: &Path, filename: &str) -> Option<ModelEngine> {
    let path = models_dir.join(filename);
    let is_ok = path.exists() && path.metadata().map(|m| m.len()).unwrap_or(0) > 1024 * 1024;
    if !is_ok {
        return None;
    }
    super::ep::build_session(&path)
        .ok()
        .map(|s| Arc::new(Mutex::new(s)))
}

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

            let arr = Array2::from_shape_vec((1, 77), ids).unwrap();
            let shape = arr.shape().to_vec();
            let data = arr.into_raw_vec_and_offset().0;

            if let Ok(id_tensor) = ort::value::Value::from_array((shape, data)) {
                let extracted = {
                    let mut lock = text_model.lock().unwrap();
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
