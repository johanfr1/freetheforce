//! Log file writer

use crate::platform::paths::DataDir;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LoggingError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Log level for entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DEBUG" => Ok(LogLevel::Debug),
            "INFO" => Ok(LogLevel::Info),
            "WARN" => Ok(LogLevel::Warn),
            "ERROR" => Ok(LogLevel::Error),
            _ => Err(format!("Unknown log level: {}", s)),
        }
    }
}

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    #[serde(rename = "ts")]
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "lvl")]
    pub level: LogLevel,
    #[serde(rename = "mod")]
    pub module: String,
    #[serde(rename = "msg")]
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(level: LogLevel, module: &str, message: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            module: module.to_string(),
            message: message.to_string(),
            data: None,
        }
    }

    /// Add structured data to the entry
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Format for human-readable display
    pub fn display(&self) -> String {
        let data_str = self
            .data
            .as_ref()
            .map(|d| format!(" {}", d))
            .unwrap_or_default();

        format!(
            "[{}] {:5} {}: {}{}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.level,
            self.module,
            self.message,
            data_str
        )
    }
}

/// Log writer that writes to daily log files
pub struct LogWriter {
    data_dir: DataDir,
    current_file: Mutex<Option<(String, File)>>,
}

impl LogWriter {
    /// Create a new log writer
    pub fn new(data_dir: DataDir) -> Self {
        Self {
            data_dir,
            current_file: Mutex::new(None),
        }
    }

    /// Write a log entry
    pub fn write(&self, entry: &LogEntry) -> Result<(), LoggingError> {
        let date = entry.timestamp.format("%Y-%m-%d").to_string();
        let mut guard = self.current_file.lock().unwrap();

        // Check if we need to open a new file
        let file = if let Some((current_date, ref mut file)) = *guard {
            if current_date == date {
                file
            } else {
                // Date changed, open new file
                drop(guard);
                *self.current_file.lock().unwrap() = None;
                guard = self.current_file.lock().unwrap();
                self.open_file(&date, &mut guard)?
            }
        } else {
            self.open_file(&date, &mut guard)?
        };

        // Write JSON line
        let json = serde_json::to_string(entry)?;
        writeln!(file, "{}", json)?;
        file.flush()?;

        Ok(())
    }

    /// Open or get the log file for a date
    fn open_file<'a>(
        &self,
        date: &str,
        guard: &'a mut Option<(String, File)>,
    ) -> Result<&'a mut File, LoggingError> {
        let path = self.data_dir.log_file_path(date);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        *guard = Some((date.to_string(), file));
        Ok(&mut guard.as_mut().unwrap().1)
    }

    /// Log a debug message
    pub fn debug(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.write(&LogEntry::new(LogLevel::Debug, module, message))
    }

    /// Log an info message
    pub fn info(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.write(&LogEntry::new(LogLevel::Info, module, message))
    }

    /// Log a warning message
    pub fn warn(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.write(&LogEntry::new(LogLevel::Warn, module, message))
    }

    /// Log an error message
    pub fn error(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.write(&LogEntry::new(LogLevel::Error, module, message))
    }

    /// Read the last n lines from today's log
    pub fn tail(&self, n: usize) -> Result<Vec<LogEntry>, LoggingError> {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        self.tail_date(&date, n)
    }

    /// Read the last n lines from a specific date's log
    pub fn tail_date(&self, date: &str, n: usize) -> Result<Vec<LogEntry>, LoggingError> {
        let path = self.data_dir.log_file_path(date);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;
        let start = lines.len().saturating_sub(n);

        let entries: Vec<LogEntry> = lines[start..]
            .iter()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        Ok(entries)
    }

    /// Query logs with a filter
    pub fn query(
        &self,
        date: &str,
        level: Option<LogLevel>,
        module: Option<&str>,
    ) -> Result<Vec<LogEntry>, LoggingError> {
        let path = self.data_dir.log_file_path(date);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let entries: Vec<LogEntry> = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str::<LogEntry>(&line).ok())
            .filter(|entry| {
                let level_match = level.map_or(true, |l| entry.level == l);
                let module_match = module.map_or(true, |m| entry.module == m);
                level_match && module_match
            })
            .collect();

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_entry() {
        let entry = LogEntry::new(LogLevel::Info, "identity", "Identity loaded");
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("INFO"));
        assert!(json.contains("identity"));
    }

    #[test]
    fn test_log_writer() {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();

        let writer = LogWriter::new(data_dir);
        writer.info("test", "Hello, logs!").unwrap();
        writer.debug("test", "Debug message").unwrap();

        let entries = writer.tail(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "Hello, logs!");
    }

    #[test]
    fn test_query_by_level() {
        let tmp = TempDir::new().unwrap();
        let data_dir = DataDir::with_path(tmp.path().to_path_buf());
        data_dir.ensure_dirs().unwrap();

        let writer = LogWriter::new(data_dir);
        writer.info("mod1", "Info message").unwrap();
        writer.error("mod2", "Error message").unwrap();
        writer.info("mod1", "Another info").unwrap();

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let errors = writer.query(&date, Some(LogLevel::Error), None).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "Error message");
    }
}
