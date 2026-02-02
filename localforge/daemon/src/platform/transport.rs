//! Platform-aware transport abstraction
//!
//! Linux: Unix domain socket
//! Windows: TCP on localhost with port stored in endpoint.json

use crate::platform::paths::DataDir;
use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to bind to address")]
    BindFailed,
    #[error("Failed to connect to daemon")]
    ConnectFailed,
    #[error("Endpoint file not found")]
    EndpointNotFound,
    #[error("Invalid endpoint file")]
    InvalidEndpoint,
}

/// Endpoint information for TCP transport (Windows)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpEndpoint {
    #[serde(rename = "type")]
    pub transport_type: String,
    pub host: String,
    pub port: u16,
}

impl TcpEndpoint {
    pub fn new(port: u16) -> Self {
        Self {
            transport_type: "tcp".to_string(),
            host: "127.0.0.1".to_string(),
            port,
        }
    }
}

/// Transport abstraction for cross-platform IPC
pub struct Transport;

impl Transport {
    /// Create a listener on the appropriate transport
    #[cfg(not(target_os = "windows"))]
    pub async fn listen(
        data_dir: &DataDir,
    ) -> Result<tokio::net::UnixListener, TransportError> {
        let socket_path = data_dir.socket_path();

        // Remove existing socket file if present
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        let listener = tokio::net::UnixListener::bind(&socket_path)?;
        tracing::info!("Listening on Unix socket: {}", socket_path.display());

        Ok(listener)
    }

    /// Create a listener on TCP (Windows)
    #[cfg(target_os = "windows")]
    pub async fn listen(
        data_dir: &DataDir,
    ) -> Result<tokio::net::TcpListener, TransportError> {
        // Try to bind to a random available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        // Write endpoint file
        let endpoint = TcpEndpoint::new(addr.port());
        let endpoint_path = data_dir.endpoint_path();
        let endpoint_json = serde_json::to_string_pretty(&endpoint)
            .map_err(|_| TransportError::InvalidEndpoint)?;
        std::fs::write(&endpoint_path, endpoint_json)?;

        tracing::info!("Listening on TCP: {}", addr);
        tracing::info!("Endpoint file: {}", endpoint_path.display());

        Ok(listener)
    }

    /// Connect to the daemon (Unix socket)
    #[cfg(not(target_os = "windows"))]
    pub async fn connect(
        data_dir: &DataDir,
    ) -> Result<tokio::net::UnixStream, TransportError> {
        let socket_path = data_dir.socket_path();

        if !socket_path.exists() {
            return Err(TransportError::EndpointNotFound);
        }

        let stream = tokio::net::UnixStream::connect(&socket_path)
            .await
            .map_err(|_| TransportError::ConnectFailed)?;

        Ok(stream)
    }

    /// Connect to the daemon (TCP on Windows)
    #[cfg(target_os = "windows")]
    pub async fn connect(
        data_dir: &DataDir,
    ) -> Result<tokio::net::TcpStream, TransportError> {
        let endpoint_path = data_dir.endpoint_path();

        if !endpoint_path.exists() {
            return Err(TransportError::EndpointNotFound);
        }

        let endpoint_json = std::fs::read_to_string(&endpoint_path)?;
        let endpoint: TcpEndpoint = serde_json::from_str(&endpoint_json)
            .map_err(|_| TransportError::InvalidEndpoint)?;

        let addr = format!("{}:{}", endpoint.host, endpoint.port);
        let stream = tokio::net::TcpStream::connect(&addr)
            .await
            .map_err(|_| TransportError::ConnectFailed)?;

        Ok(stream)
    }

    /// Clean up transport resources (remove socket file on Unix)
    #[cfg(not(target_os = "windows"))]
    pub fn cleanup(data_dir: &DataDir) {
        let socket_path = data_dir.socket_path();
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    /// Clean up transport resources (remove endpoint file on Windows)
    #[cfg(target_os = "windows")]
    pub fn cleanup(data_dir: &DataDir) {
        let endpoint_path = data_dir.endpoint_path();
        if endpoint_path.exists() {
            let _ = std::fs::remove_file(&endpoint_path);
        }
    }
}

/// Unified stream wrapper for cross-platform async read/write
pub enum DaemonStream {
    #[cfg(not(target_os = "windows"))]
    Unix(tokio::net::UnixStream),
    #[cfg(target_os = "windows")]
    Tcp(tokio::net::TcpStream),
}
