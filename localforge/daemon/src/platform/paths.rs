//! Platform-aware data directory paths
//!
//! Linux: ~/.localforge/
//! Windows: %LOCALAPPDATA%\LocalForge\

use directories::BaseDirs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PathError {
    #[error("Could not determine home directory")]
    NoHomeDir,
    #[error("Could not create directory: {path}")]
    CreateDir { path: PathBuf, source: std::io::Error },
}

/// Data directory abstraction for cross-platform support
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// Create a new DataDir, using platform-appropriate location
    ///
    /// Linux: ~/.localforge/
    /// Windows: %LOCALAPPDATA%\LocalForge\
    pub fn new() -> Result<Self, PathError> {
        let root = Self::default_path()?;
        Ok(Self { root })
    }

    /// Create a DataDir at a custom path (useful for testing)
    pub fn with_path(root: PathBuf) -> Self {
        Self { root }
    }

    /// Get the default data directory path for the current platform
    fn default_path() -> Result<PathBuf, PathError> {
        #[cfg(target_os = "windows")]
        {
            let base = BaseDirs::new().ok_or(PathError::NoHomeDir)?;
            Ok(base.data_local_dir().join("LocalForge"))
        }

        #[cfg(not(target_os = "windows"))]
        {
            let base = BaseDirs::new().ok_or(PathError::NoHomeDir)?;
            Ok(base.home_dir().join(".localforge"))
        }
    }

    /// Ensure all required directories exist
    pub fn ensure_dirs(&self) -> Result<(), PathError> {
        let dirs = [
            self.root.clone(),
            self.identity_dir(),
            self.entitlements_dir(),
            self.grants_dir(),
            self.config_dir(),
            self.projects_dir(),
            self.logs_dir(),
        ];

        for dir in dirs {
            std::fs::create_dir_all(&dir).map_err(|e| PathError::CreateDir {
                path: dir,
                source: e,
            })?;
        }

        Ok(())
    }

    /// Root data directory
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    /// Identity directory: <root>/identity/
    pub fn identity_dir(&self) -> PathBuf {
        self.root.join("identity")
    }

    /// Private key file: <root>/identity/private.key
    pub fn private_key_path(&self) -> PathBuf {
        self.identity_dir().join("private.key")
    }

    /// Identity metadata file: <root>/identity/identity.json
    pub fn identity_json_path(&self) -> PathBuf {
        self.identity_dir().join("identity.json")
    }

    /// Entitlements directory: <root>/entitlements/
    pub fn entitlements_dir(&self) -> PathBuf {
        self.root.join("entitlements")
    }

    /// Trusted issuers file: <root>/entitlements/trusted_issuers.json
    pub fn trusted_issuers_path(&self) -> PathBuf {
        self.entitlements_dir().join("trusted_issuers.json")
    }

    /// Grants directory: <root>/entitlements/grants/
    pub fn grants_dir(&self) -> PathBuf {
        self.entitlements_dir().join("grants")
    }

    /// Config directory: <root>/config/
    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    /// Daemon config file: <root>/config/forge.toml
    pub fn forge_config_path(&self) -> PathBuf {
        self.config_dir().join("forge.toml")
    }

    /// Projects config directory: <root>/config/projects/
    pub fn projects_dir(&self) -> PathBuf {
        self.config_dir().join("projects")
    }

    /// Project config file: <root>/config/projects/<namespace>.toml
    pub fn project_config_path(&self, namespace: &str) -> PathBuf {
        self.projects_dir().join(format!("{}.toml", namespace))
    }

    /// Logs directory: <root>/logs/
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Daily log file: <root>/logs/YYYY-MM-DD.log
    pub fn log_file_path(&self, date: &str) -> PathBuf {
        self.logs_dir().join(format!("{}.log", date))
    }

    /// Socket path (Linux only): <root>/forge.sock
    #[cfg(not(target_os = "windows"))]
    pub fn socket_path(&self) -> PathBuf {
        self.root.join("forge.sock")
    }

    /// Endpoint file (Windows only): <root>/endpoint.json
    #[cfg(target_os = "windows")]
    pub fn endpoint_path(&self) -> PathBuf {
        self.root.join("endpoint.json")
    }

    /// Endpoint path for cross-platform code
    #[cfg(not(target_os = "windows"))]
    pub fn endpoint_path(&self) -> PathBuf {
        self.root.join("endpoint.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_custom_path() {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());

        assert_eq!(data_dir.root(), tmp.path());
        assert_eq!(data_dir.identity_dir(), tmp.path().join("identity"));
    }

    #[test]
    fn test_ensure_dirs() {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());

        data_dir.ensure_dirs().unwrap();

        assert!(data_dir.identity_dir().exists());
        assert!(data_dir.entitlements_dir().exists());
        assert!(data_dir.grants_dir().exists());
        assert!(data_dir.config_dir().exists());
        assert!(data_dir.logs_dir().exists());
    }
}
