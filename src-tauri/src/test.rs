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

    #[test]
    fn test_ai_pipeline_initialization() {
        #[cfg(not(target_os = "android"))]
        {
            assert!(ort::init().with_name("siegu-test").commit());
        }
    }

    #[cfg(not(target_os = "android"))]
    struct ModelSmokeCase {
        name: &'static str,
        file: &'static str,
        input_name: &'static str,
        shape: (usize, usize, usize, usize),
    }

    #[cfg(not(target_os = "android"))]
    fn test_models_dir() -> PathBuf {
        std::env::var_os("SIEGU_TEST_MODELS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("test_models"))
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
    fn load_model(path: &Path, name: &str) -> ModelEngine {
        assert_downloaded_model(path);
        let session = Session::builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::Disable)
            .unwrap()
            .commit_from_file(path)
            .unwrap_or_else(|e| panic!("failed to load {name} from {}: {e}", path.display()));

        ModelEngine::Ort(Arc::new(Mutex::new(session)))
    }

    #[cfg(not(target_os = "android"))]
    fn assert_image_model_infers(models_dir: &Path, case: ModelSmokeCase) {
        let model = load_model(&models_dir.join(case.file), case.name);
        let (_, _, height, width) = case.shape;
        let input = sample_image_tensor(width as u32, height as u32);
        let output = model
            .run(input, case.input_name)
            .unwrap_or_else(|e| panic!("{} inference failed: {e}", case.name));

        assert!(
            !output.is_empty(),
            "{} inference returned an empty tensor",
            case.name
        );
        assert!(
            output.iter().all(|value| value.is_finite()),
            "{} inference returned non-finite values",
            case.name
        );
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

    #[test]
    #[ignore = "downloads large production ONNX models; run in CI with SIEGU_TEST_MODELS_DIR"]
    fn test_full_inference_on_sample() {
        #[cfg(not(target_os = "android"))]
        {
            assert!(
                ort::init().with_name("siegu-model-smoke-test").commit(),
                "failed to initialize ONNX Runtime"
            );

            let models_dir = test_models_dir();
            assert_clip_text_infers(&models_dir);

            let cases = [
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
                    input_name: "pixel_values",
                    shape: (1, 3, 224, 224),
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
                    input_name: "data",
                    shape: (1, 3, 112, 112),
                },
                ModelSmokeCase {
                    name: "midas",
                    file: "midas.onnx",
                    input_name: "0",
                    shape: (1, 3, 256, 256),
                },
            ];

            for case in cases {
                assert_image_model_infers(&models_dir, case);
            }

            // These files are part of the production download set. OCR detection and Whisper are
            // kept as download/load contract checks until the worker wires them into image/audio
            // inference paths with stable input preprocessing.
            assert_downloaded_model(&models_dir.join("ocr_det.onnx"));
            assert_downloaded_model(&models_dir.join("whisper.onnx"));
            assert_downloaded_model(&models_dir.join("en_dict.txt"));
        }
    }
}
