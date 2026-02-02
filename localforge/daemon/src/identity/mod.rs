//! Identity module
//!
//! Manages the device's Ed25519 keypair and identity metadata.

mod keypair;
mod storage;

pub use keypair::{sign_message, verify_signature, Keypair, KeypairError};
pub use storage::{Identity, IdentityStore, StorageError};
