use std::collections::HashMap;
use std::time::Instant;

use crate::database::{AiStatus, Face};
use crate::face_detector;
use crate::ml_worker::should_run_model;
use crate::thumbnail;

use super::models::{LoadedModels, ModelEngine};
use super::preprocessing;

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
    models: &mut LoadedModels,
    config: &HashMap<String, String>,
    target_model: Option<&str>,
    faces_dir: &str,
) -> PhotoResult {
    let mut result = PhotoResult::default();

    let dynamic_img = match image::open(location) {
        Ok(img) => img,
        Err(_) => return result,
    };

    let orientation = thumbnail::read_exif_orientation(location);
    let dynamic_img = thumbnail::apply_orientation(dynamic_img, orientation);
    let img = dynamic_img.to_rgb8();
    let (orig_w, orig_h) = (img.width() as f32, img.height() as f32);

    // CLIP Visual
    if should_run_model(target_model, "clip", Some(config)) && ai_status.clip == 0 {
        if let Some(ref visual_model) = models.clip_visual {
            let start = Instant::now();
            let input = preprocessing::clip_preprocess(&img);
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
            let input = preprocessing::aesthetics_preprocess(&img);
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
            let input = preprocessing::nsfw_preprocess(&img);
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
        if models.ocr_det.is_some() {
            if let Some(ref rec_model) = models.ocr_rec {
                if !models.ocr_alphabet.is_empty() {
                    let start = Instant::now();
                    let input = preprocessing::ocr_preprocess(&img);
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
            }
        }
    }

    // YOLOv8
    if should_run_model(target_model, "yolo", Some(config)) && ai_status.yolo == 0 {
        if let Some(ref model) = models.yolo {
            let start = Instant::now();
            let input = preprocessing::yolo_preprocess(&img);
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
            let input = preprocessing::face_preprocess(&img);
            if let Ok(data) = run_model(face_model, input, "input") {
                if data.len() >= 4420 * 6 {
                    let scores = &data[..4420 * 2];
                    let boxes = &data[4420 * 2..];
                    let anchors = face_detector::generate_anchors();
                    let mut proposals = Vec::new();
                    for i in 0..anchors.len() {
                        let score = scores[i * 2 + 1];
                        if score > 0.6 {
                            let loc = [
                                boxes[i * 4],
                                boxes[i * 4 + 1],
                                boxes[i * 4 + 2],
                                boxes[i * 4 + 3],
                            ];
                            let decoded = face_detector::decode(&loc, &anchors[i]);
                            proposals.push((decoded, score));
                        }
                    }
                    let keep = face_detector::nms(&mut proposals, 0.3);
                    for &idx in &keep {
                        let bbox = proposals[idx].0;
                        let xmin = (bbox[0] * orig_w).max(0.0) as u32;
                        let ymin = (bbox[1] * orig_h).max(0.0) as u32;
                        let xmax = (bbox[2] * orig_w).min(orig_w) as u32;
                        let ymax = (bbox[3] * orig_h).min(orig_h) as u32;
                        if xmax > xmin && ymax > ymin {
                            let (w, h) = (xmax - xmin, ymax - ymin);
                            if w > 20 && h > 20 {
                                let face_crop =
                                    image::imageops::crop_imm(&img, xmin, ymin, w, h).to_image();
                                let face_id = format!("{photo_id}_face_{xmin}_{ymin}");
                                let crop_path = format!("{faces_dir}/{face_id}.jpg");
                                if face_crop.save(&crop_path).is_ok() {
                                    let mut face_embedding = Vec::new();
                                    if let Some(ref model) = models.arcface {
                                        let f_resized = image::imageops::resize(
                                            &face_crop,
                                            112,
                                            112,
                                            image::imageops::FilterType::Triangle,
                                        );
                                        let f_input = preprocessing::arcface_preprocess(&f_resized);
                                        if let Ok(emb) = run_model(model, f_input, "input.1") {
                                            let mut e = emb;
                                            let norm: f32 =
                                                e.iter().map(|x| x * x).sum::<f32>().sqrt();
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
                                        let face_input =
                                            preprocessing::clip_preprocess(&face_resized);
                                        if let Ok(emb) =
                                            run_model(visual_model, face_input, "pixel_values")
                                        {
                                            let mut e = emb;
                                            let norm: f32 =
                                                e.iter().map(|x| x * x).sum::<f32>().sqrt();
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
                                        if highest_similarity > 0.75 {
                                            assigned_person_id = best_match_id;
                                        }
                                    }

                                    let mut buffer = std::io::Cursor::new(Vec::new());
                                    let _ =
                                        face_crop.write_to(&mut buffer, image::ImageFormat::Jpeg);
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

    // BLIP
    if should_run_model(target_model, "blip", Some(config)) && ai_status.blip == 0 {
        if let Some(ref model) = models.blip {
            let start = Instant::now();
            let input = preprocessing::blip_preprocess(&img);
            if let Ok(_data) = run_model(model, input, "pixel_values") {
                result.completed_models.push("blip");
                result
                    .model_timings
                    .insert("blip".to_string(), start.elapsed().as_secs_f64());
            }
        }
    }

    // MiDaS
    if should_run_model(target_model, "midas", Some(config)) && ai_status.midas == 0 {
        if let Some(ref model) = models.midas {
            let start = Instant::now();
            let input = preprocessing::midas_preprocess(&img);
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

fn run_model(
    model: &ModelEngine,
    input: ndarray::Array4<f32>,
    input_name: &str,
) -> Result<Vec<f32>, String> {
    let shape = input.shape().to_vec();
    let data = input.into_raw_vec_and_offset().0;
    let tensor = ort::value::Value::from_array((shape, data)).map_err(|e| e.to_string())?;
    let mut lock = model.lock().map_err(|e| e.to_string())?;
    let outputs = lock
        .run(ort::inputs![input_name => tensor])
        .map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for i in 0..outputs.len() {
        if let Ok((_shape, data)) = outputs[i].try_extract_tensor::<f32>() {
            results.extend_from_slice(data);
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

        if target_model.is_none() {
            db.update_photo_indexed(photo_id, 2);
        }
        let _ = db
            .connection
            .execute("UPDATE photo SET sync_needed = 1 WHERE id = ?1", [photo_id]);

        Ok(())
    });
}
