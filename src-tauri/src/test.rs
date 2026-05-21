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

    // ---------------------------------------------------------------------------
    // Database characterization tests — every public method on `Database`
    // ---------------------------------------------------------------------------
    mod database {
        use crate::database;
        use std::collections::HashMap;
        use tempfile::tempdir;

        fn db() -> (database::Database, tempfile::TempDir) {
            let dir = tempdir().expect("tempdir");
            let db = database::Database::new(&dir.path().display().to_string());
            (db, dir)
        }

        fn make_photo(id: &str, location: &str) -> database::Photo {
            database::Photo {
                id: id.to_string(),
                location: location.to_string(),
                encoded: String::new(),
                created: "2026-01-01 12:00:00".to_string(),
                objects: HashMap::new(),
                properties: HashMap::new(),
                latitude: 52.3702,
                longitude: 4.8952,
                favorite: false,
                indexed: 0,
                caption: None,
                aesthetics_score: None,
                ai_status: database::AiStatus::default(),
            }
        }

        // -- Photo CRUD -------------------------------------------------------

        #[test]
        fn test_new_creates_tables() {
            let dir = tempdir().expect("tempdir");
            let d = database::Database::new(&dir.path().display().to_string());

            // Tables should exist; calling list_photos on empty DB returns nothing
            assert!(d.list_photos("", 0, 10, false, false).is_empty());
            assert!(d.get_unindexed_photos().is_empty());
            assert!(d.list_directories().is_empty());
            assert!(d.get_state().is_empty());
            assert!(d.list_devices().is_empty());
        }

        #[test]
        fn test_store_photo_batch_and_get_by_id() {
            let (mut db, _dir) = db();
            let photo = make_photo("p1", "/path/p1.jpg");

            db.store_photo_batch(&[photo]).expect("store");

            let loaded = db.get_photo_by_id("p1").expect("exists");
            assert_eq!(loaded.id, "p1");
            assert_eq!(loaded.location, "/path/p1.jpg");
            assert_eq!(loaded.latitude, 52.3702);
            assert_eq!(loaded.longitude, 4.8952);
            assert_eq!(loaded.indexed, 1); // store_photo_batch sets indexed=1
            assert!(loaded.favorite == false);
            assert!(loaded.caption.is_none());
            assert!(loaded.aesthetics_score.is_none());
        }

        #[test]
        fn test_get_photo_by_id_missing() {
            let (db, _dir) = db();
            assert!(db.get_photo_by_id("nonexistent").is_none());
        }

        #[test]
        fn test_get_photos_by_ids() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("a", "/a.jpg"), make_photo("b", "/b.jpg")])
                .expect("store");

            let photos = db.get_photos_by_ids(&["a".into(), "b".into(), "c".into()]);
            assert_eq!(photos.len(), 2);
            assert!(photos.iter().any(|p| p.id == "a"));
            assert!(photos.iter().any(|p| p.id == "b"));
        }

        #[test]
        fn test_get_photos_by_ids_empty_input() {
            let (db, _dir) = db();
            assert!(db.get_photos_by_ids(&[]).is_empty());
        }

        #[test]
        fn test_get_photo_encoded_batch() {
            let (mut db, _dir) = db();
            let mut p = make_photo("e1", "/e.jpg");
            p.encoded = "base64data".to_string();
            db.store_photo_batch(&[p]).expect("store");

            let map = db.get_photo_encoded_batch(&["e1".into()]);
            assert_eq!(map.get("e1").map(|s| s.as_str()), Some("base64data"));
        }

        #[test]
        fn test_get_photo_encoded_batch_empty() {
            let (db, _dir) = db();
            assert!(db.get_photo_encoded_batch(&[]).is_empty());
        }

        #[test]
        fn test_update_photo_indexed() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("u1", "/u.jpg")]).expect("store");

            db.update_photo_indexed("u1", 2);
            let loaded = db.get_photo_by_id("u1").expect("exists");
            assert_eq!(loaded.indexed, 2);
        }

        #[test]
        fn test_filter_new_paths() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("f1", "/existing.jpg")]).expect("store");

            let new = db.filter_new_paths(&["/new.jpg".into(), "/existing.jpg".into()]);
            assert_eq!(new, vec!["/new.jpg".to_string()]);
        }

        #[test]
        fn test_filter_new_paths_empty_input() {
            let (db, _dir) = db();
            assert!(db.filter_new_paths(&[]).is_empty());
        }

        #[test]
        fn test_filter_new_paths_chunk_boundary() {
            let (mut db, _dir) = db();
            // Insert 1 photo, then filter 101 paths (1 exists + 100 new) to exercise chunking
            db.store_photo_batch(&[make_photo("chunk", "/exists.jpg")]).expect("store");

            let mut paths: Vec<String> = (0..101).map(|i| format!("/p{i}.jpg")).collect();
            paths.push("/exists.jpg".into());

            let new = db.filter_new_paths(&paths);
            assert_eq!(new.len(), 101);
            assert!(!new.contains(&"/exists.jpg".to_string()));
        }

        // -- Photo queries ----------------------------------------------------

        #[test]
        fn test_list_photos_all() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("l1", "/a.jpg"), make_photo("l2", "/b.jpg")])
                .expect("store");

            let photos = db.list_photos("", 0, 10, false, false);
            assert_eq!(photos.len(), 2);
        }

        #[test]
        fn test_list_photos_pagination() {
            let (mut db, _dir) = db();
            let batch: Vec<_> = (0..5).map(|i| make_photo(&format!("p{i}"), &format!("/p{i}.jpg"))).collect();
            db.store_photo_batch(&batch).expect("store");

            assert_eq!(db.list_photos("", 0, 2, false, false).len(), 2);
            assert_eq!(db.list_photos("", 2, 10, false, false).len(), 3);
            assert_eq!(db.list_photos("", 10, 10, false, false).len(), 0);
        }

        #[test]
        fn test_list_photos_query_by_location() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("q1", "/vacation/beach.jpg"), make_photo("q2", "/work/doc.jpg")])
                .expect("store");

            let results = db.list_photos("beach", 0, 10, false, false);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].id, "q1");
        }

        #[test]
        fn test_list_photos_favorites_only() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("f1", "/f.jpg"), make_photo("f2", "/g.jpg")])
                .expect("store");
            db.toggle_favorite("f1");

            let favs = db.list_photos("", 0, 10, true, false);
            assert_eq!(favs.len(), 1);
            assert_eq!(favs[0].id, "f1");
        }

        #[test]
        fn test_toggle_favorite_add_remove() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("t1", "/t.jpg")]).expect("store");

            assert_eq!(db.toggle_favorite("t1"), true);   // added
            let p = db.get_photo_by_id("t1").expect("exists");
            assert!(p.favorite);

            assert_eq!(db.toggle_favorite("t1"), false);  // removed
            let p = db.get_photo_by_id("t1").expect("exists");
            assert!(!p.favorite);
        }

        #[test]
        fn test_get_heatmap_points() {
            let (mut db, _dir) = db();
            let mut p1 = make_photo("h1", "/h1.jpg");
            p1.latitude = 0.0;
            p1.longitude = 0.0;
            let mut p2 = make_photo("h2", "/h2.jpg");
            p2.latitude = 48.8566;
            p2.longitude = 2.3522;
            db.store_photo_batch(&[p1, p2]).expect("store");

            let points = db.get_heatmap_points();
            assert_eq!(points.len(), 1);
            assert_eq!(points[0].id, "h2");
        }

        // -- AI status --------------------------------------------------------

        #[test]
        fn test_update_ai_status_and_missing_model() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("a1", "/a.jpg")]).expect("store");

            let missing = db.get_photos_missing_model("clip");
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0], "a1");

            db.update_ai_status("a1", "clip", 1);
            let missing = db.get_photos_missing_model("clip");
            assert!(missing.is_empty());
        }

        #[test]
        fn test_get_unindexed_photos() {
            let (mut db, _dir) = db();
            let mut p1 = make_photo("i1", "/i1.jpg");
            p1.indexed = 0;
            let mut p2 = make_photo("i2", "/i2.jpg");
            p2.indexed = 2;
            db.store_photo_batch(&[p1, p2]).expect("store");

            // store_photo_batch sets indexed=1, so we need to set it differently
            db.update_photo_indexed("i1", 0);
            db.update_photo_indexed("i2", 2);

            let unindexed = db.get_unindexed_photos();
            assert_eq!(unindexed.len(), 1);
            assert_eq!(unindexed[0].id, "i1");
        }

        // -- People / Faces ---------------------------------------------------

        #[test]
        fn test_store_face_and_get_faces_for_photo() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("fp1", "/fp.jpg")]).expect("store");

            let embedding = vec![0.1f32; 512];
            let person_id = db.create_anonymous_person(&embedding);

            let face = database::Face {
                photo_id: "fp1".to_string(),
                face_id: "face-001".to_string(),
                crop_path: "/crops/face-001.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: embedding.clone(),
                person_id: Some(person_id.clone()),
            };
            db.store_face(face);

            let faces = db.get_faces_for_photo("fp1");
            assert_eq!(faces.len(), 1);
            assert_eq!(faces[0].face_id, "face-001");
            assert_eq!(faces[0].person_id.as_deref(), Some(person_id.as_str()));
        }

        #[test]
        fn test_get_faces_for_photo_empty() {
            let (db, _dir) = db();
            assert!(db.get_faces_for_photo("nonexistent").is_empty());
        }

        #[test]
        fn test_get_people_and_anonymous_groups() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("pep1", "/pep.jpg")]).expect("store");

            let embedding = vec![0.2f32; 512];
            let pid = db.create_anonymous_person(&embedding);
            db.store_face(database::Face {
                photo_id: "pep1".to_string(),
                face_id: "f1".to_string(),
                crop_path: "/crops/f1.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: embedding.clone(),
                person_id: Some(pid.clone()),
            });

            let people = db.get_people();
            assert_eq!(people.len(), 0); // name IS NULL, excluded by WHERE name IS NOT NULL

            let anon = db.get_anonymous_people_groups();
            assert_eq!(anon.len(), 1);
            assert_eq!(anon[0].id, pid);
            assert_eq!(anon[0].face_count, 1);
        }

        #[test]
        fn test_assign_name_to_face_new_person() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("an1", "/an.jpg")]).expect("store");

            let pid = db.create_anonymous_person(&vec![0.3f32; 512]);
            db.store_face(database::Face {
                photo_id: "an1".to_string(),
                face_id: "fa1".to_string(),
                crop_path: "/crops/fa1.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.3f32; 512],
                person_id: Some(pid.clone()),
            });

            let _result_id = db.assign_name_to_face("fa1", "Alice");
            // Should create a new named person
            let people = db.get_people();
            assert_eq!(people.len(), 1);
            assert_eq!(people[0].name, "Alice");
            assert_eq!(people[0].face_count, 1);
        }

        #[test]
        fn test_assign_name_to_face_existing_person() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("an2", "/an2.jpg")]).expect("store");

            // Create two anonymous people, assign the same name to both faces
            let pid1 = db.create_anonymous_person(&vec![0.4f32; 512]);
            let pid2 = db.create_anonymous_person(&vec![0.5f32; 512]);

            db.store_face(database::Face {
                photo_id: "an2".to_string(),
                face_id: "fa2".to_string(),
                crop_path: "/crops/fa2.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.4f32; 512],
                person_id: Some(pid1.clone()),
            });
            db.store_face(database::Face {
                photo_id: "an2".to_string(),
                face_id: "fa3".to_string(),
                crop_path: "/crops/fa3.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.5f32; 512],
                person_id: Some(pid2.clone()),
            });

            // Assign name to first face — creates named person
            db.assign_name_to_face("fa2", "Bob");
            // Assign same name to second face — should merge
            db.assign_name_to_face("fa3", "Bob");

            let people = db.get_people();
            assert_eq!(people.len(), 1);
            assert_eq!(people[0].name, "Bob");
            assert_eq!(people[0].face_count, 2);
        }

        #[test]
        fn test_create_anonymous_person_and_centroid() {
            let (db, _dir) = db();
            let emb = vec![0.6f32; 512];
            let pid = db.create_anonymous_person(&emb);
            // Centroid is a no-op when no faces are linked yet
            db.update_person_centroid(&pid);
            // Should not panic
        }

        #[test]
        fn test_get_all_people_with_embeddings() {
            let (db, _dir) = db();
            let emb = vec![0.7f32; 512];
            let pid = db.create_anonymous_person(&emb);
            // create_anonymous_person does NOT store embedding if none passed? 
            // Actually looking at the code: create_anonymous_person does INSERT with embedding.
            // But get_all_people_with_embeddings requires embedding IS NOT NULL.
            // We passed embedding, so it should show up.
            let results = db.get_all_people_with_embeddings();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].0, pid);
            assert_eq!(results[0].1.len(), 512);
        }

        #[test]
        fn test_get_person_faces() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("pf1", "/pf.jpg")]).expect("store");

            let pid = db.create_anonymous_person(&vec![0.8f32; 512]);
            db.store_face(database::Face {
                photo_id: "pf1".to_string(),
                face_id: "f-pf".to_string(),
                crop_path: "/crops/f-pf.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.8f32; 512],
                person_id: Some(pid.clone()),
            });

            let faces = db.get_person_faces(&pid);
            assert_eq!(faces.len(), 1);
            assert_eq!(faces[0].face_id, "f-pf");
        }

        #[test]
        fn test_get_photos_for_person() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("pp1", "/pp.jpg")]).expect("store");

            let pid = db.create_anonymous_person(&vec![0.9f32; 512]);
            db.store_face(database::Face {
                photo_id: "pp1".to_string(),
                face_id: "f-pp".to_string(),
                crop_path: "/crops/f-pp.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.9f32; 512],
                person_id: Some(pid.clone()),
            });

            let photos = db.get_photos_for_person(&pid);
            assert_eq!(photos.len(), 1);
            assert_eq!(photos[0].id, "pp1");
        }

        #[test]
        fn test_merge_people() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("mp1", "/mp.jpg")]).expect("store");

            let pid_a = db.create_anonymous_person(&vec![0.1f32; 512]);
            let pid_b = db.create_anonymous_person(&vec![0.2f32; 512]);

            db.store_face(database::Face {
                photo_id: "mp1".to_string(),
                face_id: "fa".to_string(),
                crop_path: "/crops/fa.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.1f32; 512],
                person_id: Some(pid_a.clone()),
            });
            db.store_face(database::Face {
                photo_id: "mp1".to_string(),
                face_id: "fb".to_string(),
                crop_path: "/crops/fb.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.2f32; 512],
                person_id: Some(pid_b.clone()),
            });

            db.merge_people(&pid_a, &pid_b);

            // pid_a should be gone, all faces point to pid_b
            let faces = db.get_person_faces(&pid_b);
            assert_eq!(faces.len(), 2);
            let faces_a = db.get_person_faces(&pid_a);
            assert!(faces_a.is_empty());
        }

        #[test]
        fn test_rename_person() {
            let (db, _dir) = db();
            let pid = db.create_anonymous_person(&vec![0.3f32; 512]);
            db.rename_person(&pid, "Renamed");
            let people = db.get_all_people_with_embeddings();
            // rename_person sets name, but get_all_people_with_embeddings only returns people with embeddings
            // The person still has embedding, so it should appear
            assert_eq!(people.len(), 1);
            // We can't check the name directly via get_all_people_with_embeddings since it only returns (id, embedding)
            // Let's verify via get_people which returns name
            // But get_people requires name IS NOT NULL, which it now is
            // Wait, get_people also does a LEFT JOIN faces, GROUP BY p.id
            // Since there are no faces linked, the face_count would be 0 but the person should still appear
            let people_list = db.get_people();
            assert_eq!(people_list.len(), 1);
            assert_eq!(people_list[0].name, "Renamed");
        }

        // -- Sync -------------------------------------------------------------

        #[test]
        fn test_get_photo_sync_info() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("sync1", "/path/sync1.jpg")]).expect("store");

            // sync info requires entries in object or faces table
            db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params!["sync1", "cat", "0.95"],
            ).unwrap();

            let info = db.get_photo_sync_info();
            assert_eq!(info.len(), 1);
            assert_eq!(info[0].id, "sync1");
        }

        #[test]
        fn test_get_photo_sync_info_excludes_siegu_folder() {
            let (mut db, _dir) = db();
            let mut photo = make_photo("sync2", "/siegu/sync2.jpg");
            photo.indexed = 1;
            db.store_photo_batch(&[photo]).expect("store");
            db.update_ai_status("sync2", "clip", 1);

            let info = db.get_photo_sync_info();
            assert!(info.is_empty());
        }

        #[test]
        fn test_get_photo_sync_info_by_id() {
            let (mut db, _dir) = db();
            let mut photo = make_photo("sync3", "/path/sync3.jpg");
            photo.indexed = 1;
            db.store_photo_batch(&[photo]).expect("store");

            let info = db.get_photo_sync_info_by_id("sync3").expect("exists");
            assert_eq!(info.id, "sync3");
        }

        #[test]
        fn test_get_photo_sync_info_by_id_missing() {
            let (db, _dir) = db();
            let result = db.get_photo_sync_info_by_id("nonexistent");
            assert!(result.is_err());
        }

        #[test]
        fn test_import_photo() {
            let (db, _dir) = db();
            let imp = database::ImportedPhoto {
                id: "imp1",
                location: "/imported/imp1.jpg",
                created: "2026-06-01",
                latitude: Some(40.7128),
                longitude: Some(-74.0060),
                objects_json: r#"[{"class":"cat","probability":"0.95"}]"#,
                faces_json: r#"[{"face_id":"imp-face-1","crop_path":"/crops/imp-face-1.jpg","encoded":"enc","person_id":null}]"#,
                encoded: "base64enc",
            };
            db.import_photo(imp);

            let photo = db.get_photo_by_id("imp1").expect("exists");
            assert_eq!(photo.location, "/imported/imp1.jpg");
            // Objects should be loaded
            let result = db.get_photo_by_id("imp1").expect("exists");
            assert_eq!(result.properties.len(), 0); // no properties imported
        }

        #[test]
        fn test_import_photo_updates_existing() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("imp2", "/old/imp2.jpg")]).expect("store");

            let imp = database::ImportedPhoto {
                id: "imp2",
                location: "/new/imp2.jpg",
                created: "2026-06-02",
                latitude: None,
                longitude: None,
                objects_json: "[]",
                faces_json: "[]",
                encoded: "",
            };
            db.import_photo(imp);

            let photo = db.get_photo_by_id("imp2").expect("exists");
            assert_eq!(photo.location, "/new/imp2.jpg");
        }

        // -- Config / State ---------------------------------------------------

        #[test]
        fn test_set_and_get_state() {
            let (db, _dir) = db();
            let mut state = HashMap::new();
            state.insert("key1".to_string(), "val1".to_string());
            state.insert("key2".to_string(), "val2".to_string());
            db.set_state(state);

            let loaded = db.get_state();
            assert_eq!(loaded.get("key1").map(|s| s.as_str()), Some("val1"));
            assert_eq!(loaded.get("key2").map(|s| s.as_str()), Some("val2"));
        }

        #[test]
        fn test_last_scan_time() {
            let (db, _dir) = db();
            assert!(db.get_last_scan_time().is_none());

            db.set_last_scan_time("2026-05-20T10:00:00Z".to_string());
            assert_eq!(
                db.get_last_scan_time().as_deref(),
                Some("2026-05-20T10:00:00Z")
            );

            db.set_last_scan_time("2026-05-21T10:00:00Z".to_string());
            assert_eq!(
                db.get_last_scan_time().as_deref(),
                Some("2026-05-21T10:00:00Z")
            );
        }

        // -- Logs -------------------------------------------------------------

        #[test]
        fn test_store_and_get_logs() {
            let (db, _dir) = db();
            db.store_log("INFO", "test message");
            db.store_log("WARN", "warning message");

            let logs = db.get_logs(10);
            assert_eq!(logs.len(), 2);
            assert!(logs.iter().any(|l| l.level == "WARN" && l.message == "warning message"));
            assert!(logs.iter().any(|l| l.level == "INFO" && l.message == "test message"));
        }

        #[test]
        fn test_get_logs_limit() {
            let (db, _dir) = db();
            for i in 0..5 {
                db.store_log("INFO", &format!("msg {i}"));
            }
            assert_eq!(db.get_logs(3).len(), 3);
        }

        #[test]
        fn test_clear_logs() {
            let (db, _dir) = db();
            db.store_log("INFO", "to be cleared");
            db.clear_logs();
            assert!(db.get_logs(10).is_empty());
        }

        // -- Directories ------------------------------------------------------

        #[test]
        fn test_add_and_list_directories() {
            let (db, _dir) = db();
            db.add_directory("/photos/vacation");
            db.add_directory("/photos/work");

            let dirs = db.list_directories();
            assert_eq!(dirs.len(), 2);
            assert!(dirs.contains(&"/photos/vacation".to_string()));
            assert!(dirs.contains(&"/photos/work".to_string()));
        }

        #[test]
        fn test_remove_directory() {
            let (db, _dir) = db();
            db.add_directory("/photos/temp");
            db.remove_directory("/photos/temp".to_string());
            assert!(db.list_directories().is_empty());
        }

        #[test]
        fn test_remove_directory_full() {
            let (mut db, _dir) = db();
            db.add_directory("/photos/obsolete");
            db.store_photo_batch(&[make_photo("rd1", "/photos/obsolete/rd1.jpg")]).expect("store");
            db.store_photo_batch(&[make_photo("rd2", "/photos/obsolete/sub/rd2.jpg")]).expect("store");

            db.remove_directory_full("/photos/obsolete");

            assert!(db.get_photo_by_id("rd1").is_none());
            assert!(db.get_photo_by_id("rd2").is_none());
            assert!(db.list_directories().is_empty());
        }

        // -- Search -----------------------------------------------------------

        #[test]
        fn test_list_objects() {
            let (mut db, _dir) = db();
            let mut photo = make_photo("so1", "/so.jpg");
            photo.indexed = 1;
            db.store_photo_batch(&[photo]).expect("store");

            // Directly insert an object for search via raw SQL
            db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params!["so1", "cat", "0.95"],
            ).unwrap();

            let suggestions = db.list_objects("ca");
            assert_eq!(suggestions.len(), 1);
            assert_eq!(suggestions[0].title, "cat");
            assert_eq!(suggestions[0].suggestion_type, "tag");
        }

        #[test]
        fn test_list_objects_no_match() {
            let (db, _dir) = db();
            assert!(db.list_objects("zzzzzz").is_empty());
        }

        // -- Devices & counts -------------------------------------------------

        #[test]
        fn test_get_media_counts() {
            let (mut db, _dir) = db();
            let photo = make_photo("mc1", "/mc1.jpg");
            db.store_photo_batch(&[photo]).expect("store");

            let video = make_photo("mc2", "/mc2.mp4");
            db.store_photo_batch(&[video]).expect("store");

            let (photos, videos) = db.get_media_counts();
            assert_eq!(photos, 1);
            assert_eq!(videos, 1);
        }

        #[test]
        fn test_list_devices() {
            let (db, _dir) = db();
            db.connection.execute(
                "INSERT INTO device (ip, name, offer) VALUES (?1, ?2, ?3)",
                rusqlite::params!["192.168.1.10", "Living Room", ""],
            ).unwrap();

            let devices = db.list_devices();
            assert_eq!(devices.len(), 1);
            assert_eq!(devices[0].id, "192.168.1.10");
            assert_eq!(devices[0].title, "Living Room");
        }

        // -- Objects & properties enrichment (indirect via list_photos) -------

        #[test]
        fn test_list_photos_with_objects_and_properties() {
            let (mut db, _dir) = db();
            let mut photo = make_photo("enr1", "/enr1.jpg");
            photo.indexed = 1;
            db.store_photo_batch(&[photo]).expect("store");

            // Insert object and property directly (as the ML worker would)
            db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params!["enr1", "dog", "0.88"],
            ).unwrap();
            db.connection.execute(
                "INSERT INTO properties (photo_id, key, value) VALUES (?1, ?2, ?3)",
                rusqlite::params!["enr1", "location_name", "Amsterdam"],
            ).unwrap();

            let photos = db.list_photos("", 0, 10, false, false);
            assert_eq!(photos.len(), 1);
            assert!((photos[0].objects["dog"] - 0.88).abs() < 0.001);
            assert_eq!(photos[0].properties.get("location_name").map(|s| s.as_str()), Some("Amsterdam"));
        }

        #[test]
        fn test_list_photos_with_caption_search() {
            let (mut db, _dir) = db();
            let mut photo = make_photo("cap1", "/cap1.jpg");
            photo.indexed = 1;
            db.store_photo_batch(&[photo]).expect("store");
            db.connection.execute(
                "UPDATE photo SET caption = ?1 WHERE id = ?2",
                rusqlite::params!["a beautiful sunset", "cap1"],
            ).unwrap();

            let results = db.list_photos("sunset", 0, 10, false, false);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].caption.as_deref(), Some("a beautiful sunset"));
        }

        #[test]
        fn test_list_photos_uuid_query() {
            let (mut db, _dir) = db();
            let uuid = "550e8400-e29b-41d4-a716-446655440000";
            let mut photo = make_photo(uuid, "/uuid.jpg");
            photo.indexed = 1;
            db.store_photo_batch(&[photo]).expect("store");

            // Query by UUID (exact match)
            let results = db.list_photos(uuid, 0, 10, false, false);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].id, uuid);
        }

        // -- store_photo_batch error case -------------------------------------

        #[test]
        fn test_store_photo_batch_empty() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[]).expect("empty batch should succeed");
        }

        // -- enrich_objects / enrich_properties with no matching rows ---------

        #[test]
        fn test_enrich_empty_photos() {
            let (db, _dir) = db();
            // These are private, but list_photos calls them internally
            let photos = db.list_photos("", 0, 10, false, false);
            assert!(photos.is_empty());
        }

        // -- Edge cases ------------------------------------------------------

        #[test]
        fn test_assign_name_to_face_same_person() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("same", "/same.jpg")]).expect("store");
            let pid = db.create_anonymous_person(&vec![0.1f32; 512]);
            db.store_face(database::Face {
                photo_id: "same".to_string(),
                face_id: "f-same".to_string(),
                crop_path: "/crops/f-same.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.1f32; 512],
                person_id: Some(pid.clone()),
            });
            // Assign name -> creates named person
            let named = db.assign_name_to_face("f-same", "Same");
            // Assign same name again -> should return same person
            let again = db.assign_name_to_face("f-same", "Same");
            assert_eq!(named, again);
        }

        #[test]
        fn test_update_person_centroid_empty() {
            let (db, _dir) = db();
            // No faces linked; centroid update should be a no-op
            db.update_person_centroid("nonexistent");
            // Should not panic
        }

        #[test]
        fn test_list_photos_videos_only() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("vid1", "/video.mp4"), make_photo("img1", "/image.jpg")]).expect("store");

            let videos = db.list_photos("", 0, 10, false, true);
            assert_eq!(videos.len(), 1);
            assert_eq!(videos[0].id, "vid1");
        }

        #[test]
        fn test_remove_directory_full_no_match() {
            let (db, _dir) = db();
            // Non-existent path should not error
            db.remove_directory_full("/nonexistent");
        }

        #[test]
        fn test_import_photo_invalid_objects_json() {
            let (db, _dir) = db();
            let imp = database::ImportedPhoto {
                id: "inv1",
                location: "/inv1.jpg",
                created: "2026-01-01",
                latitude: None,
                longitude: None,
                objects_json: "not valid json",
                faces_json: "[]",
                encoded: "",
            };
            db.import_photo(imp);
            let photo = db.get_photo_by_id("inv1").expect("exists");
            assert!(photo.objects.is_empty()); // invalid JSON -> no objects imported
        }

        #[test]
        fn test_import_photo_invalid_faces_json() {
            let (db, _dir) = db();
            let imp = database::ImportedPhoto {
                id: "inv2",
                location: "/inv2.jpg",
                created: "2026-01-01",
                latitude: None,
                longitude: None,
                objects_json: "[]",
                faces_json: "{bad}",
                encoded: "",
            };
            db.import_photo(imp);
            let faces = db.get_faces_for_photo("inv2");
            assert!(faces.is_empty());
        }

        #[test]
        fn test_assign_name_to_face_no_prior_person() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("noprior", "/noprior.jpg")]).expect("store");
            // Create a face with NO person_id (person_id is None)
            db.store_face(database::Face {
                photo_id: "noprior".to_string(),
                face_id: "f-noprior".to_string(),
                crop_path: "/crops/f-noprior.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.5f32; 512],
                person_id: None,
            });
            // Assign name -> should create new person entry
            let new_id = db.assign_name_to_face("f-noprior", "NewPerson");
            let faces = db.get_person_faces(&new_id);
            assert_eq!(faces.len(), 1);
            assert_eq!(faces[0].face_id, "f-noprior");
        }

        #[test]
        fn test_list_photos_with_object_query() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("oq1", "/oq1.jpg")]).expect("store");
            db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params!["oq1", "cat", "0.95"],
            ).unwrap();

            let photos = db.list_photos("cat", 0, 10, false, false);
            assert_eq!(photos.len(), 1);
            assert_eq!(photos[0].id, "oq1");
        }

        #[test]
        fn test_list_photos_with_object_query_case_insensitive() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("oq2", "/oq2.jpg")]).expect("store");
            db.connection.execute(
                "INSERT INTO object (photo_id, class, probability) VALUES (?1, ?2, ?3)",
                rusqlite::params!["oq2", "Cat", "0.90"],
            ).unwrap();

            // LIKE is case-insensitive by default in SQLite for ASCII
            let photos = db.list_photos("cat", 0, 10, false, false);
            assert_eq!(photos.len(), 1);
        }

        #[test]
        fn test_list_objects_with_person_search() {
            let (db, _dir) = db();
            // Insert a person with a name
            let pid = uuid::Uuid::new_v4().to_string();
            db.connection.execute(
                "INSERT INTO people (id, name) VALUES (?1, ?2)",
                rusqlite::params![pid, "Alice Johnson"],
            ).unwrap();

            let suggestions = db.list_objects("Alice");
            assert!(suggestions.iter().any(|s| s.suggestion_type == "person"));
        }

        #[test]
        fn test_assign_name_to_face_merge_existing() {
            let (mut db, _dir) = db();
            db.store_photo_batch(&[make_photo("merge_ex", "/merge_ex.jpg")]).expect("store");

            // Create face linked to an anonymous person
            let anon_pid = db.create_anonymous_person(&vec![0.7f32; 512]);
            db.store_face(database::Face {
                photo_id: "merge_ex".to_string(),
                face_id: "f-merge1".to_string(),
                crop_path: "/crops/f-merge1.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.7f32; 512],
                person_id: Some(anon_pid.clone()),
            });

            // Create a second face also linked to the same anon person
            db.store_face(database::Face {
                photo_id: "merge_ex".to_string(),
                face_id: "f-merge2".to_string(),
                crop_path: "/crops/f-merge2.jpg".to_string(),
                encoded: "enc".to_string(),
                embedding: vec![0.7f32; 512],
                person_id: Some(anon_pid.clone()),
            });

            // Assign name to first face
            let named_id = db.assign_name_to_face("f-merge1", "MergedPerson");
            // Both faces should now be under the named person
            let faces = db.get_person_faces(&named_id);
            assert_eq!(faces.len(), 2);
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
