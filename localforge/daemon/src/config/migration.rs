//! Config migration system
//!
//! Provides declarative migration scripts for schema evolution.
//! Migrations are JSON files stored in ~/.localforge/migrations/

use crate::platform::paths::DataDir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Migration not found: {0}")]
    NotFound(String),
    #[error("Invalid migration: {0}")]
    Invalid(String),
    #[error("Migration failed: {0}")]
    Failed(String),
}

/// Migration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    /// Source schema version
    pub from_version: u32,
    /// Target schema version
    pub to_version: u32,
    /// Description of changes
    #[serde(default)]
    pub description: Option<String>,
    /// Migration operations to apply
    pub operations: Vec<MigrationOp>,
}

/// Migration operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MigrationOp {
    /// Rename a key
    Rename {
        from: String,
        to: String,
    },
    /// Set a default value if key doesn't exist
    SetDefault {
        key: String,
        value: serde_json::Value,
    },
    /// Delete a key
    Delete {
        key: String,
    },
    /// Copy a key's value to another key
    Copy {
        from: String,
        to: String,
    },
}

/// Migration store for managing and running migrations
pub struct MigrationStore {
    data_dir: DataDir,
}

impl MigrationStore {
    pub fn new(data_dir: DataDir) -> Self {
        Self { data_dir }
    }

    /// Get the migrations directory
    fn migrations_dir(&self) -> PathBuf {
        self.data_dir.root().join("migrations")
    }

    /// Ensure migrations directory exists
    pub fn ensure_dir(&self) -> Result<(), MigrationError> {
        let dir = self.migrations_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    /// List available migrations for a namespace
    pub fn list(&self, namespace: &str) -> Result<Vec<Migration>, MigrationError> {
        let dir = self.migrations_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let prefix = format!("{}_v", namespace);
        let mut migrations = Vec::new();

        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with(&prefix) && name.ends_with(".json") {
                    let content = fs::read_to_string(&path)?;
                    let migration: Migration = serde_json::from_str(&content)?;
                    migrations.push(migration);
                }
            }
        }

        // Sort by from_version
        migrations.sort_by_key(|m| m.from_version);

        Ok(migrations)
    }

    /// Find migration path from current version to target
    pub fn find_path(
        &self,
        namespace: &str,
        from: u32,
        to: u32,
    ) -> Result<Vec<Migration>, MigrationError> {
        let all_migrations = self.list(namespace)?;
        let mut path = Vec::new();
        let mut current = from;

        while current < to {
            let next = all_migrations
                .iter()
                .find(|m| m.from_version == current)
                .cloned();

            match next {
                Some(m) => {
                    current = m.to_version;
                    path.push(m);
                }
                None => {
                    return Err(MigrationError::NotFound(format!(
                        "No migration from version {} for namespace '{}'",
                        current, namespace
                    )));
                }
            }
        }

        Ok(path)
    }

    /// Apply a single migration to config values
    pub fn apply_migration(
        &self,
        values: &mut HashMap<String, toml::Value>,
        migration: &Migration,
    ) -> Result<Vec<String>, MigrationError> {
        let mut changes = Vec::new();

        for op in &migration.operations {
            match op {
                MigrationOp::Rename { from, to } => {
                    if let Some(value) = values.remove(from) {
                        values.insert(to.clone(), value);
                        changes.push(format!("Renamed '{}' -> '{}'", from, to));
                    }
                }
                MigrationOp::SetDefault { key, value } => {
                    if !values.contains_key(key) {
                        if let Some(toml_value) = json_to_toml(value.clone()) {
                            values.insert(key.clone(), toml_value);
                            changes.push(format!("Set default '{}'", key));
                        }
                    }
                }
                MigrationOp::Delete { key } => {
                    if values.remove(key).is_some() {
                        changes.push(format!("Deleted '{}'", key));
                    }
                }
                MigrationOp::Copy { from, to } => {
                    if let Some(value) = values.get(from).cloned() {
                        values.insert(to.clone(), value);
                        changes.push(format!("Copied '{}' -> '{}'", from, to));
                    }
                }
            }
        }

        Ok(changes)
    }

    /// Run migrations on a namespace (dry run or real)
    pub fn migrate(
        &self,
        namespace: &str,
        target_version: Option<u32>,
        dry_run: bool,
    ) -> Result<MigrationResult, MigrationError> {
        // Load current config
        let config_path = self.data_dir.project_config_path(namespace);
        if !config_path.exists() {
            return Err(MigrationError::Invalid(format!(
                "No config file for namespace '{}'",
                namespace
            )));
        }

        let content = fs::read_to_string(&config_path)?;
        let mut config: super::store::ConfigFile = toml::from_str(&content)?;
        let current_version = config.schema_version;

        // Determine target version
        let target = target_version.unwrap_or_else(|| {
            self.list(namespace)
                .ok()
                .and_then(|m| m.last().map(|l| l.to_version))
                .unwrap_or(current_version)
        });

        if current_version >= target {
            return Ok(MigrationResult {
                namespace: namespace.to_string(),
                from_version: current_version,
                to_version: current_version,
                changes: Vec::new(),
                applied: false,
            });
        }

        // Find migration path
        let path = self.find_path(namespace, current_version, target)?;

        // Create backup if not dry run
        if !dry_run {
            let backup_path = self
                .data_dir
                .projects_dir()
                .join(format!("{}.v{}.backup.toml", namespace, current_version));
            fs::copy(&config_path, &backup_path)?;
        }

        // Apply migrations
        let mut all_changes = Vec::new();
        for migration in &path {
            let changes = self.apply_migration(&mut config.values, migration)?;
            all_changes.extend(changes);
            config.schema_version = migration.to_version;
        }

        // Write if not dry run
        if !dry_run && !all_changes.is_empty() {
            let new_content = toml::to_string_pretty(&config)?;
            fs::write(&config_path, new_content)?;
        }

        Ok(MigrationResult {
            namespace: namespace.to_string(),
            from_version: current_version,
            to_version: config.schema_version,
            changes: all_changes,
            applied: !dry_run,
        })
    }

    /// Save a migration file
    pub fn save(&self, namespace: &str, migration: &Migration) -> Result<(), MigrationError> {
        self.ensure_dir()?;

        let filename = format!(
            "{}_v{}_to_v{}.json",
            namespace, migration.from_version, migration.to_version
        );
        let path = self.migrations_dir().join(filename);

        let content = serde_json::to_string_pretty(migration)?;
        fs::write(&path, content)?;

        Ok(())
    }
}

/// Result of a migration run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    pub namespace: String,
    pub from_version: u32,
    pub to_version: u32,
    pub changes: Vec<String>,
    pub applied: bool,
}

/// Convert JSON value to TOML value
fn json_to_toml(value: serde_json::Value) -> Option<toml::Value> {
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
            let values: Vec<toml::Value> = arr.into_iter().filter_map(json_to_toml).collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, MigrationStore) {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();
        (tmp, MigrationStore::new(data_dir))
    }

    #[test]
    fn test_rename_operation() {
        let (_tmp, store) = temp_store();

        let mut values: HashMap<String, toml::Value> = HashMap::new();
        values.insert("old_key".to_string(), toml::Value::String("value".into()));

        let migration = Migration {
            from_version: 1,
            to_version: 2,
            description: Some("Rename old_key to new_key".into()),
            operations: vec![MigrationOp::Rename {
                from: "old_key".into(),
                to: "new_key".into(),
            }],
        };

        let changes = store.apply_migration(&mut values, &migration).unwrap();

        assert!(!values.contains_key("old_key"));
        assert_eq!(
            values.get("new_key"),
            Some(&toml::Value::String("value".into()))
        );
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_set_default_operation() {
        let (_tmp, store) = temp_store();

        let mut values: HashMap<String, toml::Value> = HashMap::new();

        let migration = Migration {
            from_version: 1,
            to_version: 2,
            description: None,
            operations: vec![MigrationOp::SetDefault {
                key: "timeout".into(),
                value: serde_json::json!(30),
            }],
        };

        let changes = store.apply_migration(&mut values, &migration).unwrap();

        assert_eq!(values.get("timeout"), Some(&toml::Value::Integer(30)));
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_delete_operation() {
        let (_tmp, store) = temp_store();

        let mut values: HashMap<String, toml::Value> = HashMap::new();
        values.insert("deprecated".to_string(), toml::Value::Boolean(true));

        let migration = Migration {
            from_version: 1,
            to_version: 2,
            description: None,
            operations: vec![MigrationOp::Delete {
                key: "deprecated".into(),
            }],
        };

        let changes = store.apply_migration(&mut values, &migration).unwrap();

        assert!(!values.contains_key("deprecated"));
        assert_eq!(changes.len(), 1);
    }
}
