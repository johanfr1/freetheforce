//! LocalForge Daemon
//!
//! Local-first infrastructure for identity, trust, config, and logging.

pub mod api;
pub mod config;
pub mod entitlements;
pub mod identity;
pub mod logging;
pub mod platform;
pub mod router;
pub mod server;

pub use platform::paths::DataDir;
