//! Shared settings for Phone Agent CLI and GUI.
//! Persisted in the platform-specific config directory via `directories::ProjectDirs`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application settings that can be saved and loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
    /// Planner model API base URL
    pub planner_base_url: String,
    /// Planner model API key
    pub planner_api_key: String,
    /// Planner model name
    pub planner_model_name: String,
    /// Max executor feedback history for planner
    pub max_executor_feedback_history: usize,
    /// Stuck threshold for planner (consecutive unchanged screens)
    pub stuck_threshold: u32,
    /// Prompt memory file path
    pub prompt_memory_path: String,
    /// Planner loop interval in milliseconds
    pub planner_interval_ms: u64,
    /// Executor loop interval in milliseconds
    pub executor_interval_ms: u64,
    /// Enable dual-loop mode (planner + executor)
    pub dual_loop_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8000/v1".to_string(),
            api_key: "EMPTY".to_string(),
            model_name: "autoglm-phone-9b".to_string(),
            device_id: String::new(),
            lang: "cn".to_string(),
            coordinate_system: "relative".to_string(),
            scale_x: 1.0,
            scale_y: 1.0,
            max_retries: 3,
            retry_delay: 2,
            max_steps: 100,
            enable_calibration: false,
            calibration_mode: "simple".to_string(),
            calibration_rounds: 5,
            planner_base_url: "https://api.deepseek.com/v1".to_string(),
            planner_api_key: "EMPTY".to_string(),
            planner_model_name: "deepseek-chat".to_string(),
            max_executor_feedback_history: 2,
            stuck_threshold: 3,
            prompt_memory_path: "prompt_memory.json".to_string(),
            planner_interval_ms: 2000,
            executor_interval_ms: 500,
            dual_loop_mode: false,
        }
    }
}

impl AppSettings {
    /// Get the config directory path.
    pub fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "moderras", "phone-agent")
            .map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Get the settings file path.
    pub fn settings_path() -> Option<PathBuf> {
        Self::config_dir().map(|dir| dir.join("settings.json"))
    }

    /// Load settings from the config file.
    pub fn load() -> Self {
        let defaults = Self::default();

        let mut loaded = Self::settings_path()
            .and_then(|path| fs::read_to_string(&path).ok())
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default();

        // Backfill new fields when loading older config files
        if loaded.planner_base_url.is_empty() {
            loaded.planner_base_url = defaults.planner_base_url;
        }
        if loaded.planner_api_key.is_empty() {
            loaded.planner_api_key = defaults.planner_api_key;
        }
        if loaded.planner_model_name.is_empty() {
            loaded.planner_model_name = defaults.planner_model_name;
        }
        if loaded.prompt_memory_path.is_empty() {
            loaded.prompt_memory_path = defaults.prompt_memory_path;
        }
        if loaded.planner_interval_ms == 0 {
            loaded.planner_interval_ms = defaults.planner_interval_ms;
        }
        if loaded.executor_interval_ms == 0 {
            loaded.executor_interval_ms = defaults.executor_interval_ms;
        }

        loaded
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
