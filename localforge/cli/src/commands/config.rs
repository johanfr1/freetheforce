//! forge config commands

use crate::client::{call, ClientError};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn get(namespace: &str, key: &str, json_output: bool) -> Result<(), ClientError> {
    #[derive(Deserialize)]
    struct GetResponse {
        value: Option<Value>,
    }

    let response: GetResponse = call(
        "config.get",
        json!({ "namespace": namespace, "key": key }),
    )
    .await?;

    if json_output {
        let output = serde_json::json!({
            "namespace": namespace,
            "key": key,
            "value": response.value
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        match response.value {
            Some(v) => println!("{}", format_value(&v)),
            None => println!("(not set)"),
        }
    }

    Ok(())
}

pub async fn set(
    namespace: &str,
    key: &str,
    value: &str,
    json_output: bool,
) -> Result<(), ClientError> {
    // Try to parse the value as JSON, fall back to string
    let parsed_value: Value = serde_json::from_str(value).unwrap_or(Value::String(value.to_string()));

    call::<Value>(
        "config.set",
        json!({ "namespace": namespace, "key": key, "value": parsed_value }),
    )
    .await?;

    if json_output {
        let output = serde_json::json!({
            "namespace": namespace,
            "key": key,
            "value": parsed_value
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Set {}.{} = {}", namespace, key, format_value(&parsed_value));
    }

    Ok(())
}

pub async fn list(namespace: &str, json_output: bool) -> Result<(), ClientError> {
    #[derive(Deserialize)]
    struct ListResponse {
        entries: Value,
    }

    let response: ListResponse = call("config.list", json!({ "namespace": namespace })).await?;

    if json_output {
        let output = serde_json::json!({
            "namespace": namespace,
            "entries": response.entries
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        if let Value::Object(map) = response.entries {
            if map.is_empty() {
                println!("(no values)");
            } else {
                for (key, value) in map {
                    println!("{} = {}", key, format_value(&value));
                }
            }
        }
    }

    Ok(())
}

pub async fn reset(namespace: &str, json_output: bool) -> Result<(), ClientError> {
    call::<Value>("config.reset", json!({ "namespace": namespace })).await?;

    if json_output {
        let output = serde_json::json!({ "namespace": namespace, "reset": true });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Reset {} to defaults.", namespace);
    }

    Ok(())
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}
