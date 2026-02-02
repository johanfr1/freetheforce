//! forge identity commands

use crate::client::{call, ClientError};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityResponse {
    public_key: String,
    created_at: String,
    alias: Option<String>,
}

pub async fn show(json_output: bool) -> Result<(), ClientError> {
    let identity: IdentityResponse = call("identity.get", json!({})).await?;

    if json_output {
        let output = serde_json::json!({
            "publicKey": identity.public_key,
            "createdAt": identity.created_at,
            "alias": identity.alias
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        let display_key = format_display_key(&identity.public_key);
        println!("Public Key: {}", display_key);
        println!("Created:    {}", identity.created_at);
        if let Some(alias) = identity.alias {
            println!("Alias:      {}", alias);
        }
    }

    Ok(())
}

pub async fn alias(name: &str, json_output: bool) -> Result<(), ClientError> {
    call::<serde_json::Value>("identity.setAlias", json!({ "alias": name })).await?;

    if json_output {
        let output = serde_json::json!({ "alias": name });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Alias set: {}", name);
    }

    Ok(())
}

pub async fn export() -> Result<(), ClientError> {
    #[derive(Deserialize)]
    struct ExportResponse {
        bundle: String,
    }

    let response: ExportResponse = call("identity.export", json!({})).await?;

    // Output to stdout
    println!("{}", response.bundle);
    eprintln!("Exported identity to stdout.");

    Ok(())
}

pub async fn import() -> Result<(), ClientError> {
    // Read from stdin
    eprintln!("Reading identity bundle from stdin...");

    let stdin = io::stdin();
    let mut bundle = String::new();

    for line in stdin.lock().lines() {
        bundle.push_str(&line?);
        bundle.push('\n');
    }

    let identity: IdentityResponse =
        call("identity.import", json!({ "bundle": bundle.trim() })).await?;

    let display_key = format_display_key(&identity.public_key);
    eprintln!("Imported identity: {}", display_key);

    Ok(())
}

fn format_display_key(key: &str) -> String {
    if key.len() > 12 {
        format!("ed25519:{}...{}", &key[..8], &key[key.len() - 4..])
    } else {
        format!("ed25519:{}", key)
    }
}
