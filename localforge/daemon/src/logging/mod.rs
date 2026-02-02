//! Logging module
//!
//! Structured, human-readable logging to daily log files.

mod writer;

pub use writer::{LogEntry, LogLevel, LogWriter, LoggingError};
