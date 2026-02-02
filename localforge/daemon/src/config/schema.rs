//! JSON Schema validation for config values
//!
//! Provides optional schema validation for config.set operations.
//! Schemas are stored co-located with config files.

use crate::platform::paths::DataDir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Schema not found for namespace: {0}")]
    NotFound(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

/// JSON Schema (simplified subset for Phase 0.5)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    #[serde(rename = "$schema", default)]
    pub schema_url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub value_type: Option<String>,
    #[serde(default)]
    pub properties: HashMap<String, PropertySchema>,
    #[serde(default)]
    pub required: Vec<String>,
}

/// Property schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(rename = "enum", default)]
    pub enum_values: Option<Vec<Value>>,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
    #[serde(rename = "minLength", default)]
    pub min_length: Option<usize>,
    #[serde(rename = "maxLength", default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub pattern: Option<String>,
}

/// Schema store for managing namespace schemas
pub struct SchemaStore {
    data_dir: DataDir,
}

impl SchemaStore {
    pub fn new(data_dir: DataDir) -> Self {
        Self { data_dir }
    }

    /// Load schema for a namespace (if exists)
    pub fn load(&self, namespace: &str) -> Result<Option<Schema>, SchemaError> {
        let path = self.data_dir.project_schema_path(namespace);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let schema: Schema = serde_json::from_str(&content)?;
        Ok(Some(schema))
    }

    /// Save schema for a namespace
    pub fn save(&self, namespace: &str, schema: &Schema) -> Result<(), SchemaError> {
        let path = self.data_dir.project_schema_path(namespace);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(schema)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Check if schema exists for namespace
    pub fn exists(&self, namespace: &str) -> bool {
        self.data_dir.project_schema_path(namespace).exists()
    }

    /// Validate a value against the schema for a namespace/key
    pub fn validate(
        &self,
        namespace: &str,
        key: &str,
        value: &Value,
    ) -> Result<(), SchemaError> {
        let schema = match self.load(namespace)? {
            Some(s) => s,
            None => return Ok(()), // No schema = accept anything
        };

        // Find property schema for this key
        let prop_schema = match schema.properties.get(key) {
            Some(ps) => ps,
            None => return Ok(()), // Key not in schema = accept (additionalProperties: true)
        };

        // Validate type
        let actual_type = json_type_name(value);
        let expected_type = &prop_schema.value_type;

        // Handle "integer" vs "number" compatibility
        let type_ok = match expected_type.as_str() {
            "integer" => actual_type == "integer" || (actual_type == "number" && is_integer(value)),
            "number" => actual_type == "integer" || actual_type == "number",
            _ => actual_type == expected_type,
        };

        if !type_ok {
            return Err(SchemaError::ValidationFailed(format!(
                "Key '{}' expected type '{}', got '{}'",
                key, expected_type, actual_type
            )));
        }

        // Validate enum
        if let Some(ref enum_values) = prop_schema.enum_values {
            if !enum_values.contains(value) {
                return Err(SchemaError::ValidationFailed(format!(
                    "Key '{}' value not in allowed enum: {:?}",
                    key, enum_values
                )));
            }
        }

        // Validate numeric ranges
        if let Some(min) = prop_schema.minimum {
            if let Some(n) = value.as_f64() {
                if n < min {
                    return Err(SchemaError::ValidationFailed(format!(
                        "Key '{}' value {} is less than minimum {}",
                        key, n, min
                    )));
                }
            }
        }

        if let Some(max) = prop_schema.maximum {
            if let Some(n) = value.as_f64() {
                if n > max {
                    return Err(SchemaError::ValidationFailed(format!(
                        "Key '{}' value {} is greater than maximum {}",
                        key, n, max
                    )));
                }
            }
        }

        // Validate string length
        if let Some(s) = value.as_str() {
            if let Some(min) = prop_schema.min_length {
                if s.len() < min {
                    return Err(SchemaError::ValidationFailed(format!(
                        "Key '{}' string length {} is less than minLength {}",
                        key,
                        s.len(),
                        min
                    )));
                }
            }

            if let Some(max) = prop_schema.max_length {
                if s.len() > max {
                    return Err(SchemaError::ValidationFailed(format!(
                        "Key '{}' string length {} is greater than maxLength {}",
                        key,
                        s.len(),
                        max
                    )));
                }
            }
        }

        Ok(())
    }

    /// Validate all values in a config
    pub fn validate_all(
        &self,
        namespace: &str,
        values: &HashMap<String, Value>,
    ) -> Result<Vec<String>, SchemaError> {
        let schema = match self.load(namespace)? {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        let mut errors = Vec::new();

        // Check required keys
        for required_key in &schema.required {
            if !values.contains_key(required_key) {
                errors.push(format!("Missing required key: {}", required_key));
            }
        }

        // Validate each value
        for (key, value) in values {
            if let Err(e) = self.validate(namespace, key, value) {
                errors.push(e.to_string());
            }
        }

        Ok(errors)
    }
}

/// Get JSON type name
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Check if a JSON number is an integer
fn is_integer(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.is_i64() || n.is_u64(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, SchemaStore) {
        let tmp = TempDir::new().unwrap();
        let data_dir = crate::platform::paths::DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();
        (tmp, SchemaStore::new(data_dir))
    }

    #[test]
    fn test_no_schema_accepts_all() {
        let (_tmp, store) = temp_store();

        // No schema = accept any value
        let result = store.validate("myapp", "anything", &serde_json::json!("hello"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_type_validation() {
        let (_tmp, store) = temp_store();

        let schema = Schema {
            schema_url: None,
            title: Some("Test".into()),
            description: None,
            value_type: Some("object".into()),
            properties: {
                let mut p = HashMap::new();
                p.insert(
                    "timeout".into(),
                    PropertySchema {
                        value_type: "integer".into(),
                        description: None,
                        default: None,
                        enum_values: None,
                        minimum: Some(1.0),
                        maximum: Some(3600.0),
                        min_length: None,
                        max_length: None,
                        pattern: None,
                    },
                );
                p
            },
            required: vec![],
        };

        store.save("myapp", &schema).unwrap();

        // Valid integer
        let result = store.validate("myapp", "timeout", &serde_json::json!(30));
        assert!(result.is_ok());

        // Wrong type
        let result = store.validate("myapp", "timeout", &serde_json::json!("thirty"));
        assert!(matches!(result, Err(SchemaError::ValidationFailed(_))));

        // Out of range
        let result = store.validate("myapp", "timeout", &serde_json::json!(0));
        assert!(matches!(result, Err(SchemaError::ValidationFailed(_))));
    }

    #[test]
    fn test_enum_validation() {
        let (_tmp, store) = temp_store();

        let schema = Schema {
            schema_url: None,
            title: None,
            description: None,
            value_type: Some("object".into()),
            properties: {
                let mut p = HashMap::new();
                p.insert(
                    "log_level".into(),
                    PropertySchema {
                        value_type: "string".into(),
                        description: None,
                        default: None,
                        enum_values: Some(vec![
                            serde_json::json!("debug"),
                            serde_json::json!("info"),
                            serde_json::json!("warn"),
                            serde_json::json!("error"),
                        ]),
                        minimum: None,
                        maximum: None,
                        min_length: None,
                        max_length: None,
                        pattern: None,
                    },
                );
                p
            },
            required: vec![],
        };

        store.save("myapp", &schema).unwrap();

        // Valid enum value
        let result = store.validate("myapp", "log_level", &serde_json::json!("info"));
        assert!(result.is_ok());

        // Invalid enum value
        let result = store.validate("myapp", "log_level", &serde_json::json!("trace"));
        assert!(matches!(result, Err(SchemaError::ValidationFailed(_))));
    }
}
