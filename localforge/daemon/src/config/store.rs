//! TOML config store

use crate::platform::paths::DataDir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Namespace not found: {0}")]
    NamespaceNotFound(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Invalid value type")]
    InvalidValueType,
}

/// Config file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub schema_version: u32,
    #[serde(default)]
    pub values: HashMap<String, toml::Value>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            values: HashMap::new(),
        }
    }
}

/// Config store for namespaced configuration
pub struct ConfigStore {
    data_dir: DataDir,
}

impl ConfigStore {
    /// Create a new config store
    pub fn new(data_dir: DataDir) -> Self {
        Self { data_dir }
    }

    /// Load config for a namespace
    pub fn load(&self, namespace: &str) -> Result<ConfigFile, ConfigError> {
        let path = self.data_dir.project_config_path(namespace);

        if !path.exists() {
            return Ok(ConfigFile::default());
        }

        let content = fs::read_to_string(&path)?;
        let config: ConfigFile = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config for a namespace
    pub fn save(&self, namespace: &str, config: &ConfigFile) -> Result<(), ConfigError> {
        let path = self.data_dir.project_config_path(namespace);

        // Ensure projects directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(config)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Get a value from a namespace
    pub fn get(&self, namespace: &str, key: &str) -> Result<Option<toml::Value>, ConfigError> {
        let config = self.load(namespace)?;
        Ok(config.values.get(key).cloned())
    }

    /// Set a value in a namespace
    pub fn set(
        &self,
        namespace: &str,
        key: &str,
        value: toml::Value,
    ) -> Result<(), ConfigError> {
        let mut config = self.load(namespace)?;
        config.values.insert(key.to_string(), value);
        self.save(namespace, &config)?;
        Ok(())
    }

    /// Remove a value from a namespace
    pub fn remove(&self, namespace: &str, key: &str) -> Result<(), ConfigError> {
        let mut config = self.load(namespace)?;

        if config.values.remove(key).is_none() {
            return Err(ConfigError::KeyNotFound(key.to_string()));
        }

        self.save(namespace, &config)?;
        Ok(())
    }

    /// List all values in a namespace
    pub fn list(&self, namespace: &str) -> Result<HashMap<String, toml::Value>, ConfigError> {
        let config = self.load(namespace)?;
        Ok(config.values)
    }

    /// Reset a namespace to defaults (empty)
    pub fn reset(&self, namespace: &str) -> Result<(), ConfigError> {
        let config = ConfigFile::default();
        self.save(namespace, &config)?;
        Ok(())
    }

    /// Check if a namespace exists
    pub fn exists(&self, namespace: &str) -> bool {
        self.data_dir.project_config_path(namespace).exists()
    }

    /// List all namespaces
    pub fn list_namespaces(&self) -> Result<Vec<String>, ConfigError> {
        let projects_dir = self.data_dir.projects_dir();

        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut namespaces = Vec::new();

        for entry in fs::read_dir(&projects_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    namespaces.push(stem.to_string_lossy().to_string());
                }
            }
        }

        Ok(namespaces)
    }

    /// Get schema version for a namespace
    pub fn schema_version(&self, namespace: &str) -> Result<u32, ConfigError> {
        let config = self.load(namespace)?;
        Ok(config.schema_version)
    }
}

/// Helper to convert JSON value to TOML value
pub fn json_to_toml(value: serde_json::Value) -> Option<toml::Value> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(b) => Some(toml::Value::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Some(toml::Value::Float(f))
            } else {
                None
            }
        }
        serde_json::Value::String(s) => Some(toml::Value::String(s)),
        serde_json::Value::Array(arr) => {
            let values: Vec<toml::Value> = arr
                .into_iter()
                .filter_map(json_to_toml)
                .collect();
            Some(toml::Value::Array(values))
        }
        serde_json::Value::Object(obj) => {
            let mut table = toml::value::Table::new();
            for (k, v) in obj {
                if let Some(tv) = json_to_toml(v) {
                    table.insert(k, tv);
                }
            }
            Some(toml::Value::Table(table))
        }
    }
}

/// Helper to convert TOML value to JSON value
pub fn toml_to_json(value: toml::Value) -> serde_json::Value {
    match value {
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Integer(i) => serde_json::json!(i),
        toml::Value::Float(f) => serde_json::json!(f),
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => {
            let obj: serde_json::Map<String, serde_json::Value> = table
                .into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, ConfigStore) {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();
        (tmp, ConfigStore::new(data_dir))
    }

    #[test]
    fn test_get_set() {
        let (_tmp, store) = temp_store();

        store
            .set("myproject", "api_endpoint", toml::Value::String("http://localhost:8080".into()))
            .unwrap();

        let value = store.get("myproject", "api_endpoint").unwrap();
        assert_eq!(
            value,
            Some(toml::Value::String("http://localhost:8080".into()))
        );
    }

    #[test]
    fn test_list() {
        let (_tmp, store) = temp_store();

        store.set("myproject", "key1", toml::Value::String("value1".into())).unwrap();
        store.set("myproject", "key2", toml::Value::Integer(42)).unwrap();

        let values = store.list("myproject").unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values.get("key1"), Some(&toml::Value::String("value1".into())));
    }

    #[test]
    fn test_reset() {
        let (_tmp, store) = temp_store();

        store.set("myproject", "key1", toml::Value::String("value1".into())).unwrap();
        assert!(!store.list("myproject").unwrap().is_empty());

        store.reset("myproject").unwrap();
        assert!(store.list("myproject").unwrap().is_empty());
    }

    #[test]
    fn test_list_namespaces() {
        let (_tmp, store) = temp_store();

        store.set("project1", "key", toml::Value::Boolean(true)).unwrap();
        store.set("project2", "key", toml::Value::Boolean(false)).unwrap();

        let namespaces = store.list_namespaces().unwrap();
        assert_eq!(namespaces.len(), 2);
        assert!(namespaces.contains(&"project1".to_string()));
        assert!(namespaces.contains(&"project2".to_string()));
    }
}
