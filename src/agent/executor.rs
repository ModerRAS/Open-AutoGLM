//! Executor wrapper for controlling PhoneAgent execution.
//!
//! This module wraps the PhoneAgent to provide external control interfaces
//! for the dual-loop architecture.

use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

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
        // Process any pending commands first
        while self.process_next_command() {}

        // Check if we should execute
        let should_execute = matches!(self.status, ExecutorStatus::Running);

        if !should_execute {
            return self.create_feedback(None, true, false);
        }

        // Determine the task for this step
        let task = if self.inner.step_count() == 0 {
            // First step - use task description
            self.current_task_description.clone()
        } else if let Some(prompt) = self.pending_prompt.take() {
            // Injected prompt
            Some(prompt)
        } else {
            // Continuation step
            None
        };

        // Execute step
        let result = self.inner.step(task.as_deref()).await;

        match result {
            Ok(step_result) => {
                // Check if this was a parse error (indicates potential context overflow)
                let is_parse_error = step_result.action
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
                    
                    // If too many consecutive parse errors, likely context overflow
                    if self.consecutive_parse_errors >= DEFAULT_PARSE_ERROR_THRESHOLD {
                        tracing::error!(
                            "Executor context overflow detected: {} consecutive parse errors",
                            self.consecutive_parse_errors
                        );
                        // Don't change status here, let Planner decide what to do
                    }
                } else {
                    // Reset parse error count on successful parse
                    self.consecutive_parse_errors = 0;
                }

                // Calculate screen hash from context (simplified - using thinking as proxy)
                let screen_hash = self.calculate_context_hash();
                let screen_changed = self.detect_screen_change(screen_hash);

                // Check for stuck
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

                // Check for completion
                if step_result.finished {
                    self.status = ExecutorStatus::Completed;
                    tracing::info!("Executor completed task");
                }

                let context_overflow = self.consecutive_parse_errors >= DEFAULT_PARSE_ERROR_THRESHOLD;
                self.create_feedback(Some(&step_result), screen_changed, context_overflow)
            }
            Err(e) => {
                self.status = ExecutorStatus::Failed(e.to_string());
                tracing::error!("Executor failed: {}", e);
                self.create_feedback(None, true, false)
            }
        }
    }

    /// Calculate a hash of the current context for change detection.
    fn calculate_context_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash the last few messages in context
        let context = self.inner.context();
        let recent_count = context.len().min(3);
        for msg in context.iter().rev().take(recent_count) {
            // Hash the content, excluding image data
            if let Some(content) = msg.get("content") {
                let content_str = content.to_string();
                // Remove base64 image data for hashing
                let cleaned: String = content_str
                    .chars()
                    .take(1000) // Only use first 1000 chars
                    .collect();
                cleaned.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Detect if screen changed based on hash.
    fn detect_screen_change(&mut self, current_hash: u64) -> bool {
        let changed = match self.last_screen_hash {
            Some(last_hash) => last_hash != current_hash,
            None => true, // First screen is always "changed"
        };
        self.last_screen_hash = Some(current_hash);
        changed
    }

    /// Create feedback for Planner.
    fn create_feedback(&self, result: Option<&StepResult>, screen_changed: bool, context_overflow: bool) -> ExecutorFeedback {
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
}
