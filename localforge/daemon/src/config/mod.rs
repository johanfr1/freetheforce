//! Config store module
//!
//! Provides namespaced configuration storage using TOML.

mod migration;
mod schema;
mod store;

pub use migration::{Migration, MigrationError, MigrationOp, MigrationResult, MigrationStore};
pub use schema::{Schema, SchemaError, SchemaStore, PropertySchema};
pub use store::{ConfigFile, ConfigStore, ConfigError, json_to_toml, toml_to_json};


