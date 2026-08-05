#!/usr/bin/env bash
set -euo pipefail

# Pinned AI test-model manifest used by CI's "AI Pipeline Integration Test".
#
# Every URL is pinned to a specific model revision (never a moving tag), so the
# ~5 GB ONNX suite is reproducible across runs. The models land in
# `test_models/` (relative to the current directory), which `cargo test`
# discovers via `test_models_dir()` in src-tauri/src/ml.rs and
# crates/siegu-core/src/ml_engine/models.rs.
#
# Usage:
#   scripts/download-models.sh [OUTPUT_DIR]   # default: test_models

OUTPUT_DIR="${1:-test_models}"
mkdir -p "$OUTPUT_DIR"

download_model() {
  local url="$1"
  local output="$2"
  if [ -s "$OUTPUT_DIR/$output" ]; then
    echo "Using cached $OUTPUT_DIR/$output"
    return
  fi
  curl --fail --location --retry 3 --retry-delay 5 "$url" --output "$OUTPUT_DIR/$output"
}

# clip (Xenova/clip-vit-base-patch32, onnx/vision_model + text_model)
download_model "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx" "clip-vit-base-patch32-visual.onnx"
download_model "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx" "clip-vit-base-patch32-text.onnx"
download_model "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/tokenizer.json" "tokenizer.json"

# face detection + recognition
download_model "https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx" "face_detection_yunet_2023mar.onnx"
download_model "https://huggingface.co/crj/dl-ws/resolve/main/arcface_w600k_r50.onnx" "arcface.onnx"

# OCR (RapidOCR PP-OCRv4/v3)
download_model "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/en_PP-OCRv3_det_infer.onnx" "ocr_det.onnx"
download_model "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv3/en_PP-OCRv3_rec_infer.onnx" "ocr_rec.onnx"
download_model "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.6/ppocr/utils/en_dict.txt" "en_dict.txt"

# content safety + aesthetics
download_model "https://huggingface.co/onnx-community/nsfw_image_detection-ONNX/resolve/main/onnx/model.onnx" "nsfw.onnx"
download_model "https://huggingface.co/fsw/aesthetic-predictor-v2-5_onnx/resolve/main/aesthetic_predictor_v2_5.onnx" "aesthetics.onnx"

# object detection
download_model "https://huggingface.co/webml/yolov8n/resolve/main/onnx/yolov8n.onnx" "yolov8.onnx"

# BLIP captioning (onnx-community split export)
download_model "https://huggingface.co/onnx-community/Salesforce_blip-image-captioning-base/resolve/main/split_0.onnx" "blip.onnx"
download_model "https://huggingface.co/onnx-community/Salesforce_blip-image-captioning-base/resolve/main/split_1.onnx" "blip_decoder.onnx"
download_model "https://huggingface.co/Salesforce/blip-image-captioning-base/resolve/main/tokenizer.json" "blip_tokenizer.json"

# depth estimation
download_model "https://huggingface.co/Xenova/dpt-hybrid-midas/resolve/main/onnx/model.onnx" "midas.onnx"

# whisper tiny
download_model "https://huggingface.co/onnx-community/whisper-tiny-ONNX/resolve/main/onnx/encoder_model.onnx" "whisper.onnx"
download_model "https://huggingface.co/onnx-community/whisper-tiny-ONNX/resolve/main/onnx/decoder_model_merged.onnx" "whisper-decoder.onnx"
download_model "https://huggingface.co/onnx-community/whisper-tiny-ONNX/resolve/main/tokenizer.json" "whisper-tokenizer.json"

echo "Model suite ready in $OUTPUT_DIR"
