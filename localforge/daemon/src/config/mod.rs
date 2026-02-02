//! Config store module
//!
//! Provides namespaced configuration storage using TOML.

mod store;

pub use store::{ConfigStore, ConfigError};
