//! Grant structure and signing

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum GrantError {
    #[error("Grant has expired")]
    Expired,
    #[error("Grant not yet valid")]
    NotYetValid,
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// A signed entitlement grant
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    /// Unique identifier for this grant
    pub id: String,

    /// Feature name this grant authorizes (e.g., "pro.export")
    pub feature: String,

    /// Public key of the identity this grant is for
    pub subject: String,

    /// Public key of the identity that issued this grant
    pub issuer: String,

    /// When this grant becomes valid
    pub valid_from: DateTime<Utc>,

    /// When this grant expires
    pub valid_until: DateTime<Utc>,

    /// Ed25519 signature of the grant fields
    pub signature: String,
}

impl Grant {
    /// Create a new unsigned grant
    pub fn new(
        feature: &str,
        subject: &str,
        issuer: &str,
        valid_from: DateTime<Utc>,
        valid_until: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            feature: feature.to_string(),
            subject: subject.to_string(),
            issuer: issuer.to_string(),
            valid_from,
            valid_until,
            signature: String::new(),
        }
    }

    /// Build the signing payload (deterministic byte sequence)
    ///
    /// Format: id|feature|subject|issuer|validFrom|validUntil
    pub fn signing_payload(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.id,
            self.feature,
            self.subject,
            self.issuer,
            self.valid_from.to_rfc3339(),
            self.valid_until.to_rfc3339()
        )
        .into_bytes()
    }

    /// Check if the grant is currently valid (time-wise)
    pub fn is_valid_now(&self) -> Result<bool, GrantError> {
        let now = Utc::now();

        if now < self.valid_from {
            return Err(GrantError::NotYetValid);
        }

        if now > self.valid_until {
            return Err(GrantError::Expired);
        }

        Ok(true)
    }

    /// Check if the grant is for a specific feature
    pub fn is_for_feature(&self, feature: &str) -> bool {
        self.feature == feature
    }

    /// Check if the grant is for a specific subject
    pub fn is_for_subject(&self, subject: &str) -> bool {
        self.subject == subject
    }

    /// Human-readable status
    pub fn status(&self) -> String {
        let now = Utc::now();

        if now < self.valid_from {
            format!("pending (starts {})", self.valid_from.format("%Y-%m-%d"))
        } else if now > self.valid_until {
            "expired".to_string()
        } else {
            format!("valid until {}", self.valid_until.format("%Y-%m-%d"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_grant_creation() {
        let now = Utc::now();
        let grant = Grant::new(
            "pro.feature",
            "subject_key",
            "issuer_key",
            now,
            now + Duration::days(30),
        );

        assert_eq!(grant.feature, "pro.feature");
        assert!(grant.is_for_feature("pro.feature"));
        assert!(!grant.is_for_feature("other.feature"));
    }

    #[test]
    fn test_signing_payload() {
        let now = Utc::now();
        let grant = Grant::new(
            "pro.feature",
            "subject",
            "issuer",
            now,
            now + Duration::days(30),
        );

        let payload = grant.signing_payload();
        let payload_str = String::from_utf8(payload).unwrap();

        assert!(payload_str.contains(&grant.id));
        assert!(payload_str.contains("pro.feature"));
        assert!(payload_str.contains("|"));
    }

    #[test]
    fn test_validity_check() {
        let now = Utc::now();

        // Valid grant
        let valid_grant = Grant::new(
            "feature",
            "subject",
            "issuer",
            now - Duration::days(1),
            now + Duration::days(30),
        );
        assert!(valid_grant.is_valid_now().unwrap());

        // Expired grant
        let expired_grant = Grant::new(
            "feature",
            "subject",
            "issuer",
            now - Duration::days(60),
            now - Duration::days(30),
        );
        assert!(matches!(
            expired_grant.is_valid_now(),
            Err(GrantError::Expired)
        ));

        // Future grant
        let future_grant = Grant::new(
            "feature",
            "subject",
            "issuer",
            now + Duration::days(30),
            now + Duration::days(60),
        );
        assert!(matches!(
            future_grant.is_valid_now(),
            Err(GrantError::NotYetValid)
        ));
    }
}
