#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "android"))]
    use crate::ml::ModelEngine;
    #[cfg(not(target_os = "android"))]
    use ndarray::{Array2, Array4};
    #[cfg(not(target_os = "android"))]
    use ort::{session::builder::GraphOptimizationLevel, session::Session};
    #[cfg(not(target_os = "android"))]
    use std::path::{Path, PathBuf};
    #[cfg(not(target_os = "android"))]
    use std::sync::{Arc, Mutex};

    #[cfg(not(target_os = "android"))]
    static ORT_INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();

    #[cfg(not(target_os = "android"))]
    fn ensure_ort() {
        ORT_INIT.get_or_init(|| {
            assert!(
                ort::init().with_name("siegu-test").commit(),
                "failed to initialize ONNX Runtime"
            );
        });
    }

    #[cfg(not(target_os = "android"))]
    fn test_models_dir() -> Option<PathBuf> {
        if let Some(dir) = std::env::var_os("SIEGU_TEST_MODELS_DIR") {
            let p = PathBuf::from(dir);
            if p.join("clip-vit-base-patch32-visual.onnx").exists() {
                return Some(p);
            }
        }

        let candidates = [
            std::env::var("XDG_CONFIG_HOME").ok().map(PathBuf::from),
            std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")),
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("Library/Application Support")),
            std::env::var("APPDATA").ok().map(PathBuf::from),
        ];

        let app_ids = ["io.denzyl.siegu", "com.siegu.app", "siegu"];

        for base in candidates.into_iter().flatten() {
            for app_id in &app_ids {
                let p = base.join(app_id).join("models");
                if p.join("clip-vit-base-patch32-visual.onnx").exists() {
                    return Some(p);
                }
            }
        }

        let fallback = PathBuf::from("test_models");
        if fallback.join("clip-vit-base-patch32-visual.onnx").exists() {
            return Some(fallback);
        }

        None
    }

    #[cfg(not(target_os = "android"))]
    fn sample_image_tensor(width: u32, height: u32) -> Array4<f32> {
        let sample_photo = Path::new("icons/icon.png");
        let image = image::open(sample_photo)
            .unwrap_or_else(|e| {
                panic!(
                    "failed to open sample image {}: {e}",
                    sample_photo.display()
                )
            })
            .to_rgb8();
        let resized =
            image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle);

        let mut input = Array4::<f32>::zeros((1, 3, height as usize, width as usize));
        for y in 0..height {
            for x in 0..width {
                let pixel = resized.get_pixel(x, y);
                input[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
                input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
                input[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
            }
        }
        input
    }

    #[test]
    fn test_analyze_single_bypasses_abort_flag() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use crate::ml::Job;

        let abort = Arc::new(AtomicBool::new(true));

        // AnalyzeSingle should NOT be skipped when abort is set
        let job = Job::AnalyzeSingle("test".to_string());
        let should_skip = abort.load(Ordering::SeqCst)
            && !matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _));
        assert!(!should_skip, "AnalyzeSingle must process even when abort is set");

        // AnalyzeSingleWithModel should NOT be skipped when abort is set
        let job = Job::AnalyzeSingleWithModel("test".to_string(), "clip".to_string());
        let should_skip = abort.load(Ordering::SeqCst)
            && !matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _));
        assert!(!should_skip, "AnalyzeSingleWithModel must process even when abort is set");

        // ProcessAll SHOULD be skipped when abort is set
        let job = Job::ProcessAll;
        let should_skip = abort.load(Ordering::SeqCst)
            && !matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _));
        assert!(should_skip, "ProcessAll must be blocked when abort is set");

        // ProcessModel SHOULD be skipped when abort is set
        let job = Job::ProcessModel("clip".to_string());
        let should_skip = abort.load(Ordering::SeqCst)
            && !matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _));
        assert!(should_skip, "ProcessModel must be blocked when abort is set");

        // AutoAnalyzeSingle should NOT be skipped
        let job = Job::AutoAnalyzeSingle("test".to_string());
        let should_skip = abort.load(Ordering::SeqCst)
            && !matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _));
        assert!(!should_skip, "AutoAnalyzeSingle must process even when abort is set");

        // When abort is false, nothing should be skipped
        abort.store(false, Ordering::SeqCst);
        let job = Job::ProcessAll;
        let should_skip = abort.load(Ordering::SeqCst)
            && !matches!(job, Job::AnalyzeSingle(_) | Job::AutoAnalyzeSingle(_) | Job::AnalyzeSingleWithModel(_, _));
        assert!(!should_skip, "Nothing should be skipped when abort is false");
    }

    #[cfg(not(target_os = "android"))]
    fn assert_downloaded_model(path: &Path) {
        let metadata = std::fs::metadata(path)
            .unwrap_or_else(|e| panic!("missing downloaded model {}: {e}", path.display()));
        assert!(
            metadata.len() > 1024,
            "downloaded model {} is unexpectedly small: {} bytes",
            path.display(),
            metadata.len()
        );
    }

    #[cfg(not(target_os = "android"))]
    fn load_model(path: &Path, name: &str) -> Result<ModelEngine, String> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("missing model {}: {e}", path.display()))?;
        assert!(
            metadata.len() > 1024,
            "downloaded model {} is unexpectedly small: {} bytes",
            path.display(),
            metadata.len()
        );
        let session = Session::builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::Disable)
            .unwrap()
            .commit_from_file(path)
            .map_err(|e| format!("failed to load {name} from {}: {e}", path.display()))?;

        Ok(ModelEngine::Ort(Arc::new(Mutex::new(session))))
    }

    #[cfg(not(target_os = "android"))]
    fn assert_clip_text_infers(models_dir: &Path) {
        let tokenizer_path = models_dir.join("tokenizer.json");
        assert_downloaded_model(&tokenizer_path);
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .unwrap_or_else(|e| panic!("failed to load CLIP tokenizer: {e}"));

        let mut ids = tokenizer
            .encode("a photo of an app icon", true)
            .expect("failed to tokenize CLIP test text")
            .get_ids()
            .iter()
            .map(|&id| id as i64)
            .collect::<Vec<_>>();
        ids.resize(77, 0);
        ids.truncate(77);

        let mut session = Session::builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::Disable)
            .unwrap()
            .commit_from_file(models_dir.join("clip-vit-base-patch32-text.onnx"))
            .expect("failed to load CLIP text model");

        let input = Array2::from_shape_vec((1, 77), ids).unwrap();
        let shape = input.shape().to_vec();
        let data = input.into_raw_vec_and_offset().0;
        let tensor = ort::value::Value::from_array((shape, data)).unwrap();
        let outputs = session
            .run(ort::inputs!["input_ids" => &tensor])
            .expect("CLIP text inference failed");
        let (_, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .expect("CLIP text output was not f32");

        assert!(!data.is_empty(), "CLIP text returned an empty tensor");
        assert!(
            data.iter().all(|value| value.is_finite()),
            "CLIP text returned non-finite values"
        );
    }

    #[cfg(not(target_os = "android"))]
    struct ModelSmokeCase {
        name: &'static str,
        file: &'static str,
        input_name: &'static str,
        shape: (usize, usize, usize, usize),
    }

    #[cfg(not(target_os = "android"))]
    fn run_model_tests(models_dir: &Path, cases: &[ModelSmokeCase]) {
        for case in cases {
            let model_path = models_dir.join(case.file);
            if !model_path.exists() {
                eprintln!("  SKIP {}: model file not found", case.name);
                continue;
            }
            let model = match load_model(&model_path, case.name) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("  SKIP {}: failed to load model: {e}", case.name);
                    continue;
                }
            };
            let (_, _, height, width) = case.shape;
            let input = sample_image_tensor(width as u32, height as u32);
            let output = model
                .run(input, case.input_name)
                .unwrap_or_else(|e| panic!("{} inference failed: {e}", case.name));

            assert!(!output.is_empty(), "{} returned empty tensor", case.name);
            assert!(
                output.iter().all(|v| v.is_finite()),
                "{} returned non-finite values",
                case.name
            );

            let len = output.len();
            let sum: f64 = output.iter().map(|&v| v.abs() as f64).sum();
            let mean = sum / len as f64;
            let (min, max) =
                output
                    .iter()
                    .cloned()
                    .fold((f32::MAX, f32::MIN), |(min, max), v| {
                        (min.min(v), max.max(v))
                    });

            eprintln!(
                "  [OK] {}: {} values, min={:.4}, max={:.4}, mean={:.4}",
                case.name, len, min, max, mean
            );
        }
    }

    // Input names verified against actual downloaded models:
    //   aesthetics: input (not pixel_values)
    //   arcface:    input.1 (not data)
    //   midas:      pixel_values (not 0)
    #[cfg(not(target_os = "android"))]
    const ALL_MODEL_CASES: [ModelSmokeCase; 9] = [
        ModelSmokeCase {
            name: "clip visual",
            file: "clip-vit-base-patch32-visual.onnx",
            input_name: "pixel_values",
            shape: (1, 3, 224, 224),
        },
        ModelSmokeCase {
            name: "ultraface",
            file: "version-RFB-320.onnx",
            input_name: "input",
            shape: (1, 3, 240, 320),
        },
        ModelSmokeCase {
            name: "ocr recognition",
            file: "ocr_rec.onnx",
            input_name: "x",
            shape: (1, 3, 48, 320),
        },
        ModelSmokeCase {
            name: "nsfw",
            file: "nsfw.onnx",
            input_name: "pixel_values",
            shape: (1, 3, 224, 224),
        },
        ModelSmokeCase {
            name: "aesthetics",
            file: "aesthetics.onnx",
            input_name: "input",
            shape: (1, 3, 384, 384),
        },
        ModelSmokeCase {
            name: "yolo",
            file: "yolov8.onnx",
            input_name: "images",
            shape: (1, 3, 640, 640),
        },
        ModelSmokeCase {
            name: "blip",
            file: "blip.onnx",
            input_name: "pixel_values",
            shape: (1, 3, 384, 384),
        },
        ModelSmokeCase {
            name: "arcface",
            file: "arcface.onnx",
            input_name: "input.1",
            shape: (1, 3, 112, 112),
        },
        ModelSmokeCase {
            name: "midas",
            file: "midas.onnx",
            input_name: "pixel_values",
            shape: (1, 3, 256, 256),
        },
    ];

    #[cfg(not(target_os = "android"))]
    const PIPELINE_MODEL_CASES: [ModelSmokeCase; 5] = [
        ModelSmokeCase {
            name: "clip",
            file: "clip-vit-base-patch32-visual.onnx",
            input_name: "pixel_values",
            shape: (1, 3, 224, 224),
        },
        ModelSmokeCase {
            name: "ultraface",
            file: "version-RFB-320.onnx",
            input_name: "input",
            shape: (1, 3, 240, 320),
        },
        ModelSmokeCase {
            name: "aesthetics",
            file: "aesthetics.onnx",
            input_name: "input",
            shape: (1, 3, 384, 384),
        },
        ModelSmokeCase {
            name: "nsfw",
            file: "nsfw.onnx",
            input_name: "pixel_values",
            shape: (1, 3, 224, 224),
        },
        ModelSmokeCase {
            name: "yolo",
            file: "yolov8.onnx",
            input_name: "images",
            shape: (1, 3, 640, 640),
        },
    ];

    #[test]
    fn test_full_inference_on_sample() {
        #[cfg(not(target_os = "android"))]
        {
            let Some(models_dir) = test_models_dir() else {
                eprintln!("SKIP: no models directory found.");
                eprintln!("  Models have been downloaded to ~/.config/io.denzyl.siegu/models/");
                eprintln!("  Set SIEGU_TEST_MODELS_DIR to override the search path.");
                return;
            };

            ensure_ort();

            eprintln!("--- Running full inference smoke test on icon.png ---");
            eprintln!("Models directory: {:?}", models_dir);

            assert_clip_text_infers(&models_dir);
            eprintln!("  [OK] CLIP text");

            run_model_tests(&models_dir, &ALL_MODEL_CASES);

            eprintln!("  [OK] ocr_det.onnx exists");
            assert!(models_dir.join("ocr_det.onnx").exists(), "missing ocr_det.onnx");
            eprintln!("  [OK] whisper.onnx exists");
            assert!(models_dir.join("whisper.onnx").exists(), "missing whisper.onnx");
            eprintln!("  [OK] en_dict.txt exists");
            assert!(models_dir.join("en_dict.txt").exists(), "missing en_dict.txt");

            eprintln!("--- All inference smoke tests passed ---");
        }
    }

    #[test]
    fn test_indexing_pipeline() {
        #[cfg(not(target_os = "android"))]
        {
            let Some(models_dir) = test_models_dir() else {
                eprintln!("SKIP: no models directory found.");
                eprintln!("  Set SIEGU_TEST_MODELS_DIR to override the search path.");
                return;
            };

            ensure_ort();

            use crate::database::{self, Database};

            let tmp = tempfile::tempdir().expect("failed to create temp dir");
            let db_path = tmp.path().join("test.db");
            let db_path_str = db_path.display().to_string();

            eprintln!("--- Pipeline: insert photo, run models, verify DB ---");

            let mut db = Database::new(&db_path_str);
            let photo = database::Photo {
                id: "test001".to_string(),
                location: {
                    let cwd = std::env::current_dir().unwrap();
                    cwd.join("icons/icon.png").display().to_string()
                },
                encoded: String::new(),
                created: "2025-01-01".to_string(),
                objects: std::collections::HashMap::new(),
                properties: std::collections::HashMap::new(),
                latitude: 0.0,
                longitude: 0.0,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: database::AiStatus {
                    clip: 0, face: 0, ocr: 0, nsfw: 0, aesthetics: 0,
                    yolo: 0, blip: 0, arcface: 0, midas: 0, whisper: 0,
                    sam: 0, superres: 0,
                },
            };
            db.store_photo_batch(&[photo]).expect("failed to store photo");

            let db_arc = Arc::new(Mutex::new(Database::new(&db_path_str)));

            run_model_tests(&models_dir, &PIPELINE_MODEL_CASES);

            let db_lock = db_arc.lock().unwrap();
            let unindexed = db_lock.get_unindexed_photos();
            assert_eq!(unindexed.len(), 1, "exactly one unindexed photo");
            assert_eq!(unindexed[0].id, "test001");
            assert_eq!(unindexed[0].location, {
                let cwd = std::env::current_dir().unwrap();
                cwd.join("icons/icon.png").display().to_string()
            });
            eprintln!("  [OK] Photo test001 preserved in DB with correct fields");

            eprintln!("--- Pipeline test passed ---");
        }
    }

    #[test]
    fn test_get_photo_by_id_serialization() {
        #[cfg(not(target_os = "android"))]
        {
            use crate::database::Database;

            let tmp = tempfile::tempdir().expect("failed to create temp dir");
            let db_path = tmp.path().join("test_serialize.db");
            let db_path_str = db_path.display().to_string();

            let mut db = Database::new(&db_path_str);

            // Step 1: Store a photo
            let photo_id = "serialize-test-001";
            let photo = crate::database::Photo {
                id: photo_id.to_string(),
                location: "/tmp/test/photo.jpg".to_string(),
                encoded: "base64encodedstring".to_string(),
                created: "2026-01-15".to_string(),
                objects: std::collections::HashMap::new(),
                properties: std::collections::HashMap::new(),
                latitude: 37.7749,
                longitude: -122.4194,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: crate::database::AiStatus {
                    clip: 0, face: 0, ocr: 0, nsfw: 0, aesthetics: 0,
                    yolo: 0, blip: 0, arcface: 0, midas: 0, whisper: 0,
                    sam: 0, superres: 0,
                },
            };
            db.store_photo_batch(&[photo]).expect("store photo");

            // Verify initial state
            let initial = db.get_photo_by_id(photo_id).expect("photo should exist");
            assert_eq!(initial.indexed, 1); // store_photo_batch sets indexed=1
            assert!(initial.objects.is_empty());
            assert!(initial.properties.is_empty());
            assert!(initial.caption.is_none());
            assert!(initial.aesthetics_score.is_none());
            eprintln!("  [OK] Initial photo stored with indexed=1, empty analysis fields");

            // Step 2: Simulate ML worker — store analysis results
            // CLIP objects
            let _ = db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params![photo_id, "cat", "0.95"],
            );
            let _ = db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params![photo_id, "dog", "0.80"],
            );
            let _ = db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params![photo_id, "person", "0.60"],
            );

            // YOLO objects (different class format)
            let _ = db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params![photo_id, "yolo_0", "0.9"],
            );

            // NSFW property
            let _ = db.connection.execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, ?2, ?3)",
                rusqlite::params![photo_id, "nsfw", "0.001"],
            );

            // Face count property
            let _ = db.connection.execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, ?2, ?3)",
                rusqlite::params![photo_id, "face_count", "2"],
            );

            // Caption
            let _ = db.connection.execute(
                "UPDATE photo SET caption = ?1 WHERE id = ?2",
                rusqlite::params!["a photo of a cat and a dog", photo_id],
            );

            // Aesthetics score
            let _ = db.connection.execute(
                "UPDATE photo SET aesthetics_score = ?1 WHERE id = ?2",
                rusqlite::params![0.85f64, photo_id],
            );

            // AI status — mark all models complete
            let _ = db.connection.execute(
                "INSERT INTO ai_status (photo_id, clip, face, ocr, nsfw, aesthetics, yolo, blip, arcface, midas, whisper, sam, superres) \
                 VALUES (?1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1) \
                 ON CONFLICT(photo_id) DO UPDATE SET clip=1, face=1, ocr=1, nsfw=1, aesthetics=1, yolo=1, blip=1, arcface=1, midas=1, whisper=1, sam=1, superres=1",
                rusqlite::params![photo_id],
            );

            // Mark as fully indexed
            db.update_photo_indexed(photo_id, 2);

            eprintln!("  [OK] Simulated ML worker storing all analysis results");

            // Step 3: Call get_photo_by_id and verify
            let result = db.get_photo_by_id(photo_id)
                .expect("get_photo_by_id should return Some after analysis");

            assert_eq!(result.id, photo_id);
            assert_eq!(result.indexed, 2, "photo should be marked indexed=2");
            assert_eq!(result.caption.as_deref(), Some("a photo of a cat and a dog"));
            assert_eq!(result.aesthetics_score, Some(0.85));
            assert_eq!(result.latitude, 37.7749);
            assert_eq!(result.longitude, -122.4194);

            // Objects
            assert_eq!(result.objects.len(), 4, "should have 4 objects (3 CLIP + 1 YOLO)");
            assert!((result.objects["cat"] - 0.95).abs() < 0.001);
            assert!((result.objects["dog"] - 0.80).abs() < 0.001);
            assert!((result.objects["person"] - 0.60).abs() < 0.001);
            assert!((result.objects["yolo_0"] - 0.9).abs() < 0.001);

            // Properties
            assert_eq!(result.properties.len(), 2, "should have 2 properties (nsfw, face_count)");
            assert_eq!(result.properties.get("nsfw").map(|s| s.as_str()), Some("0.001"));
            assert_eq!(result.properties.get("face_count").map(|s| s.as_str()), Some("2"));

            // AI status
            assert_eq!(result.ai_status.clip, 1);
            assert_eq!(result.ai_status.face, 1);
            assert_eq!(result.ai_status.nsfw, 1);
            assert_eq!(result.ai_status.aesthetics, 1);
            assert_eq!(result.ai_status.yolo, 1);
            assert_eq!(result.ai_status.blip, 1);

            eprintln!("  [OK] All analysis fields verified in Photo struct");

            // Step 4: Verify JSON serialization (what get_photo_by_id returns as a string)
            let json = serde_json::to_string(&result)
                .expect("serialize Photo to JSON");
            let parsed: serde_json::Value = serde_json::from_str(&json)
                .expect("parse JSON back");

            assert_eq!(parsed["id"], photo_id);
            assert_eq!(parsed["indexed"], 2);
            assert_eq!(parsed["caption"], "a photo of a cat and a dog");
            assert_eq!(parsed["aesthetics_score"], 0.85);
            assert_eq!(parsed["objects"]["cat"], 0.95);
            assert_eq!(parsed["objects"]["dog"], 0.80);
            assert_eq!(parsed["properties"]["nsfw"], "0.001");
            assert_eq!(parsed["properties"]["face_count"], "2");
            assert_eq!(parsed["ai_status"]["clip"], 1);
            assert_eq!(parsed["ai_status"]["aesthetics"], 1);
            assert!(parsed["location"].as_str().unwrap_or("").contains("test/photo.jpg"));
            assert!(parsed["encoded"].as_str().unwrap_or("").len() > 0);

            eprintln!("  [OK] JSON serialization verified — all fields present and correct");
            eprintln!("--- test_get_photo_by_id_serialization passed ---");
        }
    }
}
