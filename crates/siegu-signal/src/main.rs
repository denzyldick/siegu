use std::time::Duration;

use siegu_core::lan_server::{start_with_config, ServerConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let token = std::env::var("SIEGU_SIGNAL_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());

    let web_dist = std::env::var("SIEGU_WEB_DIST_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from);

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let server = start_with_config(ServerConfig {
            port,
            token,
            web_dist,
        })
        .await;
        println!("siegu-signal listening on port {}", server.port);

        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
        println!("siegu-signal shutting down");

        // Give in-flight connections a moment to flush, then exit.
        tokio::time::sleep(Duration::from_millis(250)).await;
        Ok(())
    })
}
