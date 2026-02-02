//! Trusted issuers management

use crate::platform::paths::DataDir;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TrustError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Issuer not trusted: {0}")]
    NotTrusted(String),
    #[error("Issuer already trusted")]
    AlreadyTrusted,
}

/// A trusted issuer entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedIssuer {
    pub public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub added_at: DateTime<Utc>,
}

impl TrustedIssuer {
    /// Create a new trusted issuer
    pub fn new(public_key: &str, alias: Option<&str>) -> Self {
        Self {
            public_key: public_key.to_string(),
            alias: alias.map(|s| s.to_string()),
            added_at: Utc::now(),
        }
    }
}

/// Trusted issuers file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedIssuersFile {
    pub issuers: Vec<TrustedIssuer>,
}

impl Default for TrustedIssuersFile {
    fn default() -> Self {
        Self {
            issuers: Vec::new(),
        }
    }
}

/// Trust store for managing trusted issuers
pub struct TrustStore {
    data_dir: DataDir,
}

impl TrustStore {
    /// Create a new trust store
    pub fn new(data_dir: DataDir) -> Self {
        Self { data_dir }
    }

    /// Load trusted issuers from file
    pub fn load(&self) -> Result<TrustedIssuersFile, TrustError> {
        let path = self.data_dir.trusted_issuers_path();

        if !path.exists() {
            return Ok(TrustedIssuersFile::default());
        }

        let json = fs::read_to_string(&path)?;
        let file: TrustedIssuersFile = serde_json::from_str(&json)?;
        Ok(file)
    }

    /// Save trusted issuers to file
    pub fn save(&self, file: &TrustedIssuersFile) -> Result<(), TrustError> {
        let path = self.data_dir.trusted_issuers_path();
        let json = serde_json::to_string_pretty(file)?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Add a trusted issuer
    pub fn add(&self, issuer: TrustedIssuer) -> Result<(), TrustError> {
        let mut file = self.load()?;

        // Check if already trusted
        if file.issuers.iter().any(|i| i.public_key == issuer.public_key) {
            return Err(TrustError::AlreadyTrusted);
        }

        file.issuers.push(issuer);
        self.save(&file)?;
        Ok(())
    }

    /// Remove a trusted issuer by public key
    pub fn remove(&self, public_key: &str) -> Result<(), TrustError> {
        let mut file = self.load()?;
        let original_len = file.issuers.len();

        file.issuers.retain(|i| i.public_key != public_key);

        if file.issuers.len() == original_len {
            return Err(TrustError::NotTrusted(public_key.to_string()));
        }

        self.save(&file)?;
        Ok(())
    }

    /// Check if an issuer is trusted
    pub fn is_trusted(&self, public_key: &str) -> Result<bool, TrustError> {
        let file = self.load()?;
        Ok(file.issuers.iter().any(|i| i.public_key == public_key))
    }

    /// Get list of all trusted issuers
    pub fn list(&self) -> Result<Vec<TrustedIssuer>, TrustError> {
        let file = self.load()?;
        Ok(file.issuers)
    }

    /// Ensure own identity is trusted (called during init)
    pub fn trust_self(&self, public_key: &str) -> Result<(), TrustError> {
        if !self.is_trusted(public_key)? {
            let issuer = TrustedIssuer::new(public_key, Some("self"));
            self.add(issuer)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, TrustStore) {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();
        (tmp, TrustStore::new(data_dir))
    }

    #[test]
    fn test_add_and_check() {
        let (_tmp, store) = temp_store();

        assert!(!store.is_trusted("key1").unwrap());

        store.add(TrustedIssuer::new("key1", Some("test"))).unwrap();

        assert!(store.is_trusted("key1").unwrap());
        assert!(!store.is_trusted("key2").unwrap());
    }

    #[test]
    fn test_remove() {
        let (_tmp, store) = temp_store();

        store.add(TrustedIssuer::new("key1", None)).unwrap();
        store.add(TrustedIssuer::new("key2", None)).unwrap();

        assert_eq!(store.list().unwrap().len(), 2);

        store.remove("key1").unwrap();

        assert_eq!(store.list().unwrap().len(), 1);
        assert!(!store.is_trusted("key1").unwrap());
        assert!(store.is_trusted("key2").unwrap());
    }

    #[test]
    fn test_duplicate_add_fails() {
        let (_tmp, store) = temp_store();

        store.add(TrustedIssuer::new("key1", None)).unwrap();
        let result = store.add(TrustedIssuer::new("key1", None));

        assert!(matches!(result, Err(TrustError::AlreadyTrusted)));
    }
}
