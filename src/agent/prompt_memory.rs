//! Prompt memory system for storing optimized prompts by task type.
//!
//! This module provides a simple key-value store for system prompts,
//! organized by task type. No vector database or semantic search is used.
//!
//! Enhanced with user correction learning: when users manually correct
//! the executor's behavior, the corrections are accumulated and can be
//! consolidated into optimized prompts.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// A user correction record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionRecord {
    /// The correction content (what user told the executor to do).
    pub content: String,
    /// Context: what was happening when user corrected.
    pub context: Option<String>,
    /// Timestamp of correction.
    pub timestamp: String,
}

impl CorrectionRecord {
    pub fn new(content: impl Into<String>, context: Option<String>) -> Self {
        Self {
            content: content.into(),
            context,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// A single prompt entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEntry {
    /// The optimized system prompt.
    pub system_prompt: String,
    /// Last update timestamp (ISO 8601 format).
    pub last_updated: String,
    /// Success rate (0.0 to 1.0) - optional metric.
    #[serde(default)]
    pub success_rate: Option<f32>,
    /// Number of times this prompt was used.
    #[serde(default)]
    pub usage_count: u32,
    /// Notes about the prompt.
    #[serde(default)]
    pub notes: Option<String>,
    /// User corrections accumulated (not yet consolidated).
    #[serde(default)]
    pub corrections: Vec<CorrectionRecord>,
}

impl PromptEntry {
    /// Create a new prompt entry.
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            last_updated: Utc::now().to_rfc3339(),
            success_rate: None,
            usage_count: 0,
            notes: None,
            corrections: Vec::new(),
        }
    }

    /// Update the prompt content.
    pub fn update(&mut self, new_prompt: impl Into<String>) {
        self.system_prompt = new_prompt.into();
        self.last_updated = Utc::now().to_rfc3339();
    }

    /// Record a usage and update success rate.
    pub fn record_usage(&mut self, success: bool) {
        let current_successes = self.success_rate.unwrap_or(0.0) * self.usage_count as f32;
        self.usage_count += 1;
        let new_successes = if success {
            current_successes + 1.0
        } else {
            current_successes
        };
        self.success_rate = Some(new_successes / self.usage_count as f32);
    }

    /// Add a user correction.
    pub fn add_correction(&mut self, content: impl Into<String>, context: Option<String>) {
        self.corrections
            .push(CorrectionRecord::new(content, context));
        self.last_updated = Utc::now().to_rfc3339();
    }

    /// Get pending corrections count.
    pub fn pending_corrections_count(&self) -> usize {
        self.corrections.len()
    }

    /// Clear corrections (after consolidation).
    pub fn clear_corrections(&mut self) {
        self.corrections.clear();
    }

    /// Get all corrections as a summary string.
    pub fn corrections_summary(&self) -> String {
        if self.corrections.is_empty() {
            return String::new();
        }
        self.corrections
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c.content))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Prompt memory storage.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptMemory {
    /// Prompts indexed by task type.
    pub prompts: HashMap<String, PromptEntry>,
    /// Version for future compatibility.
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl PromptMemory {
    /// Create a new empty prompt memory.
    pub fn new() -> Self {
        Self {
            prompts: HashMap::new(),
            version: default_version(),
        }
    }

    /// Load prompt memory from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PromptMemoryError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new());
        }

        let content =
            fs::read_to_string(path).map_err(|e| PromptMemoryError::IoError(e.to_string()))?;

        serde_json::from_str(&content).map_err(|e| PromptMemoryError::ParseError(e.to_string()))
    }

    /// Save prompt memory to a JSON file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), PromptMemoryError> {
        let path = path.as_ref();

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| PromptMemoryError::IoError(e.to_string()))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| PromptMemoryError::SerializeError(e.to_string()))?;

        fs::write(path, content).map_err(|e| PromptMemoryError::IoError(e.to_string()))
    }

    /// Get a prompt by task type.
    pub fn get(&self, task_type: &str) -> Option<&PromptEntry> {
        self.prompts.get(task_type)
    }

    /// Get a mutable prompt by task type.
    pub fn get_mut(&mut self, task_type: &str) -> Option<&mut PromptEntry> {
        self.prompts.get_mut(task_type)
    }

    /// Get the system prompt string for a task type.
    pub fn get_prompt(&self, task_type: &str) -> Option<&str> {
        self.prompts
            .get(task_type)
            .map(|e| e.system_prompt.as_str())
    }

    /// Update or create a prompt for a task type.
    pub fn update(&mut self, task_type: impl Into<String>, prompt: impl Into<String>) {
        let task_type = task_type.into();
        let prompt = prompt.into();

        if let Some(entry) = self.prompts.get_mut(&task_type) {
            entry.update(prompt);
        } else {
            self.prompts.insert(task_type, PromptEntry::new(prompt));
        }
    }

    /// Record usage of a prompt.
    pub fn record_usage(&mut self, task_type: &str, success: bool) {
        if let Some(entry) = self.prompts.get_mut(task_type) {
            entry.record_usage(success);
        }
    }

    /// Remove a prompt.
    pub fn remove(&mut self, task_type: &str) -> Option<PromptEntry> {
        self.prompts.remove(task_type)
    }

    /// List all task types.
    pub fn task_types(&self) -> Vec<&str> {
        self.prompts.keys().map(|s| s.as_str()).collect()
    }

    /// Get all entries.
    pub fn entries(&self) -> &HashMap<String, PromptEntry> {
        &self.prompts
    }

    /// Check if a task type exists.
    pub fn contains(&self, task_type: &str) -> bool {
        self.prompts.contains_key(task_type)
    }

    /// Add a correction for a task type.
    /// If the task type doesn't exist, creates a new entry with default prompt.
    pub fn add_correction(
        &mut self,
        task_type: impl Into<String>,
        content: impl Into<String>,
        context: Option<String>,
    ) {
        let task_type = task_type.into();
        let content = content.into();

        if let Some(entry) = self.prompts.get_mut(&task_type) {
            entry.add_correction(content, context);
        } else {
            // Create new entry with empty prompt and add correction
            let mut entry = PromptEntry::new("");
            entry.add_correction(content, context);
            self.prompts.insert(task_type, entry);
        }
    }

    /// Get pending corrections count for a task type.
    pub fn pending_corrections(&self, task_type: &str) -> usize {
        self.prompts
            .get(task_type)
            .map(|e| e.pending_corrections_count())
            .unwrap_or(0)
    }

    /// Get all task types with pending corrections.
    pub fn task_types_with_corrections(&self) -> Vec<&str> {
        self.prompts
            .iter()
            .filter(|(_, e)| e.pending_corrections_count() > 0)
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Get corrections summary for a task type.
    pub fn get_corrections_summary(&self, task_type: &str) -> Option<String> {
        self.prompts.get(task_type).map(|e| e.corrections_summary())
    }

    /// Get the number of stored prompts.
    pub fn len(&self) -> usize {
        self.prompts.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }

    /// Clear all prompts.
    pub fn clear(&mut self) {
        self.prompts.clear();
    }

    /// Merge another prompt memory into this one.
    /// Existing entries are updated only if the incoming entry is newer.
    pub fn merge(&mut self, other: &PromptMemory) {
        for (task_type, entry) in &other.prompts {
            if let Some(existing) = self.prompts.get_mut(task_type) {
                // Compare timestamps and keep newer
                if entry.last_updated > existing.last_updated {
                    *existing = entry.clone();
                }
            } else {
                self.prompts.insert(task_type.clone(), entry.clone());
            }
        }
    }

    /// Get prompts with success rate above threshold.
    pub fn get_successful_prompts(&self, min_rate: f32) -> Vec<(&str, &PromptEntry)> {
        self.prompts
            .iter()
            .filter(|(_, entry)| entry.success_rate.map(|r| r >= min_rate).unwrap_or(false))
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }

    /// Get a summary of all task types for Planner to use.
    /// Returns a formatted string listing all available task types with their prompts.
    pub fn get_task_types_summary(&self) -> String {
        if self.prompts.is_empty() {
            return "（暂无已保存的任务类型记忆）".to_string();
        }

        let mut summaries: Vec<String> = self
            .prompts
            .iter()
            .map(|(task_type, entry)| {
                let prompt_preview = if entry.system_prompt.is_empty() {
                    "(无提示词，仅有纠偏记录)".to_string()
                } else if entry.system_prompt.chars().count() > 50 {
                    format!(
                        "{}...",
                        entry.system_prompt.chars().take(50).collect::<String>()
                    )
                } else {
                    entry.system_prompt.clone()
                };

                let stats = format!(
                    "使用{}次{}",
                    entry.usage_count,
                    entry
                        .success_rate
                        .map(|r| format!(", 成功率{:.0}%", r * 100.0))
                        .unwrap_or_default()
                );

                format!("- **{}**: {} [{}]", task_type, prompt_preview, stats)
            })
            .collect();

        summaries.sort(); // Alphabetical order
        summaries.join("\n")
    }

    /// Get task types as a simple list (for matching).
    pub fn get_task_types_list(&self) -> Vec<String> {
        self.prompts.keys().cloned().collect()
    }

    /// Find the best matching task type for a given description.
    /// Returns None if no good match is found (Planner should create a new type).
    /// This is a simple keyword-based matching; Planner can do better semantic matching.
    pub fn find_matching_task_type(&self, description: &str) -> Option<String> {
        let desc_lower = description.to_lowercase();

        // Simple keyword matching - find task type whose name appears in description
        for task_type in self.prompts.keys() {
            let type_lower = task_type.to_lowercase();
            // Check if task type name (or parts of it) appear in description
            if desc_lower.contains(&type_lower) || type_lower.contains(&desc_lower) {
                return Some(task_type.clone());
            }
            // Check individual words
            for word in type_lower.split(|c: char| !c.is_alphanumeric()) {
                if word.len() > 2 && desc_lower.contains(word) {
                    return Some(task_type.clone());
                }
            }
        }

        None
    }

    /// Create or get a task type entry.
    /// If the task type exists, returns it; otherwise creates a new empty entry.
    pub fn ensure_task_type(&mut self, task_type: impl Into<String>) -> &mut PromptEntry {
        let task_type = task_type.into();
        self.prompts
            .entry(task_type)
            .or_insert_with(|| PromptEntry::new(""))
    }
}

/// Prompt memory errors.
#[derive(Debug, Clone)]
pub enum PromptMemoryError {
    IoError(String),
    ParseError(String),
    SerializeError(String),
}

impl std::fmt::Display for PromptMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::ParseError(e) => write!(f, "Parse error: {}", e),
            Self::SerializeError(e) => write!(f, "Serialize error: {}", e),
        }
    }
}

impl std::error::Error for PromptMemoryError {}

/// Default task types with suggested prompts.
pub fn default_task_types() -> Vec<(&'static str, &'static str)> {
    vec![
        ("微信操作", "你是一个专门处理微信相关任务的手机操作助手。微信的常见操作包括：发送消息、查看朋友圈、添加好友等。请注意微信的界面层级结构。"),
        ("设置调整", "你是一个专门处理系统设置的手机操作助手。请熟悉 Android 设置界面的层级结构，包括：无线网络、显示、声音、应用等设置项。"),
        ("应用安装", "你是一个专门处理应用安装任务的手机操作助手。请熟悉应用商店的操作流程，包括搜索、安装、更新等。"),
        ("文件管理", "你是一个专门处理文件管理任务的手机操作助手。请熟悉文件管理器的操作，包括浏览、复制、移动、删除等。"),
        ("通用任务", "你是一个通用的手机操作助手，可以处理各种手机操作任务。请根据当前界面状态选择合适的操作。"),
    ]
}

/// Initialize prompt memory with default task types.
pub fn create_default_prompt_memory() -> PromptMemory {
    let mut memory = PromptMemory::new();
    for (task_type, prompt) in default_task_types() {
        memory.update(task_type, prompt);
    }
    memory
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_prompt_entry_creation() {
        let entry = PromptEntry::new("Test prompt");
        assert_eq!(entry.system_prompt, "Test prompt");
        assert_eq!(entry.usage_count, 0);
        assert!(entry.success_rate.is_none());
    }

    #[test]
    fn test_prompt_entry_usage() {
        let mut entry = PromptEntry::new("Test prompt");

        entry.record_usage(true);
        assert_eq!(entry.usage_count, 1);
        assert_eq!(entry.success_rate, Some(1.0));

        entry.record_usage(false);
        assert_eq!(entry.usage_count, 2);
        assert_eq!(entry.success_rate, Some(0.5));
    }

    #[test]
    fn test_prompt_memory_crud() {
        let mut memory = PromptMemory::new();

        // Create
        memory.update("test_type", "Test prompt");
        assert!(memory.contains("test_type"));

        // Read
        assert_eq!(memory.get_prompt("test_type"), Some("Test prompt"));

        // Update
        memory.update("test_type", "Updated prompt");
        assert_eq!(memory.get_prompt("test_type"), Some("Updated prompt"));

        // Delete
        memory.remove("test_type");
        assert!(!memory.contains("test_type"));
    }

    #[test]
    fn test_prompt_memory_persistence() {
        // Use temp directory from environment or current dir
        let temp_dir = env::temp_dir();
        let path = temp_dir.join("phone_agent_test_prompts.json");

        // Create and save
        let mut memory = PromptMemory::new();
        memory.update("type_a", "Prompt A");
        memory.update("type_b", "Prompt B");
        memory.save(&path).unwrap();

        // Load and verify
        let loaded = PromptMemory::load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get_prompt("type_a"), Some("Prompt A"));
        assert_eq!(loaded.get_prompt("type_b"), Some("Prompt B"));

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_default_prompt_memory() {
        let memory = create_default_prompt_memory();
        assert!(!memory.is_empty());
        assert!(memory.contains("微信操作"));
        assert!(memory.contains("通用任务"));
    }

    #[test]
    fn test_corrections_recording() {
        let mut memory = PromptMemory::new();
        memory.update("test_type", "Initial prompt");

        // Add corrections
        memory.add_correction("test_type", "Correction 1", None);
        memory.add_correction("test_type", "Correction 2", Some("Context".to_string()));

        assert_eq!(memory.pending_corrections("test_type"), 2);

        // Get corrections summary
        let summary = memory.get_corrections_summary("test_type").unwrap();
        assert!(summary.contains("Correction 1"));
        assert!(summary.contains("Correction 2"));
    }

    #[test]
    fn test_corrections_clear() {
        let mut memory = PromptMemory::new();
        memory.update("test_type", "Initial prompt");

        memory.add_correction("test_type", "Correction 1", None);
        memory.add_correction("test_type", "Correction 2", None);
        assert_eq!(memory.pending_corrections("test_type"), 2);

        // Clear corrections
        if let Some(entry) = memory.get_mut("test_type") {
            entry.clear_corrections();
        }
        assert_eq!(memory.pending_corrections("test_type"), 0);
    }

    #[test]
    fn test_task_types_summary() {
        let mut memory = PromptMemory::new();
        memory.update("微信操作", "WeChat prompt");
        memory.update("设置调整", "Settings prompt");

        let summary = memory.get_task_types_summary();
        assert!(summary.contains("微信操作"));
        assert!(summary.contains("设置调整"));
    }

    #[test]
    fn test_find_matching_task_type() {
        let mut memory = PromptMemory::new();
        memory.update("微信操作", "WeChat prompt");
        memory.update("微信消息", "WeChat message prompt");
        memory.update("设置调整", "Settings prompt");

        // Exact match
        assert_eq!(memory.find_matching_task_type("微信操作"), Some("微信操作".to_string()));

        // Partial match
        let result = memory.find_matching_task_type("微信");
        assert!(result.is_some());
        let matched = result.unwrap();
        assert!(matched.contains("微信"));
    }

    #[test]
    fn test_ensure_task_type() {
        let mut memory = PromptMemory::new();

        // Non-existent type should be created
        memory.ensure_task_type("new_type");
        assert!(memory.contains("new_type"));

        // Existing type should not be modified
        memory.update("existing", "Custom prompt");
        memory.ensure_task_type("existing");
        assert_eq!(memory.get_prompt("existing"), Some("Custom prompt"));
    }
}
