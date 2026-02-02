//! forge can command

use crate::client::{call, ClientError};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct CanResponse {
    allowed: bool,
    reason: String,
}

pub async fn run(feature: &str, json_output: bool) -> Result<(), ClientError> {
    let response: CanResponse = call("entitlements.can", json!({ "feature": feature })).await?;

    if json_output {
        let output = serde_json::json!({
            "allowed": response.allowed,
            "reason": response.reason
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("allowed: {}", response.allowed);
        println!("reason:  {}", response.reason);
    }

    Ok(())
}
