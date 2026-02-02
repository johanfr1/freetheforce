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

/// Validate config against schema (local, no daemon required)
pub fn validate_local(namespace: &str, json_output: bool) -> Result<(), std::io::Error> {
    use std::fs;
    use std::path::PathBuf;

    // Get data dir
    let home = std::env::var("HOME").unwrap_or_default();
    let data_dir = PathBuf::from(&home).join(".localforge");
    let projects_dir = data_dir.join("config").join("projects");

    // Load config
    let config_path = projects_dir.join(format!("{}.toml", namespace));
    let schema_path = projects_dir.join(format!("{}.schema.json", namespace));

    if !config_path.exists() {
        if json_output {
            let output = serde_json::json!({
                "namespace": namespace,
                "valid": true,
                "message": "No config file exists"
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("No config file for namespace '{}'", namespace);
        }
        return Ok(());
    }

    if !schema_path.exists() {
        if json_output {
            let output = serde_json::json!({
                "namespace": namespace,
                "valid": true,
                "message": "No schema file (all values accepted)"
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("No schema for '{}' (all values accepted)", namespace);
        }
        return Ok(());
    }

    // Parse config
    let config_str = fs::read_to_string(&config_path)?;
    let config: toml::Value = toml::from_str(&config_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // Parse schema
    let schema_str = fs::read_to_string(&schema_path)?;
    let schema: serde_json::Value = serde_json::from_str(&schema_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let mut errors: Vec<String> = Vec::new();

    // Extract values from config
    if let Some(values) = config.get("values") {
        if let toml::Value::Table(table) = values {
            // Get schema properties
            let properties = schema.get("properties").and_then(|p| p.as_object());

            if let Some(props) = properties {
                for (key, value) in table {
                    if let Some(prop_schema) = props.get(key) {
                        // Validate type
                        let expected_type = prop_schema.get("type").and_then(|t| t.as_str());
                        let actual_type = toml_type_name(value);

                        if let Some(expected) = expected_type {
                            if !types_compatible(expected, actual_type) {
                                errors.push(format!(
                                    "Key '{}': expected type '{}', got '{}'",
                                    key, expected, actual_type
                                ));
                            }
                        }
                    }
                }
            }

            // Check required keys
            if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                for req_val in required {
                    if let Some(req_key) = req_val.as_str() {
                        if !table.contains_key(req_key) {
                            errors.push(format!("Missing required key: {}", req_key));
                        }
                    }
                }
            }
        }
    }

    if json_output {
        let output = serde_json::json!({
            "namespace": namespace,
            "valid": errors.is_empty(),
            "errors": errors
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else if errors.is_empty() {
        println!("✓ Config '{}' is valid", namespace);
    } else {
        println!("✗ Config '{}' has {} error(s):", namespace, errors.len());
        for err in &errors {
            println!("  - {}", err);
        }
    }

    Ok(())
}

fn toml_type_name(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Integer(_) => "integer",
        toml::Value::Float(_) => "number",
        toml::Value::String(_) => "string",
        toml::Value::Array(_) => "array",
        toml::Value::Table(_) => "object",
        toml::Value::Datetime(_) => "string",
    }
}

fn types_compatible(expected: &str, actual: &str) -> bool {
    match expected {
        "integer" => actual == "integer",
        "number" => actual == "integer" || actual == "number",
        _ => expected == actual,
    }
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

