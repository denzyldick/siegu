use std::time::Duration;

use siegu_core::lan_server::{start_with_config, ServerConfig};

fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let token = std::env::var("SIEGU_SIGNAL_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());

    let runtime = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
    runtime.block_on(async move {
        let server = start_with_config(ServerConfig { port, token }).await;
        println!("siegu-signal listening on port {}", server.port);

        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
        println!("siegu-signal shutting down");

        // Give in-flight connections a moment to flush, then exit.
        tokio::time::sleep(Duration::from_millis(250)).await;
    });
}
