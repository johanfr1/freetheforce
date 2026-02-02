//! Identity storage and persistence

use super::keypair::Keypair;
use crate::platform::paths::DataDir;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Identity not initialized. Run 'forge init' first.")]
    NotInitialized,
    #[error("Identity already exists")]
    AlreadyExists,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Keypair error: {0}")]
    Keypair(#[from] super::keypair::KeypairError),
}

/// Identity metadata stored in identity.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub public_key: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

impl Identity {
    /// Create a new identity from a keypair
    pub fn new(keypair: &Keypair) -> Self {
        Self {
            public_key: keypair.public_key_base64(),
            created_at: Utc::now(),
            alias: None,
        }
    }

    /// Get the display format for the public key
    pub fn display_key(&self) -> String {
        if self.public_key.len() > 12 {
            format!(
                "ed25519:{}...{}",
                &self.public_key[..8],
                &self.public_key[self.public_key.len() - 4..]
            )
        } else {
            format!("ed25519:{}", self.public_key)
        }
    }
}

/// Storage layer for identity persistence
pub struct IdentityStore {
    data_dir: DataDir,
}

impl IdentityStore {
    /// Create a new identity store
    pub fn new(data_dir: DataDir) -> Self {
        Self { data_dir }
    }

    /// Check if identity exists
    pub fn exists(&self) -> bool {
        self.data_dir.private_key_path().exists() && self.data_dir.identity_json_path().exists()
    }

    /// Initialize a new identity
    pub fn init(&self) -> Result<(Keypair, Identity), StorageError> {
        if self.exists() {
            return Err(StorageError::AlreadyExists);
        }

        // Ensure directories exist
        self.data_dir.ensure_dirs().map_err(|e| {
            StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;

        // Generate keypair
        let keypair = Keypair::generate();
        let identity = Identity::new(&keypair);

        // Save private key (restricted permissions)
        self.save_private_key(&keypair)?;

        // Save identity metadata
        self.save_identity(&identity)?;

        Ok((keypair, identity))
    }

    /// Load existing keypair
    pub fn load_keypair(&self) -> Result<Keypair, StorageError> {
        if !self.exists() {
            return Err(StorageError::NotInitialized);
        }

        let seed_base64 = fs::read_to_string(self.data_dir.private_key_path())?;
        let keypair = Keypair::from_base64_seed(&seed_base64)?;

        Ok(keypair)
    }

    /// Load identity metadata
    pub fn load_identity(&self) -> Result<Identity, StorageError> {
        if !self.exists() {
            return Err(StorageError::NotInitialized);
        }

        let json = fs::read_to_string(self.data_dir.identity_json_path())?;
        let identity: Identity = serde_json::from_str(&json)?;

        Ok(identity)
    }

    /// Set or update alias
    pub fn set_alias(&self, alias: &str) -> Result<Identity, StorageError> {
        let mut identity = self.load_identity()?;
        identity.alias = Some(alias.to_string());
        self.save_identity(&identity)?;
        Ok(identity)
    }

    /// Clear alias
    pub fn clear_alias(&self) -> Result<Identity, StorageError> {
        let mut identity = self.load_identity()?;
        identity.alias = None;
        self.save_identity(&identity)?;
        Ok(identity)
    }

    /// Save private key with restricted permissions
    fn save_private_key(&self, keypair: &Keypair) -> Result<(), StorageError> {
        let path = self.data_dir.private_key_path();
        fs::write(&path, keypair.seed_base64())?;

        // Set file permissions to 0600 (owner read/write only)
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }

    /// Save identity metadata
    fn save_identity(&self, identity: &Identity) -> Result<(), StorageError> {
        let json = serde_json::to_string_pretty(identity)?;
        fs::write(self.data_dir.identity_json_path(), json)?;
        Ok(())
    }

    /// Export identity as a bundle (for backup)
    pub fn export(&self) -> Result<String, StorageError> {
        let keypair = self.load_keypair()?;
        let identity = self.load_identity()?;

        #[derive(Serialize)]
        struct ExportBundle {
            seed: String,
            identity: Identity,
            exported_at: DateTime<Utc>,
        }

        let bundle = ExportBundle {
            seed: keypair.seed_base64(),
            identity,
            exported_at: Utc::now(),
        };

        let json = serde_json::to_string_pretty(&bundle)?;
        Ok(json)
    }

    /// Import identity from a bundle
    pub fn import(&self, bundle_json: &str) -> Result<Identity, StorageError> {
        #[derive(Deserialize)]
        struct ImportBundle {
            seed: String,
            identity: Identity,
        }

        let bundle: ImportBundle = serde_json::from_str(bundle_json)?;
        let keypair = Keypair::from_base64_seed(&bundle.seed)?;

        // Verify the public key matches
        if keypair.public_key_base64() != bundle.identity.public_key {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Public key mismatch in import bundle",
            )));
        }

        // Save
        self.save_private_key(&keypair)?;
        self.save_identity(&bundle.identity)?;

        Ok(bundle.identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_store() -> (TempDir, IdentityStore) {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();
        (tmp, IdentityStore::new(data_dir))
    }

    #[test]
    fn test_init_and_load() {
        let (_tmp, store) = temp_store();

        assert!(!store.exists());

        let (keypair, identity) = store.init().unwrap();
        assert!(store.exists());

        let loaded_keypair = store.load_keypair().unwrap();
        let loaded_identity = store.load_identity().unwrap();

        assert_eq!(keypair.public_key_base64(), loaded_keypair.public_key_base64());
        assert_eq!(identity.public_key, loaded_identity.public_key);
    }

    #[test]
    fn test_alias() {
        let (_tmp, store) = temp_store();
        store.init().unwrap();

        let identity = store.set_alias("my-laptop").unwrap();
        assert_eq!(identity.alias, Some("my-laptop".to_string()));

        let loaded = store.load_identity().unwrap();
        assert_eq!(loaded.alias, Some("my-laptop".to_string()));
    }

    #[test]
    fn test_export_import() {
        let tmp1 = TempDir::new().unwrap();
        let data_dir1 = DataDir::with_path(tmp1.path().to_path_buf());
        data_dir1.ensure_dirs().unwrap();
        let store1 = IdentityStore::new(data_dir1);

        store1.init().unwrap();
        store1.set_alias("original").unwrap();
        let exported = store1.export().unwrap();

        let tmp2 = TempDir::new().unwrap();
        let data_dir2 = DataDir::with_path(tmp2.path().to_path_buf());
        data_dir2.ensure_dirs().unwrap();
        let store2 = IdentityStore::new(data_dir2);

        let imported = store2.import(&exported).unwrap();
        assert_eq!(imported.alias, Some("original".to_string()));
    }
}
