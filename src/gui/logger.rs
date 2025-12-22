//! Logger module for Phone Agent GUI.
//!
//! Provides log storage and retrieval functionality.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use super::settings::AppSettings;

/// Log level enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
    Debug,
    Action,
    Thinking,
}

impl LogLevel {
    /// Get display string for the log level.
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Success => "SUCCESS",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
            LogLevel::Action => "ACTION",
            LogLevel::Thinking => "THINK",
        }
    }

    /// Get emoji for the log level.
    pub fn emoji(&self) -> &'static str {
        match self {
            LogLevel::Info => "ℹ️",
            LogLevel::Success => "✅",
            LogLevel::Warning => "⚠️",
            LogLevel::Error => "❌",
            LogLevel::Debug => "🔍",
            LogLevel::Action => "🎯",
            LogLevel::Thinking => "💭",
        }
    }
}

/// A single log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now(),
            level,
            message: message.into(),
        }
    }

    /// Format the log entry for display.
    pub fn format_display(&self) -> String {
        format!(
            "[{}] {} {}",
            self.timestamp.format("%H:%M:%S"),
            self.level.emoji(),
            self.message
        )
    }

    /// Format the log entry for file storage.
    pub fn format_file(&self) -> String {
        format!(
            "[{}] [{}] {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.level.as_str(),
            self.message
        )
    }
}

/// Logger that manages log entries in memory and on disk.
#[derive(Debug, Clone)]
pub struct Logger {
    /// In-memory log entries for display.
    entries: Vec<LogEntry>,
    /// Maximum entries to keep in memory.
    max_entries: usize,
    /// Current session log file path.
    log_file: Option<PathBuf>,
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

impl Logger {
    /// Create a new logger.
    pub fn new() -> Self {
        let log_file = Self::create_log_file();
        Self {
            entries: Vec::new(),
            max_entries: 1000,
            log_file,
        }
    }

    /// Create a new log file for this session.
    fn create_log_file() -> Option<PathBuf> {
        let logs_dir = AppSettings::logs_dir()?;
        fs::create_dir_all(&logs_dir).ok()?;

        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let path = logs_dir.join(format!("session_{}.log", timestamp));

        // Create the file
        File::create(&path).ok()?;

        Some(path)
    }

    /// Add a log entry.
    pub fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        let entry = LogEntry::new(level, message);

        // Write to file
        if let Some(ref path) = self.log_file {
            if let Ok(mut file) = OpenOptions::new().append(true).open(path) {
                let _ = writeln!(file, "{}", entry.format_file());
            }
        }

        // Add to memory
        self.entries.push(entry);

        // Trim if too many entries
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Convenience methods for different log levels.
    pub fn info(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Info, message);
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Success, message);
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Warning, message);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Error, message);
    }

    pub fn debug(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message);
    }

    pub fn action(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Action, message);
    }

    pub fn thinking(&mut self, message: impl Into<String>) {
        self.log(LogLevel::Thinking, message);
    }

    /// Get all log entries.
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Clear all log entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get the current log file path.
    pub fn log_file_path(&self) -> Option<&PathBuf> {
        self.log_file.as_ref()
    }

    /// Get formatted log text for display.
    pub fn format_all(&self) -> String {
        self.entries
            .iter()
            .map(|e| e.format_display())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// List all log files in the logs directory.
    pub fn list_log_files() -> Vec<PathBuf> {
        AppSettings::logs_dir()
            .and_then(|dir| fs::read_dir(dir).ok())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|ext| ext == "log").unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Read a log file's contents.
    pub fn read_log_file(path: &PathBuf) -> Result<Vec<String>, String> {
        let file = File::open(path).map_err(|e| format!("Failed to open log file: {}", e))?;
        let reader = BufReader::new(file);
        Ok(reader.lines().map_while(Result::ok).collect())
    }
}
