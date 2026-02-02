//! IPC client for communicating with the daemon

use directories::BaseDirs;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[cfg(not(target_os = "windows"))]
use tokio::net::UnixStream;

#[cfg(target_os = "windows")]
use tokio::net::TcpStream;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Daemon not running. Start with 'forge-daemon'")]
    DaemonNotRunning,
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("RPC error: {message}")]
    Rpc { code: i32, message: String },
}

impl ClientError {
    pub fn exit_code(&self) -> std::process::ExitCode {
        match self {
            ClientError::DaemonNotRunning => std::process::ExitCode::from(3),
            ClientError::Rpc { .. } => std::process::ExitCode::from(1),
            _ => std::process::ExitCode::from(1),
        }
    }
}

/// JSON-RPC request
#[derive(Serialize)]
struct Request {
    jsonrpc: &'static str,
    id: u32,
    method: String,
    params: Value,
}

/// JSON-RPC response
#[derive(Deserialize)]
struct Response {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<Value>,
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

/// Get the data directory path
fn data_dir() -> Result<PathBuf, ClientError> {
    #[cfg(target_os = "windows")]
    {
        let base = BaseDirs::new().ok_or(ClientError::Connection("No home directory".into()))?;
        Ok(base.data_local_dir().join("LocalForge"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let base = BaseDirs::new().ok_or(ClientError::Connection("No home directory".into()))?;
        Ok(base.home_dir().join(".localforge"))
    }
}

/// Connect to the daemon
#[cfg(not(target_os = "windows"))]
async fn connect() -> Result<UnixStream, ClientError> {
    let socket_path = data_dir()?.join("forge.sock");

    if !socket_path.exists() {
        return Err(ClientError::DaemonNotRunning);
    }

    UnixStream::connect(&socket_path)
        .await
        .map_err(|_| ClientError::DaemonNotRunning)
}

#[cfg(target_os = "windows")]
async fn connect() -> Result<TcpStream, ClientError> {
    let endpoint_path = data_dir()?.join("endpoint.json");

    if !endpoint_path.exists() {
        return Err(ClientError::DaemonNotRunning);
    }

    let content = std::fs::read_to_string(&endpoint_path)?;

    #[derive(Deserialize)]
    struct Endpoint {
        host: String,
        port: u16,
    }

    let endpoint: Endpoint =
        serde_json::from_str(&content).map_err(|_| ClientError::DaemonNotRunning)?;

    let addr = format!("{}:{}", endpoint.host, endpoint.port);
    TcpStream::connect(&addr)
        .await
        .map_err(|_| ClientError::DaemonNotRunning)
}

/// Call a daemon RPC method
pub async fn call<T: DeserializeOwned>(method: &str, params: Value) -> Result<T, ClientError> {
    let stream = connect().await?;

    #[cfg(not(target_os = "windows"))]
    let (reader, mut writer) = stream.into_split();

    #[cfg(target_os = "windows")]
    let (reader, mut writer) = stream.into_split();

    // Build request
    let request = Request {
        jsonrpc: "2.0",
        id: 1,
        method: method.to_string(),
        params,
    };

    // Send request
    let request_json = serde_json::to_string(&request)?;
    writer.write_all(request_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    // Read response
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    // Parse response
    let response: Response = serde_json::from_str(&line)?;

    if let Some(error) = response.error {
        return Err(ClientError::Rpc {
            code: error.code,
            message: error.message,
        });
    }

    let result = response.result.unwrap_or(Value::Null);
    serde_json::from_value(result).map_err(|e| ClientError::Json(e))
}

/// Call a daemon RPC method and return raw JSON
pub async fn call_raw(method: &str, params: Value) -> Result<Value, ClientError> {
    call(method, params).await
}
