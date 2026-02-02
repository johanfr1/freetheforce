//! Grant signature verification

use super::grant::Grant;
use crate::identity::verify_signature;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Signature verification failed")]
    VerificationFailed,
    #[error("Grant expired")]
    Expired,
    #[error("Grant not yet valid")]
    NotYetValid,
    #[error("Issuer not trusted")]
    IssuerNotTrusted,
    #[error("Keypair error: {0}")]
    KeypairError(String),
}

/// Verify a grant's signature
pub fn verify_grant(grant: &Grant) -> Result<bool, VerifyError> {
    // Build the signing payload
    let payload = grant.signing_payload();

    // Verify the signature
    match verify_signature(&grant.issuer, &payload, &grant.signature) {
        Ok(valid) => {
            if valid {
                Ok(true)
            } else {
                Err(VerifyError::VerificationFailed)
            }
        }
        Err(e) => Err(VerifyError::KeypairError(e.to_string())),
    }
}

/// Verify a grant including time validity
/// Reserved for Phase 0.5 SDK: provides combined signature + expiry check
#[allow(dead_code)]
pub fn verify_grant_full(grant: &Grant) -> Result<bool, VerifyError> {
    // Check time validity first
    grant.is_valid_now().map_err(|e| match e {
        super::grant::GrantError::Expired => VerifyError::Expired,
        super::grant::GrantError::NotYetValid => VerifyError::NotYetValid,
        _ => VerifyError::VerificationFailed,
    })?;

    // Then verify signature
    verify_grant(grant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;
    use chrono::{Duration, Utc};

    fn create_signed_grant(keypair: &Keypair) -> Grant {
        let now = Utc::now();
        let public_key = keypair.public_key_base64();

        let mut grant = Grant::new(
            "pro.feature",
            &public_key, // self-grant
            &public_key,
            now - Duration::days(1),
            now + Duration::days(30),
        );

        // Sign the grant
        let payload = grant.signing_payload();
        grant.signature = keypair.sign_base64(&payload);

        grant
    }

    #[test]
    fn test_verify_valid_grant() {
        let keypair = Keypair::generate();
        let grant = create_signed_grant(&keypair);

        let result = verify_grant(&grant);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_tampered_grant() {
        let keypair = Keypair::generate();
        let mut grant = create_signed_grant(&keypair);

        // Tamper with the grant
        grant.feature = "tampered.feature".to_string();

        let result = verify_grant(&grant);
        assert!(matches!(result, Err(VerifyError::VerificationFailed)));
    }

    #[test]
    fn test_verify_wrong_issuer() {
        let keypair1 = Keypair::generate();
        let keypair2 = Keypair::generate();

        let now = Utc::now();
        let mut grant = Grant::new(
            "pro.feature",
            &keypair1.public_key_base64(),
            &keypair2.public_key_base64(), // Different issuer
            now - Duration::days(1),
            now + Duration::days(30),
        );

        // Sign with keypair1 but claim issuer is keypair2
        let payload = grant.signing_payload();
        grant.signature = keypair1.sign_base64(&payload);

        let result = verify_grant(&grant);
        assert!(matches!(result, Err(VerifyError::VerificationFailed)));
    }
}
