//! forge logs command
//!
//! Note: This reads logs directly from disk rather than through the daemon,
//! since the daemon may not be running when viewing historical logs.

use crate::client::ClientError;
use chrono::Utc;
use directories::BaseDirs;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[derive(Deserialize)]
struct LogEntry {
    #[serde(rename = "ts")]
    timestamp: String,
    #[serde(rename = "lvl")]
    level: String,
    #[serde(rename = "mod")]
    module: String,
    #[serde(rename = "msg")]
    message: String,
}

fn logs_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        BaseDirs::new().map(|b| b.data_local_dir().join("LocalForge").join("logs"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        BaseDirs::new().map(|b| b.home_dir().join(".localforge").join("logs"))
    }
}

pub async fn run(
    lines: usize,
    follow: bool,
    level_filter: Option<&str>,
) -> Result<(), ClientError> {
    let logs_dir = logs_dir().ok_or_else(|| {
        ClientError::Connection("Could not determine logs directory".to_string())
    })?;

    let date = Utc::now().format("%Y-%m-%d").to_string();
    let log_path = logs_dir.join(format!("{}.log", date));

    if !log_path.exists() {
        println!("No logs for today.");
        return Ok(());
    }

    // Read and display last N lines
    let file = File::open(&log_path)?;
    let reader = BufReader::new(file);

    let all_lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
    let start = all_lines.len().saturating_sub(lines);

    for line in &all_lines[start..] {
        if let Some(formatted) = format_log_line(line, level_filter) {
            println!("{}", formatted);
        }
    }

    if follow {
        // Follow mode - watch for new lines
        let mut file = File::open(&log_path)?;
        file.seek(SeekFrom::End(0))?;

        let mut reader = BufReader::new(file);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // No new data, wait a bit
                    thread::sleep(Duration::from_millis(100));
                }
                Ok(_) => {
                    if let Some(formatted) = format_log_line(&line, level_filter) {
                        println!("{}", formatted);
                    }
                }
                Err(_) => break,
            }
        }
    }

    Ok(())
}

fn format_log_line(line: &str, level_filter: Option<&str>) -> Option<String> {
    let entry: LogEntry = serde_json::from_str(line.trim()).ok()?;

    // Apply level filter
    if let Some(filter) = level_filter {
        if !entry.level.eq_ignore_ascii_case(filter) {
            return None;
        }
    }

    // Format: [2024-01-15 10:30:45] INFO  identity: Identity loaded
    let timestamp = entry
        .timestamp
        .get(..19)
        .unwrap_or(&entry.timestamp)
        .replace('T', " ");

    Some(format!(
        "[{}] {:5} {}: {}",
        timestamp, entry.level, entry.module, entry.message
    ))
}
