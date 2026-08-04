//! ONNX Runtime session builder with hardware-specific execution providers.
//!
//! Selects the best available backend per platform:
//! - **CUDA** if the `cuda` feature is enabled (Linux/Windows with NVIDIA GPU)
//! - **DirectML** on Windows (AMD/Intel/NVIDIA)
//! - **CoreML** on macOS (Apple Neural Engine)
//! - **CPU** as the universal fallback

use ort::session::Session;

/// Builds an ORT session from an ONNX model file.
///
/// Graph optimization is disabled (`Disable`) to avoid issues with certain
/// model architectures (e.g., Whisper decoder with dynamic KV cache shapes).
/// Execution providers are tried in priority order; the first successful one wins.
#[allow(clippy::vec_init_then_push)]
pub fn build_session(path: &std::path::Path) -> Result<Session, String> {
    let mut builder = Session::builder()
        .map_err(|e| format!("Session builder error: {e}"))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Disable)
        .map_err(|e| format!("Optimization level error: {e}"))?
        .with_intra_threads(intra_threads())
        .map_err(|e| format!("Intra-op thread count error: {e}"))?;

    if let Some(inter) = inter_threads() {
        builder = builder
            .with_inter_threads(inter)
            .map_err(|e| format!("Inter-op thread count error: {e}"))?;
    }

    let mut eps: Vec<ort::ep::ExecutionProviderDispatch> = Vec::new();

    #[cfg(feature = "cuda")]
    {
        eps.push(ort::ep::CUDA::default().build());
    }

    #[cfg(all(not(feature = "cuda"), target_os = "windows"))]
    {
        eps.push(ort::ep::DirectML::default().build());
    }

    #[cfg(all(not(feature = "cuda"), target_os = "macos"))]
    {
        eps.push(ort::ep::CoreML::default().build());
    }

    eps.push(ort::ep::CPU::default().build());

    builder = builder
        .with_execution_providers(eps)
        .map_err(|e| format!("EP registration error: {e}"))?;

    builder
        .commit_from_file(path)
        .map_err(|e| format!("Model load error for {}: {e}", path.display()))
}

/// Returns a human-readable name for the active execution provider.
/// Used in the UI to show which hardware backend is being used.
pub fn selected_ep() -> String {
    #[cfg(feature = "cuda")]
    {
        "CUDA".to_string()
    }
    #[cfg(all(not(feature = "cuda"), target_os = "windows"))]
    {
        "DirectML".to_string()
    }
    #[cfg(all(not(feature = "cuda"), target_os = "macos"))]
    {
        "CoreML".to_string()
    }
    #[cfg(not(any(feature = "cuda", target_os = "windows", target_os = "macos")))]
    {
        "CPU".to_string()
    }
}

/// Intra-op threads per inference run.
///
/// Defaults to `min(physical cores, 4)`: a full-library index runs several
/// photos concurrently (see the session pool in `models`), so a single run
/// using every core would oversubscribe the machine. Override with
/// `SIEGU_ORT_THREADS`.
fn intra_threads() -> usize {
    env_threads("SIEGU_ORT_THREADS").unwrap_or_else(|| {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        cores.min(4)
    })
}

/// Inter-op threads for a run. ORT manages this internally by default; only
/// set it when the user explicitly asks via `SIEGU_ORT_INTER_THREADS`.
fn inter_threads() -> Option<usize> {
    env_threads("SIEGU_ORT_INTER_THREADS")
}

fn env_threads(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n >= 1)
}
