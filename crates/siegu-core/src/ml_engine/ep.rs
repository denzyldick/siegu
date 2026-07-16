use ort::session::Session;

pub fn build_session(path: &std::path::Path) -> Result<Session, String> {
    let mut builder = Session::builder()
        .map_err(|e| format!("Session builder error: {e}"))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Disable)
        .map_err(|e| format!("Optimization level error: {e}"))?;

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
