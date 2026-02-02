//! forge init command

use crate::client::{call, ClientError};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdentityResponse {
    public_key: String,
    #[allow(dead_code)]
    created_at: String,
    #[allow(dead_code)]
    alias: Option<String>,
}

pub async fn run(json_output: bool) -> Result<(), ClientError> {
    let result: Result<IdentityResponse, _> = call("identity.init", json!({}) ).await;

    match result {
        Ok(identity) => {
            if json_output {
                let output = serde_json::json!({
                    "created": true,
                    "publicKey": identity.public_key
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                let display_key = format_display_key(&identity.public_key);
                println!("Created identity: {}", display_key);
                println!("Location: ~/.localforge/");
            }
            Ok(())
        }
        Err(ClientError::Rpc { code: -32002, .. }) => {
            // Identity already exists
            let identity: IdentityResponse = call("identity.get", json!({})).await?;

            if json_output {
                let output = serde_json::json!({
                    "created": false,
                    "publicKey": identity.public_key
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                let display_key = format_display_key(&identity.public_key);
                println!("Identity already exists: {}", display_key);
                println!("Nothing to do.");
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn format_display_key(key: &str) -> String {
    if key.len() > 12 {
        format!("ed25519:{}...{}", &key[..8], &key[key.len() - 4..])
    } else {
        format!("ed25519:{}", key)
    }
}
