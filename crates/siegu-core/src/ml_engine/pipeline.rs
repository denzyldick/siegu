use std::collections::HashMap;
use std::time::Instant;

use crate::database::{AiStatus, Face};
use crate::face_detector;
use crate::ml_worker::should_run_model;
use crate::thumbnail;

use super::models::{LoadedModels, ModelEngine};
use super::preprocessing;
use super::whisper;

const COCO_CLASSES: &[&str] = &[
    "person",
    "bicycle",
    "car",
    "motorcycle",
    "airplane",
    "bus",
    "train",
    "truck",
    "boat",
    "traffic light",
    "fire hydrant",
    "stop sign",
    "parking meter",
    "bench",
    "bird",
    "cat",
    "dog",
    "horse",
    "sheep",
    "cow",
    "elephant",
    "bear",
    "zebra",
    "giraffe",
    "backpack",
    "umbrella",
    "handbag",
    "tie",
    "suitcase",
    "frisbee",
    "skis",
    "snowboard",
    "sports ball",
    "kite",
    "baseball bat",
    "baseball glove",
    "skateboard",
    "surfboard",
    "tennis racket",
    "bottle",
    "wine glass",
    "cup",
    "fork",
    "knife",
    "spoon",
    "bowl",
    "banana",
    "apple",
    "sandwich",
    "orange",
    "broccoli",
    "carrot",
    "hot dog",
    "pizza",
    "donut",
    "cake",
    "chair",
    "couch",
    "potted plant",
    "bed",
    "dining table",
    "toilet",
    "tv",
    "laptop",
    "mouse",
    "remote",
    "keyboard",
    "cell phone",
    "microwave",
    "oven",
    "toaster",
    "sink",
    "refrigerator",
    "book",
    "clock",
    "vase",
    "scissors",
    "teddy bear",
    "hair drier",
    "toothbrush",
];

#[derive(Debug, Clone, Default)]
pub struct PhotoResult {
    pub objects: Vec<(String, String)>,
    pub aesthetics: Option<f64>,
    pub nsfw: Option<String>,
    pub ocr: Option<String>,
    pub transcript: Option<String>,
    pub caption: Option<String>,
    pub face_count: usize,
    pub faces: Vec<FaceInfo>,
    pub completed_models: Vec<&'static str>,
    pub model_timings: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct FaceInfo {
    pub face_id: String,
    pub crop_path: String,
    pub encoded: String,
    pub embedding: Vec<f32>,
    pub person_id: Option<String>,
}

pub fn analyze_photo(
    photo_id: &str,
    location: &str,
    ai_status: &AiStatus,
    models: &LoadedModels,
    config: &HashMap<String, String>,
    target_model: Option<&str>,
    faces_dir: &str,
) -> PhotoResult {
    let dynamic_img = match thumbnail::open_image(location) {
        Some(img) => img,
        None => return PhotoResult::default(),
    };

    let orientation = thumbnail::read_exif_orientation(location);
    let dynamic_img = thumbnail::apply_orientation(dynamic_img, orientation);
    let img = dynamic_img.to_rgb8();
    let img = cap_decode_dimension(img, config);

    analyze_image(
        photo_id,
        0,
        &img,
        ai_status,
        models,
        config,
        target_model,
        faces_dir,
    )
}

fn cap_decode_dimension(img: image::RgbImage, config: &HashMap<String, String>) -> image::RgbImage {
    let (w, h) = (img.width(), img.height());
    let max_dim = config
        .get("ml_max_decode_dim")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(2048);
    if max_dim == 0 || w <= max_dim && h <= max_dim {
        return img;
    }
    let scale = max_dim as f32 / w.max(h) as f32;
    let (nw, nh) = (
        (w as f32 * scale).max(1.0) as u32,
        (h as f32 * scale).max(1.0) as u32,
    );
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
}

#[allow(clippy::too_many_arguments)]
pub fn analyze_image(
    photo_id: &str,
    frame_index: usize,
    img: &image::RgbImage,
    ai_status: &AiStatus,
    models: &LoadedModels,
    config: &HashMap<String, String>,
    target_model: Option<&str>,
    faces_dir: &str,
) -> PhotoResult {
    let mut result = PhotoResult::default();
    let (orig_w, orig_h) = (img.width() as f32, img.height() as f32);

    // CLIP Visual
    if should_run_model(target_model, "clip", Some(config)) && ai_status.clip == 0 {
        if let Some(ref visual_model) = models.clip_visual {
            let start = Instant::now();
            let input = preprocessing::clip_preprocess(img);
            if let Ok(data) = run_model(visual_model, input, "pixel_values") {
                let mut visual_embedding = data;
                let norm: f32 = visual_embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for v in visual_embedding.iter_mut() {
                        *v /= norm;
                    }
                }
                let mut similarities: Vec<(String, f32)> = models
                    .text_embeddings
                    .iter()
                    .map(|(label, emb)| {
                        let dot: f32 = visual_embedding
                            .iter()
                            .zip(emb.iter())
                            .map(|(a, b)| a * b)
                            .sum();
                        (label.clone(), dot)
                    })
                    .collect();
                similarities
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                for (label, score) in similarities.iter().take(5) {
                    result.objects.push((label.clone(), format!("{score:.2}")));
                }
                result.completed_models.push("clip");
                result
                    .model_timings
                    .insert("clip".to_string(), start.elapsed().as_secs_f64());
            }
        }
    }

    // Aesthetics
    if should_run_model(target_model, "aesthetics", Some(config)) && ai_status.aesthetics == 0 {
        if let Some(ref model) = models.aesthetics {
            let start = Instant::now();
            let input = preprocessing::aesthetics_preprocess(img);
            if let Ok(data) = run_model(model, input, "input") {
                result.aesthetics = Some(data[0] as f64);
                result.completed_models.push("aesthetics");
                result
                    .model_timings
                    .insert("aesthetics".to_string(), start.elapsed().as_secs_f64());
            }
        }
    }

    // NSFW
    if should_run_model(target_model, "nsfw", Some(config)) && ai_status.nsfw == 0 {
        if let Some(ref model) = models.nsfw {
            let start = Instant::now();
            let input = preprocessing::nsfw_preprocess(img);
            if let Ok(data) = run_model(model, input, "pixel_values") {
                let nsfw_score = if data.len() >= 2 {
                    let e0 = data[0].exp();
                    let e1 = data[1].exp();
                    e1 / (e0 + e1)
                } else {
                    data[0]
                };
                result.nsfw = Some(nsfw_score.to_string());
                result.completed_models.push("nsfw");
                result
                    .model_timings
                    .insert("nsfw".to_string(), start.elapsed().as_secs_f64());
            }
        }
    }

    // OCR
    if should_run_model(target_model, "ocr", Some(config)) && ai_status.ocr == 0 {
        match &models.ocr_rec {
            Some(ref rec_model) if models.ocr_det.is_some() && !models.ocr_alphabet.is_empty() => {
                let start = Instant::now();
                let input = preprocessing::ocr_preprocess(img);
                if let Ok(rec_logits) = run_model(rec_model, input, "x") {
                    let seq_len = rec_logits.len() / models.ocr_alphabet.len();
                    let num_classes = models.ocr_alphabet.len();
                    let mut recognized_text = String::new();
                    let mut last_char_idx = 0;
                    for i in 0..seq_len {
                        let chunk = &rec_logits[i * num_classes..(i + 1) * num_classes];
                        let mut max_val = -f32::INFINITY;
                        let mut max_idx = 0;
                        for (idx, &val) in chunk.iter().enumerate() {
                            if val > max_val {
                                max_val = val;
                                max_idx = idx;
                            }
                        }
                        if max_idx != 0 && max_idx != last_char_idx {
                            if let Some(c) = models.ocr_alphabet.get(max_idx) {
                                recognized_text.push_str(c);
                            }
                        }
                        last_char_idx = max_idx;
                    }
                    if !recognized_text.trim().is_empty() {
                        result.ocr = Some(recognized_text);
                    }
                }
                result.completed_models.push("ocr");
                result
                    .model_timings
                    .insert("ocr".to_string(), start.elapsed().as_secs_f64());
            }
            _ => {}
        }
    }

    // YOLOv8
    if should_run_model(target_model, "yolo", Some(config)) && ai_status.yolo == 0 {
        if let Some(ref model) = models.yolo {
            let start = Instant::now();
            let input = preprocessing::yolo_preprocess(img);
            if let Ok(data) = run_model(model, input, "images") {
                let num_classes = 80;
                let num_anchors = 8400;
                let mut found_classes: HashMap<usize, f32> = HashMap::new();
                for a in 0..num_anchors {
                    let mut max_conf = 0.0f32;
                    let mut max_cls = 0;
                    for c in 0..num_classes {
                        let conf = data[num_anchors * (4 + c) + a];
                        if conf > max_conf {
                            max_conf = conf;
                            max_cls = c;
                        }
                    }
                    if max_conf > 0.6 {
                        found_classes
                            .entry(max_cls)
                            .and_modify(|e| *e = (*e).max(max_conf))
                            .or_insert(max_conf);
                    }
                }
                for (cls_idx, conf) in found_classes {
                    let name = COCO_CLASSES.get(cls_idx).copied().unwrap_or("unknown");
                    result.objects.push((name.to_string(), conf.to_string()));
                }
                result.completed_models.push("yolo");
                result
                    .model_timings
                    .insert("yolo".to_string(), start.elapsed().as_secs_f64());
            }
        }
    }

    // Face Detection + ArcFace
    let should_run_face =
        should_run_model(target_model, "face", Some(config)) && ai_status.face == 0;
    let should_run_arcface =
        should_run_model(target_model, "arcface", Some(config)) && ai_status.arcface == 0;
    if should_run_face || should_run_arcface {
        if let Some(ref face_model) = models.face_detector {
            let start = Instant::now();
            let input = preprocessing::yunet_preprocess(img);
            if let Ok(outputs) = run_model_named(face_model, input, "input") {
                let dets = face_detector::decode_yunet(&outputs, 640, 0.5, 0.3);
                let sx = orig_w / 640.0;
                let sy = orig_h / 640.0;
                let dets = face_detector::scale_yunet_faces(&dets, sx, sy);
                let mut buffer = std::io::Cursor::new(Vec::new());
                for det in &dets {
                    let bbox = det.bbox;
                    let xmin = bbox[0].max(0.0) as u32;
                    let ymin = bbox[1].max(0.0) as u32;
                    let xmax = bbox[2].min(orig_w) as u32;
                    let ymax = bbox[3].min(orig_h) as u32;
                    if xmax > xmin && ymax > ymin {
                        let (w, h) = (xmax - xmin, ymax - ymin);
                        if w > 20 && h > 20 {
                            let face_crop =
                                image::imageops::crop_imm(img, xmin, ymin, w, h).to_image();
                            let face_id = format!("{photo_id}_f{frame_index}_{xmin}_{ymin}");
                            let crop_path = format!("{faces_dir}/{face_id}.jpg");
                            if face_crop.save(&crop_path).is_ok() {
                                let mut face_embedding = Vec::new();
                                if let Some(ref model) = models.arcface {
                                    let lm: [(f32, f32); 5] = [
                                        (det.landmarks[0], det.landmarks[1]),
                                        (det.landmarks[2], det.landmarks[3]),
                                        (det.landmarks[4], det.landmarks[5]),
                                        (det.landmarks[6], det.landmarks[7]),
                                        (det.landmarks[8], det.landmarks[9]),
                                    ];
                                    let m = face_detector::estimate_partial_affine(
                                        &lm,
                                        &face_detector::ALIGN_REF,
                                    );
                                    let aligned = face_detector::warp_affine(img, &m, (112, 112));
                                    let f_input = preprocessing::arcface_preprocess(&aligned);
                                    if let Ok(emb) = run_model(model, f_input, "input.1") {
                                        let mut e = emb;
                                        let norm: f32 = e.iter().map(|x| x * x).sum::<f32>().sqrt();
                                        if norm > 0.0 {
                                            for v in e.iter_mut() {
                                                *v /= norm;
                                            }
                                        }
                                        face_embedding = e;
                                    }
                                } else if let Some(ref visual_model) = models.clip_visual {
                                    let face_resized = image::imageops::resize(
                                        &face_crop,
                                        224,
                                        224,
                                        image::imageops::FilterType::Triangle,
                                    );
                                    let face_input = preprocessing::clip_preprocess(&face_resized);
                                    if let Ok(emb) =
                                        run_model(visual_model, face_input, "pixel_values")
                                    {
                                        let mut e = emb;
                                        let norm: f32 = e.iter().map(|x| x * x).sum::<f32>().sqrt();
                                        if norm > 0.0 {
                                            for v in e.iter_mut() {
                                                *v /= norm;
                                            }
                                        }
                                        face_embedding = e;
                                    }
                                }

                                let mut assigned_person_id = None;
                                if !face_embedding.is_empty() {
                                    let mut highest_similarity = 0.0f32;
                                    let mut best_match_id = None;
                                    for (person_id, person_centroid) in &models.known_people {
                                        let dot: f32 = face_embedding
                                            .iter()
                                            .zip(person_centroid.iter())
                                            .map(|(a, b)| a * b)
                                            .sum();
                                        if dot > highest_similarity {
                                            highest_similarity = dot;
                                            best_match_id = Some(person_id.clone());
                                        }
                                    }
                                    if highest_similarity > 0.5 {
                                        assigned_person_id = best_match_id;
                                    }
                                }

                                buffer.get_mut().clear();
                                buffer.set_position(0);
                                let _ = face_crop.write_to(&mut buffer, image::ImageFormat::Jpeg);
                                let encoded = format!(
                                    "data:image/jpeg;base64,{}",
                                    base64::Engine::encode(
                                        &base64::engine::general_purpose::STANDARD,
                                        buffer.get_ref()
                                    )
                                );
                                result.faces.push(FaceInfo {
                                    face_id,
                                    crop_path,
                                    encoded,
                                    embedding: face_embedding,
                                    person_id: assigned_person_id,
                                });
                            }
                        }
                    }
                }
            }
            result.face_count = result.faces.len();
            result.completed_models.push("face");
            if models.arcface.is_some() {
                result.completed_models.push("arcface");
            }
            result
                .model_timings
                .insert("face".to_string(), start.elapsed().as_secs_f64());
        }
    }

    // BLIP caption generation
    if should_run_model(target_model, "blip", Some(config)) && ai_status.blip == 0 {
        let start = Instant::now();
        let caption = generate_blip_caption(img, models);
        if let Some(caption) = caption {
            result.caption = Some(caption);
            result.completed_models.push("blip");
            result
                .model_timings
                .insert("blip".to_string(), start.elapsed().as_secs_f64());
        }
    }

    // MiDaS (opt-in: depth output is currently unused, so it is gated off by default)
    if config.get("midas_enabled").is_some_and(|v| v == "true")
        && should_run_model(target_model, "midas", Some(config))
        && ai_status.midas == 0
    {
        if let Some(ref model) = models.midas {
            let start = Instant::now();
            let input = preprocessing::midas_preprocess(img);
            if let Ok(_data) = run_model(model, input, "pixel_values") {
                result.completed_models.push("midas");
                result
                    .model_timings
                    .insert("midas".to_string(), start.elapsed().as_secs_f64());
            }
        }
    }

    result
}

/// Generates an image caption using the BLIP vision encoder + text decoder.
///
/// 1. Runs `blip.onnx` (vision encoder) to produce image embeddings
/// 2. Runs `blip_decoder.onnx` (text decoder) autoregressively from [CLS]
/// 3. Returns the decoded caption string, or [`None`] on failure
pub fn generate_blip_caption(img: &image::RgbImage, models: &LoadedModels) -> Option<String> {
    let vision_encoder = models.blip.as_ref()?;
    let decoder = models.blip_decoder.as_ref()?;
    let tokenizer = models.blip_tokenizer.as_ref()?;

    // ── 1. Run vision encoder ────────────────────────────────────────────
    let input = preprocessing::blip_preprocess(img);
    let shape = input.shape().to_vec();
    let data = input.into_raw_vec_and_offset().0;
    let tensor = ort::value::Value::from_array((shape, data)).ok()?;

    let pooled = {
        let mut lock = vision_encoder.lock().ok()?;
        let encoder_outputs = match lock.run(ort::inputs!["pixel_values" => tensor]) {
            Ok(outputs) => outputs,
            Err(e) => {
                tracing::warn!("blip vision encoder run failed: {e}");
                return None;
            }
        };
        // The onnx-community split export emits `encoder_hidden_states` (f32)
        // and a degenerate single-element `encoder_attention_mask`; the decoder
        // split consumes a mean-pooled single embedding, so we only need the
        // hidden states here.
        let mut enc_hidden: Option<Vec<f32>> = None;
        for (name, output) in encoder_outputs {
            if name == "encoder_hidden_states" {
                match output.try_extract_tensor::<f32>() {
                    Ok((_, data)) => enc_hidden = Some(data.to_vec()),
                    Err(e) => tracing::warn!("blip encoder_hidden_states is not f32: {e}"),
                }
            }
        }
        let enc_hidden = enc_hidden?;
        let seq_len = enc_hidden.len() / 768;
        if seq_len == 0 {
            tracing::warn!("blip encoder_hidden_states has zero sequence length");
            return None;
        }
        // Mean-pool over the visual tokens, matching the reference
        // `image_embeds.mean(dim=1)` behavior of the captioning model.
        let mut pooled = Vec::with_capacity(768);
        for i in 0..768 {
            pooled.push(enc_hidden[i..].iter().step_by(768).sum::<f32>() / seq_len as f32);
        }
        pooled
    };

    let bos: i64 = 30522;
    let eos: i64 = 2;
    let max_len: usize = 20;

    let mut tokens: Vec<i64> = vec![bos];

    for _ in 0..max_len {
        let seq_len = tokens.len();
        let ids_arr = ndarray::Array2::from_shape_vec((1, seq_len), tokens.clone()).ok()?;
        let mask_arr = ndarray::Array2::from_shape_vec((1, seq_len), vec![1i64; seq_len]).ok()?;
        // The onnx-community decoder split accepts a single pooled image
        // embedding (its cross-attention is collapsed to one token), so the
        // mean-pooled encoder output is fed as [1, 1, 768] with a scalar mask.
        let enc_arr = ndarray::Array3::from_shape_vec((1, 1, 768), pooled.clone()).ok()?;
        let enc_mask_arr = ndarray::Array1::from_vec(vec![1i64]);

        let ids_tensor = ort::value::Value::from_array((
            ids_arr.shape().to_vec(),
            ids_arr.into_raw_vec_and_offset().0,
        ))
        .ok()?;
        let mask_tensor = ort::value::Value::from_array((
            mask_arr.shape().to_vec(),
            mask_arr.into_raw_vec_and_offset().0,
        ))
        .ok()?;
        let enc_tensor = ort::value::Value::from_array((
            enc_arr.shape().to_vec(),
            enc_arr.into_raw_vec_and_offset().0,
        ))
        .ok()?;
        let enc_mask_tensor = ort::value::Value::from_array((
            enc_mask_arr.shape().to_vec(),
            enc_mask_arr.into_raw_vec_and_offset().0,
        ))
        .ok()?;

        let mut inputs: HashMap<String, ort::value::Value> = HashMap::new();
        inputs.insert("input_ids".into(), ids_tensor.into_dyn());
        inputs.insert("attention_mask".into(), mask_tensor.into_dyn());
        inputs.insert("encoder_hidden_states".into(), enc_tensor.into_dyn());
        inputs.insert("encoder_attention_mask".into(), enc_mask_tensor.into_dyn());

        let next_token = {
            let mut lock = decoder.lock().ok()?;
            let outputs = match lock.run(inputs) {
                Ok(outputs) => outputs,
                Err(e) => {
                    tracing::warn!("blip decoder run failed: {e}");
                    return None;
                }
            };
            let (logits_shape, logits_data) = match outputs[0].try_extract_tensor::<f32>() {
                Ok((shape, data)) => (shape, data),
                Err(e) => {
                    tracing::warn!("blip decoder output[0] is not f32: {e}");
                    return None;
                }
            };
            let vocab_size = *logits_shape.last().unwrap_or(&30524) as usize;
            let last_offset = logits_data.len().saturating_sub(vocab_size);
            let last_logits = &logits_data[last_offset..last_offset + vocab_size];

            last_logits
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i as i64)?
        };

        if next_token == eos {
            break;
        }
        // Stop before the decoder collapses into repetition, which the
        // single-token export does quickly under greedy decoding.
        tokens.push(next_token);
        if tokens.len() >= 3 {
            let last3 = &tokens[tokens.len() - 3..];
            if tokens[..tokens.len() - 3].windows(3).any(|w| w == last3) {
                break;
            }
        }
    }

    // Decode tokens (skip BOS prefix)
    let decoded: Vec<u32> = tokens[1..].iter().map(|&t| t as u32).collect();
    let decoded = tokenizer.decode(&decoded, false).ok()?.trim().to_string();
    if decoded.is_empty() || decoded == "[UNK]" {
        None
    } else {
        Some(decoded)
    }
}

pub fn is_video_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".mp4")
        || lower.ends_with(".mkv")
        || lower.ends_with(".mov")
        || lower.ends_with(".avi")
        || lower.ends_with(".webm")
        || lower.ends_with(".flv")
        || lower.ends_with(".wmv")
        || lower.ends_with(".m4v")
        || lower.ends_with(".ts")
}

pub fn analyze_video(
    photo_id: &str,
    location: &str,
    ai_status: &AiStatus,
    models: &LoadedModels,
    config: &HashMap<String, String>,
    target_model: Option<&str>,
    faces_dir: &str,
) -> PhotoResult {
    let mut result = PhotoResult::default();

    if should_run_model(target_model, "whisper", Some(config)) && ai_status.whisper == 0 {
        let enc = models.whisper_encoder.as_ref();
        let dec = models.whisper_decoder.as_ref();
        let tok = models.whisper_tokenizer.as_ref();

        if let (Some(enc), Some(dec), Some(tok)) = (enc, dec, tok) {
            let start = Instant::now();
            if let Some(audio) = whisper::extract_audio(location) {
                if !audio.is_empty() {
                    let transcript = whisper::whisper_transcribe(enc, dec, tok, &audio);
                    if !transcript.is_empty() {
                        result.transcript = Some(transcript);
                        result.completed_models.push("whisper");
                        result
                            .model_timings
                            .insert("whisper".to_string(), start.elapsed().as_secs_f64());
                    }
                }
            }
        }
    }

    let any_visual_disabled = ai_status.clip == 0
        || ai_status.yolo == 0
        || ai_status.nsfw == 0
        || ai_status.aesthetics == 0
        || ai_status.face == 0;
    let any_visual_requested = target_model.is_none()
        || should_run_model(target_model, "clip", Some(config))
        || should_run_model(target_model, "yolo", Some(config))
        || should_run_model(target_model, "nsfw", Some(config))
        || should_run_model(target_model, "aesthetics", Some(config))
        || should_run_model(target_model, "face", Some(config));

    if any_visual_disabled && any_visual_requested {
        let frames = whisper::extract_frames(location);
        if !frames.is_empty() {
            let frame_count = frames.len();
            let mut frame_results: Vec<PhotoResult> = Vec::with_capacity(frame_count);

            for (i, frame) in frames.iter().enumerate() {
                let frame_ai = AiStatus::default();
                frame_results.push(analyze_image(
                    photo_id,
                    i,
                    frame,
                    &frame_ai,
                    models,
                    config,
                    target_model,
                    faces_dir,
                ));
            }

            result = aggregate_frame_results(&frame_results, frame_count);
        }
    }

    result
}

fn aggregate_frame_results(frame_results: &[PhotoResult], _frame_count: usize) -> PhotoResult {
    let mut merged = PhotoResult::default();

    let mut best_objects: HashMap<String, f32> = HashMap::new();
    let mut aesthetics_sum = 0.0f64;
    let mut aesthetics_count = 0u32;
    let mut max_nsfw = 0.0f64;
    let mut ocr_texts: Vec<String> = Vec::new();
    let mut all_faces: Vec<FaceInfo> = Vec::new();

    for fr in frame_results {
        for (cls, prob_str) in &fr.objects {
            if let Ok(p) = prob_str.parse::<f32>() {
                best_objects
                    .entry(cls.clone())
                    .and_modify(|e| {
                        if p > *e {
                            *e = p;
                        }
                    })
                    .or_insert(p);
            }
        }
        if let Some(a) = fr.aesthetics {
            aesthetics_sum += a;
            aesthetics_count += 1;
        }
        if let Some(ref nsfw_str) = fr.nsfw {
            if let Ok(n) = nsfw_str.parse::<f64>() {
                if n > max_nsfw {
                    max_nsfw = n;
                }
            }
        }
        if let Some(ref text) = fr.ocr {
            if !text.trim().is_empty() {
                ocr_texts.push(text.clone());
            }
        }
        all_faces.extend(fr.faces.iter().cloned());
    }

    for (cls, conf) in best_objects {
        merged.objects.push((cls, format!("{conf:.2}")));
    }

    if aesthetics_count > 0 {
        merged.aesthetics = Some(aesthetics_sum / aesthetics_count as f64);
    }
    if max_nsfw > 0.0 {
        merged.nsfw = Some(max_nsfw.to_string());
    }

    let mut seen_ocr = std::collections::HashSet::new();
    for text in &ocr_texts {
        if seen_ocr.insert(text.clone()) {
            merged.ocr = Some(text.clone());
        }
    }

    let mut unique_faces: Vec<FaceInfo> = Vec::new();
    for face in &all_faces {
        let is_dup = unique_faces.iter().any(|existing| {
            let dot: f32 = face
                .embedding
                .iter()
                .zip(existing.embedding.iter())
                .map(|(a, b)| a * b)
                .sum();
            dot > 0.85
        });
        if !is_dup {
            unique_faces.push(face.clone());
        }
    }
    merged.faces = unique_faces;
    merged.face_count = merged.faces.len();

    if !frame_results.is_empty() {
        merged.completed_models.push("clip");
        merged.completed_models.push("yolo");
        merged.completed_models.push("nsfw");
        merged.completed_models.push("aesthetics");
        if merged.face_count > 0 {
            merged.completed_models.push("face");
            merged.completed_models.push("arcface");
        }
        // Per-frame models whose outputs are carried into the merge but whose
        // completion markers were previously dropped here.
        let ran = |m: &'static str| {
            frame_results
                .iter()
                .any(|fr| fr.completed_models.contains(&m))
        };
        if ran("blip") {
            merged.caption = frame_results
                .iter()
                .find_map(|fr| fr.caption.as_ref().filter(|c| !c.trim().is_empty()))
                .cloned();
            merged.completed_models.push("blip");
        }
        if ran("ocr") {
            merged.completed_models.push("ocr");
        }
        if ran("midas") {
            merged.completed_models.push("midas");
        }
    }

    merged
}

fn run_model(
    model: &ModelEngine,
    input: ndarray::Array4<f32>,
    input_name: &str,
) -> Result<Vec<f32>, String> {
    let shape = input.shape().to_vec();
    let data = input.into_raw_vec_and_offset().0;
    let tensor = ort::value::Value::from_array((shape, data)).map_err(|e| e.to_string())?;
    let mut lock = model.lock().map_err(|e| e.to_string())?;
    let outputs = lock.run(ort::inputs![input_name => tensor]).map_err(|e| {
        // Surface inference failures through the app's debug log (the Tauri
        // `LogLayer` forwards every tracing event to `persist_log`), instead
        // of silently skipping the model with no trace for the user.
        tracing::warn!("AI inference failed (model output slot: {input_name}): {e}");
        e.to_string()
    })?;
    let mut results = Vec::new();
    for i in 0..outputs.len() {
        if let Ok((_shape, data)) = outputs[i].try_extract_tensor::<f32>() {
            results.extend_from_slice(data);
        }
    }
    Ok(results)
}

/// Like [`run_model`] but returns each named output tensor separately,
/// preserving the ONNX output names (required for YuNet's multi-branch
/// cls/obj/bbox/kps decoding).
fn run_model_named(
    model: &ModelEngine,
    input: ndarray::Array4<f32>,
    input_name: &str,
) -> Result<HashMap<String, Vec<f32>>, String> {
    let shape = input.shape().to_vec();
    let data = input.into_raw_vec_and_offset().0;
    let tensor = ort::value::Value::from_array((shape, data)).map_err(|e| e.to_string())?;
    let mut lock = model.lock().map_err(|e| e.to_string())?;
    let outputs = lock.run(ort::inputs![input_name => tensor]).map_err(|e| {
        tracing::warn!("AI inference failed (model output slot: {input_name}): {e}");
        e.to_string()
    })?;
    let mut results = HashMap::new();
    for (name, output) in outputs {
        if let Ok((_shape, data)) = output.try_extract_tensor::<f32>() {
            results.insert(name.to_string(), data.to_vec());
        }
    }
    Ok(results)
}

pub fn flush_results_to_db(
    db: &crate::database::Database,
    photo_id: &str,
    result: &PhotoResult,
    target_model: Option<&str>,
) {
    use crate::ml_worker::flush_batch_in_transaction;

    let _ = flush_batch_in_transaction(&db.connection, || {
        flush_results_statements(db, photo_id, result, target_model)?;
        Ok(())
    });
}

/// Flushes results for several photos inside a single transaction.
pub fn flush_results_batch_to_db(
    db: &crate::database::Database,
    results: &[(String, PhotoResult)],
    target_model: Option<&str>,
) {
    use crate::ml_worker::flush_batch_in_transaction;

    let _ = flush_batch_in_transaction(&db.connection, || {
        for (photo_id, result) in results {
            flush_results_statements(db, photo_id, result, target_model)?;
        }
        Ok(())
    });
}

fn flush_results_statements(
    db: &crate::database::Database,
    photo_id: &str,
    result: &PhotoResult,
    target_model: Option<&str>,
) -> Result<(), String> {
    for (class, prob) in &result.objects {
        let _ = db.connection.execute(
            "INSERT INTO object (photo_id, class, probability) VALUES(?1, ?2, ?3)",
            (photo_id, class, prob),
        );
    }
    if let Some(ref text) = result.ocr {
        let _ = db.connection.execute(
            "INSERT INTO ocr (photo_id, text) VALUES(?1, ?2)",
            (photo_id, text),
        );
    }
    if let Some(ref score) = result.nsfw {
        let _ = db.connection.execute(
            "INSERT INTO properties (photo_id, key, value) VALUES(?1, 'nsfw', ?2)",
            (photo_id, score),
        );
    }
    if let Some(score) = result.aesthetics {
        let _ = db.connection.execute(
            "UPDATE photo SET aesthetics_score = ?1 WHERE id = ?2",
            (score, photo_id),
        );
    }
    if let Some(ref caption) = result.caption {
        let _ = db.connection.execute(
            "UPDATE photo SET caption = ?1 WHERE id = ?2",
            (caption, photo_id),
        );
    }
    let _ = db.connection.execute(
        "INSERT OR REPLACE INTO properties (photo_id, key, value) VALUES(?1, 'face_count', ?2)",
        (photo_id, &result.face_count.to_string()),
    );

    for face_info in &result.faces {
        db.store_face(Face {
            photo_id: photo_id.to_string(),
            face_id: face_info.face_id.clone(),
            crop_path: face_info.crop_path.clone(),
            encoded: face_info.encoded.clone(),
            embedding: face_info.embedding.clone(),
            person_id: face_info.person_id.clone(),
        });
    }

    for model in &result.completed_models {
        db.update_ai_status(photo_id, model, 1);
    }

    if let Some(ref transcript) = result.transcript {
        let _ = db.connection.execute(
            "INSERT OR REPLACE INTO properties (photo_id, key, value) VALUES(?1, 'transcript', ?2)",
            (photo_id, transcript),
        );
    }

    if target_model.is_none() {
        db.update_photo_indexed(photo_id, 2);
    }
    let _ = db.connection.execute(
        "UPDATE photo SET sync_needed = 1 WHERE id = ?1 AND received = 0",
        [photo_id],
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::AiStatus;

    fn default_config() -> HashMap<String, String> {
        let mut config = HashMap::new();
        config.insert("model_enabled_clip".to_string(), "true".to_string());
        config.insert("model_enabled_yolo".to_string(), "true".to_string());
        config.insert("model_enabled_nsfw".to_string(), "true".to_string());
        config.insert("model_enabled_aesthetics".to_string(), "true".to_string());
        config.insert("model_enabled_face".to_string(), "true".to_string());
        config.insert("model_enabled_whisper".to_string(), "true".to_string());
        config
    }

    fn empty_models() -> LoadedModels {
        LoadedModels {
            clip_visual: None,
            clip_text: None,
            text_embeddings: Vec::new(),
            face_detector: None,
            arcface: None,
            ocr_det: None,
            ocr_rec: None,
            ocr_alphabet: Vec::new(),
            nsfw: None,
            aesthetics: None,
            yolo: None,
            blip: None,
            blip_decoder: None,
            blip_tokenizer: None,
            midas: None,
            whisper_encoder: None,
            whisper_decoder: None,
            whisper_tokenizer: None,
            known_people: Vec::new(),
            known_people_named: 0,
            selected_ep: "cpu".to_string(),
        }
    }

    // ── is_video_file ───────────────────────────────────────────

    #[test]
    fn test_is_video_file_mp4() {
        assert!(is_video_file("video.mp4"));
        assert!(is_video_file("VIDEO.MP4"));
    }

    #[test]
    fn test_is_video_file_mkv() {
        assert!(is_video_file("movie.mkv"));
    }

    #[test]
    fn test_is_video_file_mov() {
        assert!(is_video_file("clip.mov"));
    }

    #[test]
    fn test_is_video_file_avi() {
        assert!(is_video_file("old.avi"));
    }

    #[test]
    fn test_is_video_file_webm() {
        assert!(is_video_file("web.webm"));
    }

    #[test]
    fn test_is_video_file_flv() {
        assert!(is_video_file("flash.flv"));
    }

    #[test]
    fn test_is_video_file_wmv() {
        assert!(is_video_file("windows.wmv"));
    }

    #[test]
    fn test_is_video_file_m4v() {
        assert!(is_video_file("apple.m4v"));
    }

    #[test]
    fn test_is_video_file_ts() {
        assert!(is_video_file("stream.ts"));
    }

    #[test]
    fn test_is_video_file_not_image() {
        assert!(!is_video_file("photo.jpg"));
        assert!(!is_video_file("photo.png"));
        assert!(!is_video_file("photo.heic"));
        assert!(!is_video_file("photo.webp"));
    }

    #[test]
    fn test_is_video_file_not_video_extension() {
        assert!(!is_video_file("archive.tar.gz"));
        assert!(!is_video_file("document.pdf"));
    }

    // ── aggregate_frame_results ─────────────────────────────────

    #[test]
    fn test_aggregate_objects_picks_highest_confidence() {
        let f1 = PhotoResult {
            objects: vec![("cat".into(), "0.80".into()), ("dog".into(), "0.60".into())],
            ..Default::default()
        };
        let f2 = PhotoResult {
            objects: vec![
                ("cat".into(), "0.95".into()),
                ("bird".into(), "0.70".into()),
            ],
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);

        let cat = merged.objects.iter().find(|(c, _)| c == "cat").unwrap();
        assert_eq!(cat.1, "0.95", "should pick highest confidence for cat");

        let dog = merged.objects.iter().find(|(c, _)| c == "dog").unwrap();
        assert_eq!(dog.1, "0.60");

        let bird = merged.objects.iter().find(|(c, _)| c == "bird").unwrap();
        assert_eq!(bird.1, "0.70");
    }

    #[test]
    fn test_aggregate_aesthetics_averages() {
        let f1 = PhotoResult {
            aesthetics: Some(3.0),
            ..Default::default()
        };
        let f2 = PhotoResult {
            aesthetics: Some(5.0),
            ..Default::default()
        };
        let f3 = PhotoResult {
            aesthetics: None,
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2, f3], 3);
        assert_eq!(merged.aesthetics, Some(4.0));
    }

    #[test]
    fn test_aggregate_aesthetics_all_none() {
        let f1 = PhotoResult {
            aesthetics: None,
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1], 1);
        assert_eq!(merged.aesthetics, None);
    }

    #[test]
    fn test_aggregate_nsfw_picks_max() {
        let f1 = PhotoResult {
            nsfw: Some("0.3".into()),
            ..Default::default()
        };
        let f2 = PhotoResult {
            nsfw: Some("0.9".into()),
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);
        assert_eq!(merged.nsfw, Some("0.9".into()));
    }

    #[test]
    fn test_aggregate_nsfw_all_zero() {
        let f1 = PhotoResult {
            nsfw: Some("0.0".into()),
            ..Default::default()
        };
        let f2 = PhotoResult {
            nsfw: Some("0.0".into()),
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);
        assert_eq!(merged.nsfw, None, "all-zero nsfw should produce None");
    }

    #[test]
    fn test_aggregate_ocr_unique_texts() {
        let f1 = PhotoResult {
            ocr: Some("Hello".into()),
            ..Default::default()
        };
        let f2 = PhotoResult {
            ocr: Some("Hello".into()),
            ..Default::default()
        };
        let f3 = PhotoResult {
            ocr: Some("World".into()),
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2, f3], 3);
        // Should keep first unique text
        assert!(merged.ocr.is_some());
        let ocr = merged.ocr.as_ref().unwrap();
        assert!(ocr == "Hello" || ocr == "World");
    }

    #[test]
    fn test_aggregate_ocr_empty_text_ignored() {
        let f1 = PhotoResult {
            ocr: Some("  ".into()),
            ..Default::default()
        };
        let f2 = PhotoResult {
            ocr: None,
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);
        assert_eq!(merged.ocr, None);
    }

    #[test]
    fn test_aggregate_faces_dedup_identical_embeddings() {
        let embedding = vec![1.0, 0.0, 0.0];
        let face1 = FaceInfo {
            face_id: "p1_f0_10_20".into(),
            crop_path: "".into(),
            encoded: "".into(),
            embedding: embedding.clone(),
            person_id: None,
        };
        let face2 = FaceInfo {
            face_id: "p1_f1_30_40".into(),
            crop_path: "".into(),
            encoded: "".into(),
            embedding: embedding.clone(),
            person_id: None,
        };
        let f1 = PhotoResult {
            faces: vec![face1],
            ..Default::default()
        };
        let f2 = PhotoResult {
            faces: vec![face2],
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);
        assert_eq!(
            merged.faces.len(),
            1,
            "identical embeddings should be deduped"
        );
        assert_eq!(merged.face_count, 1);
    }

    #[test]
    fn test_aggregate_faces_dedup_different_embeddings() {
        let face1 = FaceInfo {
            face_id: "p1_f0_10_20".into(),
            crop_path: "".into(),
            encoded: "".into(),
            embedding: vec![1.0, 0.0, 0.0],
            person_id: None,
        };
        let face2 = FaceInfo {
            face_id: "p1_f1_30_40".into(),
            crop_path: "".into(),
            encoded: "".into(),
            embedding: vec![0.0, 1.0, 0.0],
            person_id: None,
        };
        let f1 = PhotoResult {
            faces: vec![face1],
            ..Default::default()
        };
        let f2 = PhotoResult {
            faces: vec![face2],
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);
        assert_eq!(
            merged.faces.len(),
            2,
            "orthogonal embeddings should not be deduped"
        );
    }

    #[test]
    fn test_aggregate_faces_similar_embeddings_dedup() {
        let base = [1.0, 0.0, 0.0];
        let similar: Vec<f32> = base.iter().map(|x| x * 0.99).collect();
        // Normalize both
        let norm_b: f32 = base.iter().map(|x| x * x).sum::<f32>().sqrt();
        let base: Vec<f32> = base.iter().map(|x| x / norm_b).collect();
        let norm_s: f32 = similar.iter().map(|x| x * x).sum::<f32>().sqrt();
        let similar: Vec<f32> = similar.iter().map(|x| x / norm_s).collect();

        let face1 = FaceInfo {
            face_id: "p1_f0_10_20".into(),
            crop_path: "".into(),
            encoded: "".into(),
            embedding: base,
            person_id: None,
        };
        let face2 = FaceInfo {
            face_id: "p1_f1_30_40".into(),
            crop_path: "".into(),
            encoded: "".into(),
            embedding: similar,
            person_id: None,
        };
        let f1 = PhotoResult {
            faces: vec![face1],
            ..Default::default()
        };
        let f2 = PhotoResult {
            faces: vec![face2],
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);
        assert_eq!(
            merged.faces.len(),
            1,
            "similar embeddings should be deduped"
        );
    }

    #[test]
    fn test_aggregate_empty_results() {
        let merged = aggregate_frame_results(&[], 0);
        assert!(merged.objects.is_empty());
        assert_eq!(merged.aesthetics, None);
        assert_eq!(merged.nsfw, None);
        assert_eq!(merged.ocr, None);
        assert_eq!(merged.face_count, 0);
    }

    #[test]
    fn test_aggregate_completed_models() {
        let f1 = PhotoResult {
            objects: vec![("cat".into(), "0.9".into())],
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1], 1);
        assert!(merged.completed_models.contains(&"clip"));
        assert!(merged.completed_models.contains(&"yolo"));
        assert!(merged.completed_models.contains(&"nsfw"));
        assert!(merged.completed_models.contains(&"aesthetics"));
    }

    #[test]
    fn test_aggregate_carries_caption_blip_ocr_midas() {
        let f1 = PhotoResult {
            completed_models: vec!["clip", "blip", "ocr", "midas"],
            caption: Some("a portrait of a man".into()),
            ..Default::default()
        };
        // Later frame with an empty caption must not override the real one.
        let f2 = PhotoResult {
            completed_models: vec!["clip", "blip"],
            caption: Some("   ".into()),
            ..Default::default()
        };
        let merged = aggregate_frame_results(&[f1, f2], 2);
        assert_eq!(merged.caption.as_deref(), Some("a portrait of a man"));
        assert!(merged.completed_models.contains(&"blip"));
        assert!(merged.completed_models.contains(&"ocr"));
        assert!(merged.completed_models.contains(&"midas"));
    }

    // ── analyze_video with no models (graceful no-op) ───────────

    #[test]
    fn test_analyze_video_no_models_no_crash() {
        let mut models = empty_models();
        let config = default_config();
        let ai = AiStatus::default();
        let tmp = tempfile::tempdir().unwrap();
        let faces_dir = tmp.path().to_str().unwrap();

        let result = analyze_video(
            "test_id",
            "/nonexistent/video.mp4",
            &ai,
            &mut models,
            &config,
            None,
            faces_dir,
        );
        assert!(result.transcript.is_none());
        assert!(result.objects.is_empty());
    }

    #[test]
    fn test_analyze_video_nonexistent_file() {
        let mut models = empty_models();
        let config = default_config();
        let ai = AiStatus::default();
        let tmp = tempfile::tempdir().unwrap();
        let faces_dir = tmp.path().to_str().unwrap();

        let result = analyze_video(
            "test_id",
            "/nonexistent/video.mp4",
            &ai,
            &mut models,
            &config,
            None,
            faces_dir,
        );
        assert!(result.transcript.is_none());
    }

    // ── analyze_photo with no models (graceful no-op) ───────────

    #[test]
    fn test_analyze_photo_nonexistent_file() {
        let mut models = empty_models();
        let config = default_config();
        let ai = AiStatus::default();
        let tmp = tempfile::tempdir().unwrap();
        let faces_dir = tmp.path().to_str().unwrap();

        let result = analyze_photo(
            "test_id",
            "/nonexistent/photo.jpg",
            &ai,
            &mut models,
            &config,
            None,
            faces_dir,
        );
        assert!(result.objects.is_empty());
    }

    // ── analyze_video with real video ───────────────────────────

    #[test]
    fn test_analyze_video_real_video_no_models() {
        let path = "/home/denzyl/Pictures/takeout-20260428T162732Z-3-001/Takeout/Google Photos/Moved to van der hoevenplein /VID_20171010_123456.mp4";
        if !std::path::Path::new(path).exists() {
            eprintln!("test video not found: {path}");
            return;
        }
        let mut models = empty_models();
        let config = default_config();
        let ai = AiStatus::default();
        let tmp = tempfile::tempdir().unwrap();
        let faces_dir = tmp.path().to_str().unwrap();

        let result = analyze_video(
            "test_video",
            path,
            &ai,
            &mut models,
            &config,
            None,
            faces_dir,
        );
        // Without models, whisper should still run if ffmpeg is available
        // (extract_audio returns Some), but no visual models run
        assert!(result.objects.is_empty());
    }

    // ── PhotoResult defaults ────────────────────────────────────

    #[test]
    fn test_photo_result_default() {
        let r = PhotoResult::default();
        assert!(r.objects.is_empty());
        assert_eq!(r.aesthetics, None);
        assert_eq!(r.nsfw, None);
        assert_eq!(r.ocr, None);
        assert_eq!(r.transcript, None);
        assert_eq!(r.face_count, 0);
        assert!(r.faces.is_empty());
        assert!(r.completed_models.is_empty());
        assert!(r.model_timings.is_empty());
    }
}
