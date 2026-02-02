//! Socket server for handling client connections

use crate::api::types::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};
use crate::router::{route, DaemonContext};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[cfg(not(target_os = "windows"))]
use tokio::net::{UnixListener, UnixStream};

#[cfg(target_os = "windows")]
use tokio::net::{TcpListener, TcpStream};

/// Handle a single client connection
#[cfg(not(target_os = "windows"))]
pub async fn handle_connection(
    stream: UnixStream,
    ctx: Arc<DaemonContext>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            // Connection closed
            break;
        }

        let response = process_request(&line, &ctx);
        let response_json = serde_json::to_string(&response)?;

        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub async fn handle_connection(
    stream: TcpStream,
    ctx: Arc<DaemonContext>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break;
        }

        let response = process_request(&line, &ctx);
        let response_json = serde_json::to_string(&response)?;

        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

/// Process a single JSON-RPC request
fn process_request(line: &str, ctx: &DaemonContext) -> JsonRpcResponse {
    // Parse the request
    let request: JsonRpcRequest = match serde_json::from_str(line.trim()) {
        Ok(req) => req,
        Err(_) => {
            return JsonRpcResponse::error(None, JsonRpcError::parse_error());
        }
    };

    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return JsonRpcResponse::error(request.id, JsonRpcError::invalid_request());
    }

    // Route to handler
    route(ctx, &request)
}

/// Run the server loop
#[cfg(not(target_os = "windows"))]
pub async fn run_server(
    listener: UnixListener,
    ctx: Arc<DaemonContext>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Server started, waiting for connections...");

    loop {
        let (stream, _addr) = listener.accept().await?;
        let ctx = Arc::clone(&ctx);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, ctx).await {
                tracing::error!("Connection error: {}", e);
            }
        });
    }
}

#[cfg(target_os = "windows")]
pub async fn run_server(
    listener: TcpListener,
    ctx: Arc<DaemonContext>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Server started, waiting for connections...");

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::debug!("Connection from: {}", addr);

        let ctx = Arc::clone(&ctx);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, ctx).await {
                tracing::error!("Connection error: {}", e);
            }
        });
    }
}
