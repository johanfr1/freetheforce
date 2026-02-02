//! Platform abstraction module
//!
//! Provides OS-aware paths and transport mechanisms.

pub mod paths;
pub mod transport;

pub use paths::DataDir;
pub use transport::Transport;
