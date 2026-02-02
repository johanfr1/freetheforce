//! Ed25519 keypair generation and operations

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeypairError {
    #[error("Invalid key length: expected {expected}, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },
    #[error("Invalid base64 encoding")]
    InvalidBase64,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Signature verification failed")]
    VerificationFailed,
}

/// Wrapper around Ed25519 signing key
pub struct Keypair {
    signing_key: SigningKey,
}

impl Keypair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Create a keypair from a 32-byte seed
    pub fn from_seed(seed: &[u8]) -> Result<Self, KeypairError> {
        if seed.len() != 32 {
            return Err(KeypairError::InvalidKeyLength {
                expected: 32,
                got: seed.len(),
            });
        }

        let mut seed_array = [0u8; 32];
        seed_array.copy_from_slice(seed);

        let signing_key = SigningKey::from_bytes(&seed_array);
        Ok(Self { signing_key })
    }

    /// Create a keypair from base64-encoded seed
    pub fn from_base64_seed(encoded: &str) -> Result<Self, KeypairError> {
        let seed = BASE64
            .decode(encoded.trim())
            .map_err(|_| KeypairError::InvalidBase64)?;
        Self::from_seed(&seed)
    }

    /// Get the seed bytes (for storage)
    pub fn seed_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Get the seed as base64 (for storage)
    pub fn seed_base64(&self) -> String {
        BASE64.encode(self.seed_bytes())
    }

    /// Get the public key bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Get the public key as base64
    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.public_key_bytes())
    }

    /// Get the public key with ed25519 prefix (for display)
    pub fn public_key_display(&self) -> String {
        let encoded = self.public_key_base64();
        // Show first 8 and last 4 characters
        if encoded.len() > 12 {
            format!("ed25519:{}...{}", &encoded[..8], &encoded[encoded.len() - 4..])
        } else {
            format!("ed25519:{}", encoded)
        }
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Sign a message and return base64-encoded signature
    pub fn sign_base64(&self, message: &[u8]) -> String {
        let signature = self.sign(message);
        BASE64.encode(signature.to_bytes())
    }

    /// Get the verifying key for signature verification
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }
}

/// Sign a message with a keypair
pub fn sign_message(keypair: &Keypair, message: &[u8]) -> String {
    keypair.sign_base64(message)
}

/// Verify a signature against a public key
pub fn verify_signature(
    public_key_base64: &str,
    message: &[u8],
    signature_base64: &str,
) -> Result<bool, KeypairError> {
    // Decode public key
    let public_key_bytes = BASE64
        .decode(public_key_base64.trim())
        .map_err(|_| KeypairError::InvalidBase64)?;

    if public_key_bytes.len() != 32 {
        return Err(KeypairError::InvalidKeyLength {
            expected: 32,
            got: public_key_bytes.len(),
        });
    }

    let mut pk_array = [0u8; 32];
    pk_array.copy_from_slice(&public_key_bytes);
    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|_| KeypairError::InvalidSignature)?;

    // Decode signature
    let signature_bytes = BASE64
        .decode(signature_base64.trim())
        .map_err(|_| KeypairError::InvalidBase64)?;

    if signature_bytes.len() != 64 {
        return Err(KeypairError::InvalidKeyLength {
            expected: 64,
            got: signature_bytes.len(),
        });
    }

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes);
    let signature = Signature::from_bytes(&sig_array);

    // Verify
    match verifying_key.verify(message, &signature) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let kp = Keypair::generate();
        assert_eq!(kp.seed_bytes().len(), 32);
        assert_eq!(kp.public_key_bytes().len(), 32);
    }

    #[test]
    fn test_keypair_from_seed() {
        let kp1 = Keypair::generate();
        let seed = kp1.seed_bytes();

        let kp2 = Keypair::from_seed(&seed).unwrap();
        assert_eq!(kp1.public_key_base64(), kp2.public_key_base64());
    }

    #[test]
    fn test_sign_and_verify() {
        let kp = Keypair::generate();
        let message = b"Hello, LocalForge!";

        let signature = kp.sign_base64(message);
        let valid = verify_signature(&kp.public_key_base64(), message, &signature).unwrap();

        assert!(valid);
    }

    #[test]
    fn test_invalid_signature() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        let message = b"Hello, LocalForge!";

        let signature = kp1.sign_base64(message);
        // Verify with wrong key
        let valid = verify_signature(&kp2.public_key_base64(), message, &signature).unwrap();

        assert!(!valid);
    }
}
