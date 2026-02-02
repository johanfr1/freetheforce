//! forge config migrate command
//!
//! Runs migrations on config files to upgrade schema versions.

use std::fs;
use std::path::PathBuf;

/// Run migrations on a namespace
pub fn migrate(
    namespace: &str,
    target: Option<u32>,
    dry_run: bool,
    json_output: bool,
) -> Result<(), std::io::Error> {
    // Get data dir
    let home = std::env::var("HOME").unwrap_or_default();
    let data_dir = PathBuf::from(&home).join(".localforge");
    let projects_dir = data_dir.join("config").join("projects");
    let migrations_dir = data_dir.join("migrations");

    // Load config
    let config_path = projects_dir.join(format!("{}.toml", namespace));
    if !config_path.exists() {
        if json_output {
            let output = serde_json::json!({
                "error": format!("No config file for namespace '{}'", namespace)
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            eprintln!("Error: No config file for namespace '{}'", namespace);
        }
        return Ok(());
    }

    // Parse current config
    let config_str = fs::read_to_string(&config_path)?;
    let config: toml::Value = toml::from_str(&config_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let current_version = config
        .get("schema_version")
        .and_then(|v| v.as_integer())
        .unwrap_or(1) as u32;

    // Find migrations
    if !migrations_dir.exists() {
        if json_output {
            let output = serde_json::json!({
                "namespace": namespace,
                "current_version": current_version,
                "message": "No migrations directory"
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("No migrations available for '{}'", namespace);
        }
        return Ok(());
    }

    // List migrations for this namespace
    let prefix = format!("{}_v", namespace);
    let mut migrations: Vec<Migration> = Vec::new();

    for entry in fs::read_dir(&migrations_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(&prefix) && name.ends_with(".json") {
                let content = fs::read_to_string(&path)?;
                if let Ok(mig) = serde_json::from_str::<Migration>(&content) {
                    migrations.push(mig);
                }
            }
        }
    }

    migrations.sort_by_key(|m| m.from_version);

    // Determine target version
    let target_version = target.unwrap_or_else(|| {
        migrations.last().map(|m| m.to_version).unwrap_or(current_version)
    });

    if current_version >= target_version {
        if json_output {
            let output = serde_json::json!({
                "namespace": namespace,
                "current_version": current_version,
                "target_version": target_version,
                "message": "Already at target version"
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("'{}' is already at version {} (target: {})", namespace, current_version, target_version);
        }
        return Ok(());
    }

    // Build migration path
    let mut path = Vec::new();
    let mut current = current_version;
    while current < target_version {
        let next = migrations.iter().find(|m| m.from_version == current);
        match next {
            Some(m) => {
                path.push(m.clone());
                current = m.to_version;
            }
            None => {
                if json_output {
                    let output = serde_json::json!({
                        "error": format!("No migration from version {} to {}", current, target_version)
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    eprintln!("Error: No migration path from v{} to v{}", current, target_version);
                }
                return Ok(());
            }
        }
    }

    // Get mutable values
    let mut values: toml::map::Map<String, toml::Value> = config
        .get("values")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    // Apply migrations
    let mut all_changes = Vec::new();
    let mut final_version = current_version;

    for migration in &path {
        for op in &migration.operations {
            match op {
                MigrationOp::Rename { from, to } => {
                    if let Some(value) = values.remove(from) {
                        values.insert(to.clone(), value);
                        all_changes.push(format!("Renamed '{}' -> '{}'", from, to));
                    }
                }
                MigrationOp::SetDefault { key, value } => {
                    if !values.contains_key(key) {
                        if let Some(toml_value) = json_to_toml(value.clone()) {
                            values.insert(key.clone(), toml_value);
                            all_changes.push(format!("Set default '{}'", key));
                        }
                    }
                }
                MigrationOp::Delete { key } => {
                    if values.remove(key).is_some() {
                        all_changes.push(format!("Deleted '{}'", key));
                    }
                }
                MigrationOp::Copy { from, to } => {
                    if let Some(value) = values.get(from).cloned() {
                        values.insert(to.clone(), value);
                        all_changes.push(format!("Copied '{}' -> '{}'", from, to));
                    }
                }
            }
        }
        final_version = migration.to_version;
    }

    if json_output {
        let output = serde_json::json!({
            "namespace": namespace,
            "from_version": current_version,
            "to_version": final_version,
            "dry_run": dry_run,
            "changes": all_changes
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        if dry_run {
            println!("Dry run: {} v{} → v{}", namespace, current_version, final_version);
        } else {
            println!("Migrating {} v{} → v{}", namespace, current_version, final_version);
        }

        if all_changes.is_empty() {
            println!("  (no changes)");
        } else {
            for change in &all_changes {
                println!("  - {}", change);
            }
        }
    }

    // Apply if not dry run
    if !dry_run && !all_changes.is_empty() {
        // Create backup
        let backup_path = projects_dir.join(format!("{}.v{}.backup.toml", namespace, current_version));
        fs::copy(&config_path, &backup_path)?;
        if !json_output {
            println!("  Backup: {}", backup_path.display());
        }

        // Write new config
        let mut new_config = toml::value::Table::new();
        new_config.insert("schema_version".into(), toml::Value::Integer(final_version as i64));
        new_config.insert("values".into(), toml::Value::Table(values));

        let new_content = toml::to_string_pretty(&new_config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        fs::write(&config_path, new_content)?;

        if !json_output {
            println!("  ✓ Migration complete");
        }
    }

    Ok(())
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Migration {
    from_version: u32,
    to_version: u32,
    #[serde(default)]
    #[allow(dead_code)] // Reserved for CLI display
    description: Option<String>,
    operations: Vec<MigrationOp>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MigrationOp {
    Rename { from: String, to: String },
    SetDefault { key: String, value: serde_json::Value },
    Delete { key: String },
    Copy { from: String, to: String },
}

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
