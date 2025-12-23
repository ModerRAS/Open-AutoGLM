//! Executor wrapper for controlling PhoneAgent execution.
//!
//! This module wraps the PhoneAgent to provide external control interfaces
//! for the dual-loop architecture.

use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::phone_agent::{AgentConfig, PhoneAgent, StepResult};
use crate::model::ModelConfig;

/// Executor status enumeration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutorStatus {
    /// Idle, waiting for task.
    Idle,
    /// Currently executing.
    Running,
    /// Paused by external command.
    Paused,
    /// Stuck (no screen change detected).
    Stuck,
    /// Task completed successfully.
    Completed,
    /// Task failed with error message.
    Failed(String),
}

impl Default for ExecutorStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Commands that can be sent from Planner to Executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutorCommand {
    /// Start a new task.
    StartTask {
        task_id: String,
        description: String,
        system_prompt: Option<String>,
    },
    /// Pause execution.
    Pause,
    /// Resume execution.
    Resume,
    /// Inject a prompt to help with stuck situations.
    InjectPrompt { content: String },
    /// Reset the executor context.
    ResetContext,
    /// Stop the current task.
    Stop,
}

/// Feedback from Executor to Planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorFeedback {
    /// Current task ID (if any).
    pub task_id: Option<String>,
    /// Current step count.
    pub step_count: u32,
    /// Current executor status.
    pub status: ExecutorStatus,
    /// Last step result (if any).
    pub last_result: Option<StepResultSummary>,
    /// Whether the screen changed since last step.
    pub screen_changed: bool,
    /// Unix timestamp of this feedback.
    pub timestamp: u64,
    /// Whether the executor detected abnormal output (context overflow).
    #[serde(default)]
    pub context_overflow_detected: bool,
    /// Consecutive parse error count.
    #[serde(default)]
    pub consecutive_parse_errors: u32,
}

/// Summarized step result for feedback (without large data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResultSummary {
    pub success: bool,
    pub finished: bool,
    pub thinking: String,
    pub message: Option<String>,
    pub action_type: Option<String>,
}

impl From<&StepResult> for StepResultSummary {
    fn from(result: &StepResult) -> Self {
        let action_type = result.action.as_ref().and_then(|a| {
            // Extract action type from the action JSON
            a.get("_metadata")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    // Try to get action name from keys
                    if let Some(obj) = a.as_object() {
                        obj.keys()
                            .find(|k| *k != "_metadata" && *k != "message")
                            .cloned()
                    } else {
                        None
                    }
                })
        });

        Self {
            success: result.success,
            finished: result.finished,
            thinking: result.thinking.clone(),
            message: result.message.clone(),
            action_type,
        }
    }
}

/// Default stuck threshold (consecutive unchanged screens).
pub const DEFAULT_STUCK_THRESHOLD: u32 = 3;

/// Default parse error threshold before suggesting context reset.
pub const DEFAULT_PARSE_ERROR_THRESHOLD: u32 = 3;

/// Executor wrapper that provides control interfaces for PhoneAgent.
pub struct ExecutorWrapper {
    /// Inner PhoneAgent instance.
    inner: PhoneAgent,
    /// Model config for recreating agent.
    model_config: ModelConfig,
    /// Agent config for recreating agent.
    agent_config: AgentConfig,
    /// Current executor status.
    status: ExecutorStatus,
    /// Current task ID.
    current_task_id: Option<String>,
    /// Current task description.
    current_task_description: Option<String>,
    /// Command queue from Planner.
    command_queue: VecDeque<ExecutorCommand>,
    /// Last screenshot hash for stuck detection.
    last_screen_hash: Option<u64>,
    /// Consecutive unchanged screen count.
    stuck_count: u32,
    /// Stuck detection threshold.
    stuck_threshold: u32,
    /// Pending prompt injection.
    pending_prompt: Option<String>,
    /// Consecutive parse error count (indicates potential context overflow).
    consecutive_parse_errors: u32,
}

impl ExecutorWrapper {
    /// Create a new ExecutorWrapper.
    pub fn new(model_config: ModelConfig, agent_config: AgentConfig) -> Self {
        let inner = PhoneAgent::new(model_config.clone(), agent_config.clone(), None, None);

        Self {
            inner,
            model_config,
            agent_config,
            status: ExecutorStatus::Idle,
            current_task_id: None,
            current_task_description: None,
            command_queue: VecDeque::new(),
            last_screen_hash: None,
            stuck_count: 0,
            stuck_threshold: DEFAULT_STUCK_THRESHOLD,
            pending_prompt: None,
            consecutive_parse_errors: 0,
        }
    }

    /// Set the stuck detection threshold.
    pub fn with_stuck_threshold(mut self, threshold: u32) -> Self {
        self.stuck_threshold = threshold;
        self
    }

    /// Get current status.
    pub fn status(&self) -> &ExecutorStatus {
        &self.status
    }

    /// Set status (mainly for testing purposes).
    #[cfg(test)]
    pub fn set_status(&mut self, status: ExecutorStatus) {
        self.status = status;
    }

    /// Get current task ID.
    pub fn task_id(&self) -> Option<&str> {
        self.current_task_id.as_deref()
    }

    /// Get current step count.
    pub fn step_count(&self) -> u32 {
        self.inner.step_count()
    }

    /// Enqueue a command from Planner.
    pub fn enqueue(&mut self, cmd: ExecutorCommand) {
        self.command_queue.push_back(cmd);
    }

    /// Check if there are pending commands.
    pub fn has_pending_commands(&self) -> bool {
        !self.command_queue.is_empty()
    }

    /// Process the next command in queue.
    /// Returns true if a command was processed.
    pub fn process_next_command(&mut self) -> bool {
        if let Some(cmd) = self.command_queue.pop_front() {
            self.handle_command(cmd);
            true
        } else {
            false
        }
    }

    /// Handle a single command.
    fn handle_command(&mut self, cmd: ExecutorCommand) {
        match cmd {
            ExecutorCommand::StartTask {
                task_id,
                description,
                system_prompt,
            } => {
                self.start_task(task_id, description, system_prompt);
            }
            ExecutorCommand::Pause => {
                if self.status == ExecutorStatus::Running {
                    self.status = ExecutorStatus::Paused;
                    tracing::info!("Executor paused");
                }
            }
            ExecutorCommand::Resume => {
                if self.status == ExecutorStatus::Paused {
                    self.status = ExecutorStatus::Running;
                    tracing::info!("Executor resumed");
                }
            }
            ExecutorCommand::InjectPrompt { content } => {
                self.pending_prompt = Some(content);
                tracing::info!("Prompt injection queued");
                // If stuck, completed, or idle - change status back to running
                // This allows user corrections to wake up the executor
                match self.status {
                    ExecutorStatus::Stuck => {
                        self.status = ExecutorStatus::Running;
                        self.stuck_count = 0;
                        tracing::info!("Executor resumed from stuck state via prompt injection");
                    }
                    ExecutorStatus::Completed => {
                        // User thinks task is not actually complete, resume execution
                        self.status = ExecutorStatus::Running;
                        tracing::info!("Executor resumed from completed state via prompt injection (user correction)");
                    }
                    ExecutorStatus::Idle => {
                        // If we have a task context, resume; otherwise log warning
                        if self.current_task_id.is_some() {
                            self.status = ExecutorStatus::Running;
                            tracing::info!("Executor resumed from idle state via prompt injection");
                        } else {
                            tracing::warn!("Prompt injection received but no task context exists");
                        }
                    }
                    _ => {
                        // Running or Paused - just queue the prompt
                    }
                }
            }
            ExecutorCommand::ResetContext => {
                self.reset_context();
            }
            ExecutorCommand::Stop => {
                self.status = ExecutorStatus::Idle;
                self.current_task_id = None;
                self.current_task_description = None;
                self.inner.reset();
                tracing::info!("Executor stopped");
            }
        }
    }

    /// Start a new task.
    fn start_task(&mut self, task_id: String, description: String, system_prompt: Option<String>) {
        // Reset state
        self.inner.reset();
        self.last_screen_hash = None;
        self.stuck_count = 0;
        self.pending_prompt = None;

        // Update agent config with custom system prompt if provided
        if let Some(prompt) = system_prompt {
            self.agent_config.system_prompt = Some(prompt);
            // Recreate inner agent with new config
            self.inner = PhoneAgent::new(
                self.model_config.clone(),
                self.agent_config.clone(),
                None,
                None,
            );
        }

        self.current_task_id = Some(task_id.clone());
        self.current_task_description = Some(description);
        self.status = ExecutorStatus::Running;

        tracing::info!("Executor started task: {}", task_id);
    }

    /// Reset context without stopping.
    fn reset_context(&mut self) {
        self.inner.reset();
        self.last_screen_hash = None;
        self.stuck_count = 0;
        self.pending_prompt = None;
        tracing::info!("Executor context reset");
    }

    /// Execute a single tick of the executor loop.
    /// Returns feedback for the Planner.
    pub async fn tick(&mut self) -> ExecutorFeedback {
        while self.process_next_command() {}

        if !matches!(self.status, ExecutorStatus::Running) {
            return self.create_feedback(None, true, false);
        }

        let task = if self.inner.step_count() == 0 {
            self.current_task_description.clone()
        } else if let Some(prompt) = self.pending_prompt.take() {
            Some(prompt)
        } else {
            None
        };

        match self.inner.step(task.as_deref()).await {
            Ok(step_result) => {
                let is_parse_error = step_result
                    .action
                    .as_ref()
                    .and_then(|a| a.get("error"))
                    .and_then(|e| e.as_str())
                    .map(|s| s == "parse_failed")
                    .unwrap_or(false);

                if is_parse_error {
                    self.consecutive_parse_errors += 1;
                    tracing::warn!(
                        "Executor parse error (consecutive: {})",
                        self.consecutive_parse_errors
                    );

                    if self.consecutive_parse_errors >= DEFAULT_PARSE_ERROR_THRESHOLD {
                        tracing::error!(
                            "Executor context overflow detected: {} consecutive parse errors",
                            self.consecutive_parse_errors
                        );
                        self.reset_context_on_error();
                    }
                } else {
                    self.consecutive_parse_errors = 0;
                }

                let screen_hash = self.calculate_context_hash();
                let screen_changed = self.detect_screen_change(screen_hash);

                if !screen_changed && !is_parse_error {
                    self.stuck_count += 1;
                    if self.stuck_count >= self.stuck_threshold {
                        self.status = ExecutorStatus::Stuck;
                        tracing::warn!(
                            "Executor stuck: {} consecutive unchanged screens",
                            self.stuck_count
                        );
                    }
                } else if !is_parse_error {
                    self.stuck_count = 0;
                }

                if step_result.finished {
                    self.status = ExecutorStatus::Completed;
                    tracing::info!("Executor completed task");
                }

                let context_overflow =
                    self.consecutive_parse_errors >= DEFAULT_PARSE_ERROR_THRESHOLD;

                self.log_context_snapshot(Some(&step_result), context_overflow);

                self.create_feedback(Some(&step_result), screen_changed, context_overflow)
            }
            Err(e) => {
                self.status = ExecutorStatus::Failed(e.to_string());
                tracing::error!("Executor failed: {}", e);
                self.log_context_snapshot(None, false);
                self.create_feedback(None, true, false)
            }
        }
    }
    /// Hash a small slice of the context (text only) to detect screen changes.
    fn calculate_context_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        let context = self.inner.context();
        let recent_count = context.len().min(3);
        for msg in context.iter().rev().take(recent_count) {
            if let Some(content) = msg.get("content") {
                let content_str = content.to_string();
                let cleaned: String = content_str.chars().take(1000).collect();
                cleaned.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Reset context to recover and avoid runaway tokens.
    fn reset_context_on_error(&mut self) {
        self.inner.reset();
        self.stuck_count = 0;
        self.pending_prompt =
            Some("请严格输出 do(...) 或 finish(...)，不要重复总结，直接给动作指令。".to_string());
        self.consecutive_parse_errors = 0;
        tracing::info!("Executor context reset due to parse error");
    }

    /// Append a slim context snapshot to a log file for debugging.
    fn log_context_snapshot(&self, result: Option<&StepResult>, context_overflow: bool) {
        let dirs: Option<PathBuf> =
            ProjectDirs::from("com", "moderras", "phone-agent").map(|d| d.data_dir().join("logs"));

        let Some(dir) = dirs else { return }; // no directory available
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }

        let path = dir.join("executor_context.log");

        let context_slim: Vec<String> = self
            .inner
            .context()
            .iter()
            .rev()
            .take(4)
            .map(|msg| Self::summarize_message(msg))
            .collect();

        let action_summary =
            result.and_then(|r| r.action.as_ref().map(|a| Self::summarize_message(a)));
        let thinking = result.map(|r| Self::shorten(&r.thinking));
        let message = result.and_then(|r| r.message.clone());

        let entry = serde_json::json!({
            "timestamp": current_timestamp(),
            "step": self.inner.step_count(),
            "status": format!("{:?}", self.status),
            "context_overflow": context_overflow,
            "consecutive_parse_errors": self.consecutive_parse_errors,
            "context": context_slim,
            "thinking": thinking,
            "message": message,
            "action": action_summary,
        });

        if let Ok(line) = serde_json::to_string(&entry) {
            let _ = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| writeln!(f, "{}", line));
        }
    }

    fn summarize_message(val: &Value) -> String {
        if let Some(obj) = val.as_object() {
            let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("");

            let content = obj.get("content");

            if let Some(Value::String(s)) = content {
                return format!("{}: {}", role, Self::shorten(s));
            }

            if let Some(Value::Array(arr)) = content {
                let texts: Vec<String> = arr
                    .iter()
                    .filter_map(|item| {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            Some(Self::shorten(text))
                        } else if let Some(txt) = item.get("content").and_then(|t| t.as_str()) {
                            Some(Self::shorten(txt))
                        } else {
                            None
                        }
                    })
                    .collect();
                if !texts.is_empty() {
                    return format!("{}: {}", role, texts.join(" | "));
                }
            }

            return Self::shorten(&val.to_string());
        }

        Self::shorten(&val.to_string())
    }

    fn shorten(s: &str) -> String {
        const MAX: usize = 400;
        if s.chars().count() > MAX {
            let mut iter = s.chars();
            let prefix: String = iter.by_ref().take(MAX).collect();
            let remaining = iter.count();
            format!("{}... <{} chars truncated>", prefix, remaining)
        } else {
            s.to_string()
        }
    }

    /// Detect if screen changed based on hash.
    fn detect_screen_change(&mut self, current_hash: u64) -> bool {
        let changed = match self.last_screen_hash {
            Some(last_hash) => last_hash != current_hash,
            None => true,
        };
        self.last_screen_hash = Some(current_hash);
        changed
    }

    /// Create feedback for Planner.
    fn create_feedback(
        &self,
        result: Option<&StepResult>,
        screen_changed: bool,
        context_overflow: bool,
    ) -> ExecutorFeedback {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        ExecutorFeedback {
            task_id: self.current_task_id.clone(),
            step_count: self.inner.step_count(),
            status: self.status.clone(),
            last_result: result.map(StepResultSummary::from),
            screen_changed,
            timestamp,
            context_overflow_detected: context_overflow,
            consecutive_parse_errors: self.consecutive_parse_errors,
        }
    }

    /// Get the inner agent's context (for debugging/logging).
    pub fn context(&self) -> &[Value] {
        self.inner.context()
    }
}

/// Get current Unix timestamp.
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_status_default() {
        let status = ExecutorStatus::default();
        assert_eq!(status, ExecutorStatus::Idle);
    }

    #[test]
    fn test_executor_command_queue() {
        let model_config = ModelConfig::default();
        let agent_config = AgentConfig::default();
        let mut executor = ExecutorWrapper::new(model_config, agent_config);

        executor.enqueue(ExecutorCommand::Pause);
        executor.enqueue(ExecutorCommand::Resume);

        assert!(executor.has_pending_commands());
        assert!(executor.process_next_command());
        assert!(executor.has_pending_commands());
        assert!(executor.process_next_command());
        assert!(!executor.has_pending_commands());
    }

    #[test]
    fn test_executor_pause_resume() {
        let model_config = ModelConfig::default();
        let agent_config = AgentConfig::default();
        let mut executor = ExecutorWrapper::new(model_config, agent_config);

        // Start a task first
        executor.enqueue(ExecutorCommand::StartTask {
            task_id: "test".to_string(),
            description: "Test task".to_string(),
            system_prompt: None,
        });
        executor.process_next_command();
        assert_eq!(*executor.status(), ExecutorStatus::Running);

        // Pause
        executor.enqueue(ExecutorCommand::Pause);
        executor.process_next_command();
        assert_eq!(*executor.status(), ExecutorStatus::Paused);

        // Resume
        executor.enqueue(ExecutorCommand::Resume);
        executor.process_next_command();
        assert_eq!(*executor.status(), ExecutorStatus::Running);
    }

    #[test]
    fn test_executor_inject_prompt_wakes_completed() {
        let model_config = ModelConfig::default();
        let agent_config = AgentConfig::default();
        let mut executor = ExecutorWrapper::new(model_config, agent_config);

        // Start and complete a task
        executor.enqueue(ExecutorCommand::StartTask {
            task_id: "test".to_string(),
            description: "Test task".to_string(),
            system_prompt: None,
        });
        executor.process_next_command();

        // Simulate completion
        executor.set_status(ExecutorStatus::Completed);
        assert_eq!(*executor.status(), ExecutorStatus::Completed);

        // Inject prompt should wake it up
        executor.enqueue(ExecutorCommand::InjectPrompt {
            content: "Continue please".to_string(),
        });
        executor.process_next_command();
        assert_eq!(*executor.status(), ExecutorStatus::Running);
    }

    #[test]
    fn test_executor_inject_prompt_wakes_idle() {
        let model_config = ModelConfig::default();
        let agent_config = AgentConfig::default();
        let mut executor = ExecutorWrapper::new(model_config, agent_config);

        // Start with a task (creates context)
        executor.enqueue(ExecutorCommand::StartTask {
            task_id: "test".to_string(),
            description: "Test task".to_string(),
            system_prompt: None,
        });
        executor.process_next_command();

        // Go to Idle
        executor.set_status(ExecutorStatus::Idle);
        assert_eq!(*executor.status(), ExecutorStatus::Idle);

        // Inject prompt should wake it up
        executor.enqueue(ExecutorCommand::InjectPrompt {
            content: "Do something".to_string(),
        });
        executor.process_next_command();
        assert_eq!(*executor.status(), ExecutorStatus::Running);
    }

    #[test]
    fn test_executor_reset_context() {
        let model_config = ModelConfig::default();
        let agent_config = AgentConfig::default();
        let mut executor = ExecutorWrapper::new(model_config, agent_config);

        executor.enqueue(ExecutorCommand::StartTask {
            task_id: "test".to_string(),
            description: "Test task".to_string(),
            system_prompt: None,
        });
        executor.process_next_command();

        // Reset - should clear context but keep running
        executor.enqueue(ExecutorCommand::ResetContext);
        executor.process_next_command();

        // Status stays Running (reset_context only clears context, doesn't change status)
        assert_eq!(*executor.status(), ExecutorStatus::Running);
    }

    #[test]
    fn test_executor_feedback_creation() {
        let model_config = ModelConfig::default();
        let agent_config = AgentConfig::default();
        let executor = ExecutorWrapper::new(model_config, agent_config);

        let feedback = executor.create_feedback(None, false, false);
        assert!(feedback.task_id.is_none());
        assert_eq!(feedback.step_count, 0);
        assert!(!feedback.screen_changed);
        assert!(!feedback.context_overflow_detected);
    }

    #[test]
    fn test_step_result_summary() {
        let result = StepResult {
            thinking: "I need to click".to_string(),
            action: Some(serde_json::json!({"type": "Tap", "x": 100, "y": 200})),
            message: Some("Tapping button".to_string()),
            success: true,
            finished: false,
        };

        let summary = StepResultSummary::from(&result);
        assert_eq!(summary.thinking, "I need to click");
        assert!(summary.success);
        assert!(!summary.finished);
    }
}
