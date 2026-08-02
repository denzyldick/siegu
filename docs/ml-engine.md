# ML Engine

The ML pipeline runs entirely on-device via ONNX Runtime. All 9 AI models (14 ONNX model files plus 4 auxiliary tokenizer/dictionary files) are loaded and executed locally — no cloud API calls.

## Architecture

```
crates/siegu-core/src/ml_engine/
├── mod.rs            # Re-exports
├── ep.rs             # Execution provider: CUDA/DML/CoreML/CPU
├── models.rs         # LoadedModels — 14 ONNX session handles
├── pipeline.rs       # Photo analysis pipeline (9 model groups)
├── preprocessing.rs  # Image preprocessing per model
├── whisper.rs        # Audio transcription + mel spectrogram
└── worker.rs         # Background job processor + AnalysisCallbacks
```

## Model Registry

18 files across 9 model groups, defined in `model_manager.rs`. The "expected size" in the registry is a *minimum-valid-size* guard, not the real file size; the sizes below are the actual download sizes:

| Model | Files | Approx Size | Purpose |
|-------|-------|-------------|---------|
| **clip** | visual (~335MB), text (~242MB), tokenizer.json (~2MB) | ~580MB | Semantic search embeddings |
| **face** | version-RFB-320.onnx (~1MB), arcface.onnx (~166MB) | ~167MB | Face detection + recognition/grouping (512-dim) |
| **ocr** | det (~2MB), rec (~9MB), en_dict.txt (~1KB) | ~11MB | PP-OCRv3 text recognition |
| **nsfw** | nsfw.onnx (~327MB) | ~327MB | Sensitive content detection |
| **aesthetics** | aesthetics.onnx (~1.6GB) | ~1.6GB | Photo quality scoring (1-10) |
| **yolo** | yolov8n.onnx (~12MB) | ~12MB | 80-class object detection |
| **blip** | encoder (~329MB), decoder (~170MB), blip_tokenizer.json (~0.5MB) | ~500MB | Image captioning |
| **midas** | midas.onnx (~120MB) | ~120MB | Depth estimation |
| **whisper** | encoder (~31MB), decoder (~113MB), tokenizer.json (~3.7MB) | ~148MB | Audio transcription |

**Total**: ~3.4GB on disk (varies slightly per platform/version)

### Download verification

Model files are verified against expected SHA-256 hashes after download. Files with mismatched hashes are deleted automatically.

## Preprocessing

Each model has model-specific input dimensions and normalization:

| Model | Input Size | Normalization |
|-------|-----------|---------------|
| CLIP | 224×224 | ImageNet mean/std |
| Aesthetics | 384×384 | [-1, 1] |
| NSFW | 224×224 | ImageNet mean/std |
| OCR | 320×48 (det) / varies (rec) | [0, 1] |
| YOLO | 640×640 | [0, 1] |
| BLIP | 384×384 | ImageNet mean/std |
| MiDaS | 256×256 | [0, 1] |
| ArcFace | 112×112 | [-1, 1] |
| UltraFace | 320×240 | [-1, 1] |

## Analysis Pipeline

For each photo, `analyze_single_photo()` runs enabled models sequentially:

1. **CLIP** — 512-dim embedding → stored for semantic search
2. **UltraFace** — Bounding boxes → crop → ArcFace → cluster → DB
3. **OCR** — Text detection → recognition → store
4. **NSFW** — Binary classification → skip flag
5. **Aesthetics** — Score 1-10 → `aesthetics_score` column
6. **YOLO** — 80 COCO classes, filtered at ≥0.5 confidence
7. **BLIP** — Greedy autoregressive caption (max 20 tokens)
8. **ArcFace** — 512-dim embedding for detected faces
9. **MiDaS** — Depth map (stored as binary blob)
10. **Whisper** — Audio transcription (video files only)

Results are flushed in batched transactions for performance.

## Audio Transcription

Whisper tiny processes audio from video files:

1. Extract audio frames via ffmpeg at 1s intervals
2. Compute Log-Mel spectrogram (80 mel bins, 3000 frames)
3. Run Whisper encoder → autoregressive decoder loop
4. Greedy token decoding with BOS/EOS handling

## Execution Providers

| Provider | Platform | Enabled when |
|----------|----------|-------------|
| CUDA | NVIDIA GPU | `OrtStrategy::Cuda` |
| DirectML | Windows GPU | `OrtStrategy::DirectML` |
| CoreML | Apple Silicon | feature flag |
| CPU | All | Always (fallback) |

Default is CPU. GPU providers are configured via `ep.rs` based on `ORT_STRATEGY` env var.

## Feature Flags

The ML engine is gated behind the `ml` feature (default on):

```toml
[features]
default = ["ml"]
ml = ["dep:ort", "dep:tokenizers"]
```

When disabled, all inference calls return no-ops.
