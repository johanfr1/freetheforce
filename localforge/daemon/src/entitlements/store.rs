//! Grant storage

use super::grant::Grant;
use super::trust::TrustStore;
use super::verify::verify_grant;
use crate::platform::paths::DataDir;
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Grant not found: {0}")]
    NotFound(String),
    #[error("Invalid grant signature")]
    InvalidSignature,
    #[error("Issuer not trusted: {0}")]
    IssuerNotTrusted(String),
    #[error("Verification error: {0}")]
    VerifyError(String),
}

/// Grant store for persisting and querying grants
pub struct GrantStore {
    data_dir: DataDir,
    trust_store: TrustStore,
}

impl GrantStore {
    /// Create a new grant store
    pub fn new(data_dir: DataDir) -> Self {
        let trust_store = TrustStore::new(data_dir.clone());
        Self {
            data_dir,
            trust_store,
        }
    }

    /// Add a grant after verification
    pub fn add(&self, grant: Grant) -> Result<String, StoreError> {
        // Verify issuer is trusted
        let is_trusted = self
            .trust_store
            .is_trusted(&grant.issuer)
            .map_err(|e| StoreError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))?;

        if !is_trusted {
            return Err(StoreError::IssuerNotTrusted(grant.issuer.clone()));
        }

        // Verify signature
        verify_grant(&grant).map_err(|e| StoreError::VerifyError(e.to_string()))?;

        // Save to file
        let path = self.data_dir.grants_dir().join(format!("{}.grant.json", grant.id));
        let json = serde_json::to_string_pretty(&grant)?;
        fs::write(&path, json)?;

        Ok(grant.id)
    }

    /// Remove a grant by ID
    pub fn remove(&self, id: &str) -> Result<(), StoreError> {
        let path = self.data_dir.grants_dir().join(format!("{}.grant.json", id));

        if !path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }

        fs::remove_file(&path)?;
        Ok(())
    }

    /// Get a grant by ID
    pub fn get(&self, id: &str) -> Result<Grant, StoreError> {
        let path = self.data_dir.grants_dir().join(format!("{}.grant.json", id));

        if !path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }

        let json = fs::read_to_string(&path)?;
        let grant: Grant = serde_json::from_str(&json)?;
        Ok(grant)
    }

    /// List all grants
    pub fn list(&self) -> Result<Vec<Grant>, StoreError> {
        let grants_dir = self.data_dir.grants_dir();

        if !grants_dir.exists() {
            return Ok(Vec::new());
        }

        let mut grants = Vec::new();

        for entry in fs::read_dir(&grants_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let json = fs::read_to_string(&path)?;
                if let Ok(grant) = serde_json::from_str::<Grant>(&json) {
                    grants.push(grant);
                }
            }
        }

        Ok(grants)
    }

    /// Check if subject has a valid grant for a feature
    pub fn can(&self, subject: &str, feature: &str) -> Result<(bool, String), StoreError> {
        let grants = self.list()?;

        for grant in grants {
            // Check subject and feature match
            if !grant.is_for_subject(subject) || !grant.is_for_feature(feature) {
                continue;
            }

            // Check time validity
            if grant.is_valid_now().is_err() {
                continue;
            }

            // Verify signature (in case file was tampered)
            if verify_grant(&grant).is_ok() {
                return Ok((
                    true,
                    format!("grant '{}' {}", grant.id, grant.status()),
                ));
            }
        }

        Ok((false, "no valid grant found".to_string()))
    }

    /// Get grants for a specific feature
    pub fn grants_for_feature(&self, feature: &str) -> Result<Vec<Grant>, StoreError> {
        let grants = self.list()?;
        Ok(grants.into_iter().filter(|g| g.is_for_feature(feature)).collect())
    }

    /// Count grants by status
    pub fn count_by_status(&self) -> Result<(usize, usize), StoreError> {
        let grants = self.list()?;
        let mut active = 0;
        let mut expired = 0;

        for grant in grants {
            if grant.is_valid_now().is_ok() {
                active += 1;
            } else {
                expired += 1;
            }
        }

        Ok((active, expired))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;
    use chrono::{Duration, Utc};
    use tempfile::TempDir;

    fn setup() -> (TempDir, GrantStore, Keypair) {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();

        let keypair = Keypair::generate();

        // Trust the keypair
        let trust_store = TrustStore::new(data_dir.clone());
        trust_store.trust_self(&keypair.public_key_base64()).unwrap();

        (tmp, GrantStore::new(data_dir), keypair)
    }

    fn create_signed_grant(keypair: &Keypair, feature: &str) -> Grant {
        let now = Utc::now();
        let public_key = keypair.public_key_base64();

        let mut grant = Grant::new(
            feature,
            &public_key,
            &public_key,
            now - Duration::days(1),
            now + Duration::days(30),
        );

        let payload = grant.signing_payload();
        grant.signature = keypair.sign_base64(&payload);
        grant
    }

    #[test]
    fn test_add_and_list() {
        let (_tmp, store, keypair) = setup();

        let grant = create_signed_grant(&keypair, "pro.feature");
        store.add(grant.clone()).unwrap();

        let grants = store.list().unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].feature, "pro.feature");
    }

    #[test]
    fn test_can() {
        let (_tmp, store, keypair) = setup();

        let grant = create_signed_grant(&keypair, "pro.export");
        store.add(grant).unwrap();

        let public_key = keypair.public_key_base64();
        let (allowed, _reason) = store.can(&public_key, "pro.export").unwrap();
        assert!(allowed);

        let (not_allowed, _) = store.can(&public_key, "other.feature").unwrap();
        assert!(!not_allowed);
    }

    #[test]
    fn test_remove() {
        let (_tmp, store, keypair) = setup();

        let grant = create_signed_grant(&keypair, "pro.feature");
        let id = grant.id.clone();
        store.add(grant).unwrap();

        assert_eq!(store.list().unwrap().len(), 1);

        store.remove(&id).unwrap();

        assert_eq!(store.list().unwrap().len(), 0);
    }
}
