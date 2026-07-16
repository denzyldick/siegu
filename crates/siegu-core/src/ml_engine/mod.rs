//! On-device ML inference engine using ONNX Runtime.
//!
//! Modules:
//! - `ep`: Execution provider selection (CUDA/DirectML/CoreML/CPU)
//! - `models`: Model loading and lifecycle management
//! - `pipeline`: Photo/video analysis pipeline (CLIP, face detection, OCR, etc.)
//! - `preprocessing`: Image preprocessing (resize, normalize) for models
//! - `whisper`: Audio transcription via Whisper tiny ONNX
//! - `worker`: Background worker thread that processes the analysis job queue

pub mod ep;
pub mod models;
pub mod pipeline;
pub mod preprocessing;
pub mod whisper;
pub mod worker;

pub use models::LoadedModels;
pub use pipeline::{FaceInfo, PhotoResult};
pub use worker::{AnalysisCallbacks, NoopCallbacks};
