//! Entitlements module
//!
//! Manages signed grants and feature entitlement verification.

mod grant;
mod store;
mod trust;
mod verify;

pub use grant::{Grant, GrantError};
pub use store::{GrantStore, StoreError};
pub use trust::{TrustedIssuer, TrustStore, TrustError};
pub use verify::{verify_grant, VerifyError};
