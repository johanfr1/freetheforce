//! LocalForge Daemon Entry Point

use forge_daemon::platform::{DataDir, Transport};
use forge_daemon::router::DaemonContext;
use forge_daemon::server::run_server;
use std::sync::Arc;
use tokio::signal;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("forge_daemon=info".parse()?))
        .init();

    tracing::info!("LocalForge Daemon v{}", env!("CARGO_PKG_VERSION"));

    // Initialize data directory
    let data_dir = DataDir::new()?;
    data_dir.ensure_dirs()?;

    tracing::info!("Data directory: {}", data_dir.root().display());

    // Create daemon context
    let ctx = Arc::new(DaemonContext::new(data_dir.clone()));

    // Log startup
    let _ = ctx.log_writer.info("daemon", "Daemon starting");

    // Create listener
    let listener = Transport::listen(&data_dir).await?;

    // Spawn server task
    let server_ctx = Arc::clone(&ctx);
    let server_handle = tokio::spawn(async move {
        if let Err(e) = run_server(listener, server_ctx).await {
            tracing::error!("Server error: {}", e);
        }
    });

    // Wait for shutdown signal
    tracing::info!("Press Ctrl+C to stop");

    signal::ctrl_c().await?;

    tracing::info!("Shutting down...");

    // Log shutdown
    let _ = ctx.log_writer.info("daemon", "Daemon stopping");

    // Clean up transport
    Transport::cleanup(&data_dir);

    // Abort server (it's in an infinite loop)
    server_handle.abort();

    tracing::info!("Goodbye!");

    Ok(())
}
