use ort::session::Session;

pub fn build_session(path: &std::path::Path) -> Result<Session, String> {
    let mut builder = Session::builder()
        .map_err(|e| format!("Session builder error: {e}"))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Disable)
        .map_err(|e| format!("Optimization level error: {e}"))?;

    #[cfg(feature = "cuda")]
    {
        builder = builder
            .with_execution_providers([ort::ep::CUDA::default().build()])
            .map_err(|e| format!("CUDA EP error: {e}"))?;
    }

    #[cfg(all(not(feature = "cuda"), target_os = "windows"))]
    {
        builder = builder
            .with_execution_providers([ort::ep::DirectML::default().build()])
            .map_err(|e| format!("DirectML EP error: {e}"))?;
    }

    #[cfg(all(not(feature = "cuda"), target_os = "macos"))]
    {
        builder = builder
            .with_execution_providers([ort::ep::CoreML::default().build()])
            .map_err(|e| format!("CoreML EP error: {e}"))?;
    }

    builder
        .commit_from_file(path)
        .map_err(|e| format!("Model load error for {}: {e}", path.display()))
}

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
