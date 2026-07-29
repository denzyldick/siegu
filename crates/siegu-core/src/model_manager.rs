use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use sysinfo::System;
use tracing::info;

/// Known model files with their URLs and expected SHA-256 hashes.
/// Hashes are hex-encoded. Empty string means "skip verification".
pub struct ModelFile {
    pub model_name: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub expected_size: u64,
    pub sha256: &'static str,
}

pub const MODEL_REGISTRY: &[ModelFile] = &[
    ModelFile {
        model_name: "clip",
        filename: "clip-vit-base-patch32-visual.onnx",
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx",
        expected_size: 150_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "clip",
        filename: "clip-vit-base-patch32-text.onnx",
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx",
        expected_size: 40_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "clip",
        filename: "tokenizer.json",
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/tokenizer.json",
        expected_size: 1_000,
        sha256: "",
    },
    ModelFile {
        model_name: "ultraface",
        filename: "version-RFB-320.onnx",
        url: "https://raw.githubusercontent.com/Linzaer/Ultra-Light-Fast-Generic-Face-Detector-1MB/master/models/onnx/version-RFB-320.onnx",
        expected_size: 1_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "ocr",
        filename: "ocr_det.onnx",
        url: "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/en_PP-OCRv3_det_infer.onnx",
        expected_size: 2_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "ocr",
        filename: "ocr_rec.onnx",
        url: "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv3/en_PP-OCRv3_rec_infer.onnx",
        expected_size: 2_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "ocr",
        filename: "en_dict.txt",
        url: "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.6/ppocr/utils/en_dict.txt",
        expected_size: 1_000,
        sha256: "",
    },
    ModelFile {
        model_name: "nsfw",
        filename: "nsfw.onnx",
        url: "https://huggingface.co/onnx-community/nsfw_image_detection-ONNX/resolve/main/onnx/model.onnx",
        expected_size: 10_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "aesthetics",
        filename: "aesthetics.onnx",
        url: "https://huggingface.co/fsw/aesthetic-predictor-v2-5_onnx/resolve/main/aesthetic_predictor_v2_5.onnx",
        expected_size: 10_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "yolo",
        filename: "yolov8.onnx",
        url: "https://huggingface.co/webml/yolov8n/resolve/main/onnx/yolov8n.onnx",
        expected_size: 10_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "blip",
        filename: "blip.onnx",
        url: "https://huggingface.co/onnx-community/Salesforce_blip-image-captioning-base/resolve/main/split_0.onnx",
        expected_size: 340_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "blip",
        filename: "blip_decoder.onnx",
        url: "https://huggingface.co/onnx-community/Salesforce_blip-image-captioning-base/resolve/main/split_1.onnx",
        expected_size: 640_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "blip",
        filename: "blip_tokenizer.json",
        url: "https://huggingface.co/Salesforce/blip-image-captioning-base/resolve/main/tokenizer.json",
        expected_size: 500_000,
        sha256: "",
    },
    ModelFile {
        model_name: "arcface",
        filename: "arcface.onnx",
        url: "https://huggingface.co/crj/dl-ws/resolve/main/arcface_w600k_r50.onnx",
        expected_size: 10_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "midas",
        filename: "midas.onnx",
        url: "https://huggingface.co/Xenova/dpt-hybrid-midas/resolve/main/onnx/model.onnx",
        expected_size: 100_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "whisper",
        filename: "whisper.onnx",
        url: "https://huggingface.co/onnx-community/whisper-tiny-ONNX/resolve/main/onnx/encoder_model.onnx",
        expected_size: 32_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "whisper",
        filename: "whisper-decoder.onnx",
        url: "https://huggingface.co/onnx-community/whisper-tiny-ONNX/resolve/main/onnx/decoder_model_merged.onnx",
        expected_size: 118_000_000,
        sha256: "",
    },
    ModelFile {
        model_name: "whisper",
        filename: "whisper-tokenizer.json",
        url: "https://huggingface.co/onnx-community/whisper-tiny-ONNX/resolve/main/tokenizer.json",
        expected_size: 3_800_000,
        sha256: "",
    },
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub model: String,
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelStatus {
    pub name: String,
    pub downloaded: bool,
}

pub fn verify_sha256(path: &Path, expected_hash: &str) -> Result<bool, std::io::Error> {
    if expected_hash.is_empty() {
        return Ok(true);
    }
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let result = hex::encode(hasher.finalize());
    Ok(result == expected_hash)
}

pub fn check_models_downloaded(models_dir: &Path) -> Vec<String> {
    let mut downloaded = Vec::new();

    let clip_files = [
        "clip-vit-base-patch32-visual.onnx",
        "clip-vit-base-patch32-text.onnx",
        "tokenizer.json",
    ];
    let mut clip_ok = true;
    for name in clip_files {
        let p = models_dir.join(name);
        let min_size = match name {
            "clip-vit-base-patch32-visual.onnx" => 150 * 1024 * 1024,
            "clip-vit-base-patch32-text.onnx" => 40 * 1024 * 1024,
            _ => 1024,
        };
        if !p.exists() || p.metadata().map(|m| m.len()).unwrap_or(0) < min_size {
            clip_ok = false;
            break;
        }
    }
    if clip_ok {
        downloaded.push("clip".to_string());
    }

    let ultraface_path = models_dir.join("version-RFB-320.onnx");
    if ultraface_path.exists()
        && ultraface_path.metadata().map(|m| m.len()).unwrap_or(0) > 1024 * 1024
    {
        downloaded.push("ultraface".to_string());
    }

    let ocr_files = ["ocr_det.onnx", "ocr_rec.onnx"];
    let mut ocr_ok = true;
    for name in ocr_files {
        let p = models_dir.join(name);
        if !p.exists() || p.metadata().map(|m| m.len()).unwrap_or(0) < 1024 {
            ocr_ok = false;
            break;
        }
    }
    if ocr_ok {
        let dict_path = models_dir.join("en_dict.txt");
        if !dict_path.exists() {
            ocr_ok = false;
        }
    }
    if ocr_ok {
        downloaded.push("ocr".to_string());
    }

    for name in &["nsfw", "aesthetics", "yolo", "blip", "arcface", "midas"] {
        let filename = match *name {
            "yolo" => "yolov8.onnx",
            _ => &format!("{name}.onnx"),
        };
        if models_dir.join(filename).exists() {
            downloaded.push(name.to_string());
        }
    }

    let whisper_files = [
        "whisper.onnx",
        "whisper-decoder.onnx",
        "whisper-tokenizer.json",
    ];
    let whisper_ok = whisper_files.iter().all(|f| models_dir.join(f).exists());
    if whisper_ok {
        downloaded.push("whisper".to_string());
    }

    downloaded
}

pub fn resolve_files_for_models(models: &[String]) -> Vec<(&'static ModelFile, PathBuf)> {
    let mut result = Vec::new();
    for model in models {
        let m = model.to_lowercase();
        for entry in MODEL_REGISTRY {
            if entry.model_name == m {
                result.push((entry, PathBuf::from(entry.filename)));
            }
        }
    }
    result
}

pub fn total_model_disk_usage(models_dir: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(models_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

pub fn available_memory_bytes() -> u64 {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.available_memory()
}

pub fn models_fit_in_memory(model_sizes: &[(String, u64)]) -> bool {
    let available = available_memory_bytes();
    let total_needed: u64 = model_sizes.iter().map(|(_, size)| size).sum();
    let budget = available / 2;
    info!(
        "Memory check: need {} MB, budget {} MB (available {} MB)",
        total_needed / 1024 / 1024,
        budget / 1024 / 1024,
        available / 1024 / 1024
    );
    total_needed <= budget
}

pub fn model_sizes_on_disk(models_dir: &Path, model_names: &[String]) -> Vec<(String, u64)> {
    let mut sizes = Vec::new();
    for name in model_names {
        let mut total = 0u64;
        for entry in MODEL_REGISTRY {
            if entry.model_name == name {
                let p = models_dir.join(entry.filename);
                if let Ok(meta) = std::fs::metadata(&p) {
                    total += meta.len();
                }
            }
        }
        sizes.push((name.clone(), total));
    }
    sizes
}

pub fn all_model_status(models_dir: &Path) -> Vec<ModelStatus> {
    let downloaded = check_models_downloaded(models_dir);
    let all_models = [
        "clip",
        "ultraface",
        "ocr",
        "nsfw",
        "aesthetics",
        "yolo",
        "blip",
        "arcface",
        "midas",
        "whisper",
    ];
    all_models
        .iter()
        .map(|m| ModelStatus {
            name: m.to_string(),
            downloaded: downloaded.iter().any(|d| d == m),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_models_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = check_models_downloaded(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_resolve_files_for_clip() {
        let files = resolve_files_for_models(&["clip".to_string()]);
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_resolve_files_unknown_model() {
        let files = resolve_files_for_models(&["nonexistent".to_string()]);
        assert!(files.is_empty());
    }

    #[test]
    fn test_model_registry_has_entries() {
        assert!(MODEL_REGISTRY.len() >= 14);
    }

    #[test]
    fn test_total_disk_usage_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(total_model_disk_usage(dir.path()), 0);
    }

    #[test]
    fn test_available_memory_nonzero() {
        let mem = available_memory_bytes();
        assert!(mem > 0);
    }

    #[test]
    fn test_models_fit_in_memory_small() {
        let sizes = vec![("clip".to_string(), 1024 * 1024)];
        assert!(models_fit_in_memory(&sizes));
    }

    #[test]
    fn test_model_sizes_empty() {
        let dir = tempfile::tempdir().unwrap();
        let sizes = model_sizes_on_disk(dir.path(), &["clip".to_string()]);
        assert_eq!(sizes[0].1, 0);
    }

    #[test]
    fn test_all_model_status() {
        let dir = tempfile::tempdir().unwrap();
        let statuses = all_model_status(dir.path());
        assert_eq!(statuses.len(), 10);
        assert!(statuses.iter().all(|s| !s.downloaded));
    }

    #[test]
    fn test_verify_sha256_empty_hash_always_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello").unwrap();
        assert!(verify_sha256(&path, "").unwrap());
    }

    #[test]
    fn test_verify_sha256_correct_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello").unwrap();
        let hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(&path, hash).unwrap());
    }

    #[test]
    fn test_verify_sha256_wrong_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello").unwrap();
        assert!(!verify_sha256(
            &path,
            "0000000000000000000000000000000000000000000000000000000000000000"
        )
        .unwrap());
    }
}
