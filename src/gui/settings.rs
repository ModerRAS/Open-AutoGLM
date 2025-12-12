//! Settings management for Phone Agent GUI.
//!
//! Provides configuration persistence using JSON files.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application settings that can be saved and loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Model API base URL
    pub base_url: String,
    /// Model API key
    pub api_key: String,
    /// Model name
    pub model_name: String,
    /// ADB device ID (optional)
    pub device_id: String,
    /// Language code ("cn" or "en")
    pub lang: String,
    /// Coordinate system ("relative" or "absolute")
    pub coordinate_system: String,
    /// Coordinate scale X
    pub scale_x: f64,
    /// Coordinate scale Y
    pub scale_y: f64,
    /// Maximum retries for model requests
    pub max_retries: u32,
    /// Retry delay in seconds
    pub retry_delay: u64,
    /// Maximum steps for agent
    pub max_steps: u32,
    /// Enable calibration
    pub enable_calibration: bool,
    /// Calibration mode ("simple" or "complex")
    pub calibration_mode: String,
    /// Complex calibration rounds
    pub calibration_rounds: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8000/v1".to_string(),
            api_key: "EMPTY".to_string(),
            model_name: "autoglm-phone-9b".to_string(),
            device_id: String::new(),
            lang: "cn".to_string(),
            coordinate_system: "absolute".to_string(),
            scale_x: 1.61,
            scale_y: 1.61,
            max_retries: 3,
            retry_delay: 2,
            max_steps: 100,
            enable_calibration: false,
            calibration_mode: "simple".to_string(),
            calibration_rounds: 5,
        }
    }
}

impl AppSettings {
    /// Get the config directory path.
    fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "moderras", "phone-agent")
            .map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Get the settings file path.
    fn settings_path() -> Option<PathBuf> {
        Self::config_dir().map(|dir| dir.join("settings.json"))
    }

    /// Load settings from the config file.
    pub fn load() -> Self {
        Self::settings_path()
            .and_then(|path| fs::read_to_string(&path).ok())
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save settings to the config file.
    pub fn save(&self) -> Result<(), String> {
        let dir = Self::config_dir().ok_or("Cannot determine config directory")?;
        
        // Create config directory if it doesn't exist
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
        
        let path = dir.join("settings.json");
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        
        fs::write(&path, content)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;
        
        Ok(())
    }

    /// Get logs directory path.
    pub fn logs_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "moderras", "phone-agent")
            .map(|dirs| dirs.data_dir().join("logs"))
    }
}
