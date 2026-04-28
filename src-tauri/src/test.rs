#[cfg(test)]
mod tests {
    use crate::ml::ModelEngine;
    use ndarray::Array4;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    #[cfg(not(target_os = "android"))]
    use ort::{session::builder::GraphOptimizationLevel, session::Session};

    #[test]
    fn test_ai_pipeline_initialization() {
        // This test verifies that we can at least initialize the ORT environment
        #[cfg(not(target_os = "android"))]
        {
            let res = ort::init().with_name("siegu-test").commit();
            assert!(res.is_ok());
        }
    }

    #[test]
    #[ignore] // Manual run or CI only because it requires downloaded models
    fn test_full_inference_on_sample() {
        let models_dir = Path::new("test_models");
        let sample_photo = Path::new("../branding/logo.png");
        
        if !sample_photo.exists() {
            println!("Skipping inference test: sample photo not found");
            return;
        }

        let ultraface_path = models_dir.join("version-RFB-320.onnx");
        if !ultraface_path.exists() {
            println!("Skipping inference test: models not downloaded");
            return;
        }

        #[cfg(not(target_os = "android"))]
        {
            let session = Session::builder()
                .unwrap()
                .with_optimization_level(GraphOptimizationLevel::Disable)
                .unwrap()
                .commit_from_file(&ultraface_path)
                .expect("Failed to load ultraface");
            
            let model = ModelEngine::Ort(Arc::new(Mutex::new(session)));
            let input = Array4::<f32>::zeros((1, 3, 240, 320));
            let result = model.run(input, "input").expect("Inference failed");
            
            assert!(!result.is_empty());
            println!("Inference successful, output size: {}", result.len());
        }
    }
}
