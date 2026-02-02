//! forge status command

use crate::client::{call, ClientError};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    version: String,
    status: String,
    uptime_seconds: u64,
    identity: Option<String>,
    grants_active: usize,
    grants_expired: usize,
}

pub async fn run(json_output: bool) -> Result<(), ClientError> {
    let response: StatusResponse = call("status", json!({})).await?;

    if json_output {
        let output = serde_json::json!({
            "version": response.version,
            "status": response.status,
            "uptimeSeconds": response.uptime_seconds,
            "identity": response.identity,
            "grantsActive": response.grants_active,
            "grantsExpired": response.grants_expired
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("LocalForge v{}", response.version);
        println!("Status:   {}", response.status);
        println!("Uptime:   {}", format_uptime(response.uptime_seconds));
        println!("Socket:   ~/.localforge/forge.sock");

        if let Some(identity) = response.identity {
            println!("Identity: {}", identity);
        } else {
            println!("Identity: (not initialized)");
        }

        println!(
            "Grants:   {} active, {} expired",
            response.grants_active, response.grants_expired
        );
    }

    Ok(())
}

fn format_uptime(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}
