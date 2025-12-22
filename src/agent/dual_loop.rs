//! Dual loop runner for coordinating Planner and Executor loops.
//!
//! This module provides the main entry point for running the dual-loop
//! architecture, where Planner and Executor run in separate async tasks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::interval;

use super::executor::{ExecutorFeedback, ExecutorStatus};
use super::planner::PlannerAgent;

/// Configuration for the dual loop runner.
#[derive(Debug, Clone)]
pub struct DualLoopConfig {
    /// Interval between Planner ticks (milliseconds).
    pub planner_interval_ms: u64,
    /// Interval between Executor ticks (milliseconds).
    pub executor_interval_ms: u64,
    /// Whether to start immediately.
    pub auto_start: bool,
}

impl Default for DualLoopConfig {
    fn default() -> Self {
        Self {
            planner_interval_ms: 2000, // 2 seconds
            executor_interval_ms: 500, // 0.5 seconds
            auto_start: true,
        }
    }
}

impl DualLoopConfig {
    /// Set the planner interval.
    pub fn with_planner_interval(mut self, ms: u64) -> Self {
        self.planner_interval_ms = ms;
        self
    }

    /// Set the executor interval.
    pub fn with_executor_interval(mut self, ms: u64) -> Self {
        self.executor_interval_ms = ms;
        self
    }

    /// Set auto-start.
    pub fn with_auto_start(mut self, auto_start: bool) -> Self {
        self.auto_start = auto_start;
        self
    }
}

/// Handle for controlling the dual loop from outside.
#[derive(Clone)]
pub struct DualLoopHandle {
    /// Channel for sending user input.
    user_input_tx: mpsc::Sender<String>,
    /// Channel for sending control commands.
    control_tx: mpsc::Sender<ControlCommand>,
    /// Running flag.
    running: Arc<AtomicBool>,
}

impl DualLoopHandle {
    /// Send user input to the planner.
    pub async fn send_user_input(&self, input: String) -> Result<(), DualLoopError> {
        self.user_input_tx
            .send(input)
            .await
            .map_err(|_| DualLoopError::ChannelClosed)
    }

    /// Send user input (non-async version).
    pub fn send_user_input_blocking(&self, input: String) -> Result<(), DualLoopError> {
        self.user_input_tx
            .blocking_send(input)
            .map_err(|_| DualLoopError::ChannelClosed)
    }

    /// Stop the dual loop.
    pub async fn stop(&self) -> Result<(), DualLoopError> {
        self.running.store(false, Ordering::SeqCst);
        self.control_tx
            .send(ControlCommand::Stop)
            .await
            .map_err(|_| DualLoopError::ChannelClosed)
    }

    /// Pause the dual loop.
    pub async fn pause(&self) -> Result<(), DualLoopError> {
        self.control_tx
            .send(ControlCommand::Pause)
            .await
            .map_err(|_| DualLoopError::ChannelClosed)
    }

    /// Resume the dual loop.
    pub async fn resume(&self) -> Result<(), DualLoopError> {
        self.control_tx
            .send(ControlCommand::Resume)
            .await
            .map_err(|_| DualLoopError::ChannelClosed)
    }

    /// Check if the loop is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Control commands for the dual loop.
#[derive(Debug, Clone)]
enum ControlCommand {
    Stop,
    Pause,
    Resume,
}

/// Errors from the dual loop.
#[derive(Debug, Clone)]
pub enum DualLoopError {
    ChannelClosed,
    AlreadyRunning,
    NotRunning,
}

impl std::fmt::Display for DualLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChannelClosed => write!(f, "Channel closed"),
            Self::AlreadyRunning => write!(f, "Already running"),
            Self::NotRunning => write!(f, "Not running"),
        }
    }
}

impl std::error::Error for DualLoopError {}

/// Callback for executor feedback events.
pub type FeedbackCallback = Box<dyn Fn(&ExecutorFeedback) + Send + Sync>;

/// Dual loop runner that coordinates Planner and Executor.
pub struct DualLoopRunner {
    /// The planner agent (owns the executor).
    planner: PlannerAgent,
    /// Configuration.
    config: DualLoopConfig,
    /// Running flag.
    running: Arc<AtomicBool>,
    /// Paused flag.
    paused: Arc<AtomicBool>,
    /// Optional feedback callback.
    feedback_callback: Option<FeedbackCallback>,
}

impl DualLoopRunner {
    /// Create a new dual loop runner.
    pub fn new(planner: PlannerAgent, config: DualLoopConfig) -> Self {
        Self {
            planner,
            config,
            running: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            feedback_callback: None,
        }
    }

    /// Set a callback for executor feedback.
    pub fn with_feedback_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ExecutorFeedback) + Send + Sync + 'static,
    {
        self.feedback_callback = Some(Box::new(callback));
        self
    }

    /// Create a handle for external control.
    fn create_handle(
        &self,
        user_input_tx: mpsc::Sender<String>,
        control_tx: mpsc::Sender<ControlCommand>,
    ) -> DualLoopHandle {
        DualLoopHandle {
            user_input_tx,
            control_tx,
            running: self.running.clone(),
        }
    }

    /// Run the dual loop.
    /// Returns a handle for external control.
    pub async fn run(mut self) -> DualLoopHandle {
        // Create channels
        let (user_input_tx, mut user_input_rx) = mpsc::channel::<String>(100);
        let (control_tx, mut control_rx) = mpsc::channel::<ControlCommand>(10);

        // Create handle before moving self
        let handle = self.create_handle(user_input_tx, control_tx);

        // Set running flag
        self.running.store(true, Ordering::SeqCst);

        // Start planner
        self.planner.start();

        // Spawn the main loop
        tokio::spawn(async move {
            let mut planner_interval =
                interval(Duration::from_millis(self.config.planner_interval_ms));
            let mut executor_interval =
                interval(Duration::from_millis(self.config.executor_interval_ms));

            loop {
                // PRIORITY 1: Always check for user input first (non-blocking)
                // This ensures user commands are processed immediately
                while let Ok(input) = user_input_rx.try_recv() {
                    println!("\n📥 [用户输入已接收] {}", input);
                    self.planner.queue_user_input(input);
                    // Process user input immediately
                    if !self.paused.load(Ordering::SeqCst) {
                        let _ = self.planner.tick_planner().await;
                    }
                }

                tokio::select! {
                    biased; // Use biased selection to prioritize in order

                    // Handle control commands (highest priority)
                    Some(cmd) = control_rx.recv() => {
                        match cmd {
                            ControlCommand::Stop => {
                                tracing::info!("Dual loop stopping...");
                                self.planner.stop();
                                self.running.store(false, Ordering::SeqCst);
                                break;
                            }
                            ControlCommand::Pause => {
                                tracing::info!("Dual loop paused");
                                self.paused.store(true, Ordering::SeqCst);
                            }
                            ControlCommand::Resume => {
                                tracing::info!("Dual loop resumed");
                                self.paused.store(false, Ordering::SeqCst);
                            }
                        }
                    }

                    // Executor tick (faster)
                    _ = executor_interval.tick() => {
                        if !self.paused.load(Ordering::SeqCst) {
                            let feedback = self.planner.tick_executor().await;

                            // Call feedback callback if set
                            if let Some(ref callback) = self.feedback_callback {
                                callback(&feedback);
                            }

                            // Log significant events
                            match &feedback.status {
                                ExecutorStatus::Completed => {
                                    tracing::info!("Executor completed task");
                                }
                                ExecutorStatus::Failed(reason) => {
                                    tracing::error!("Executor failed: {}", reason);
                                }
                                ExecutorStatus::Stuck => {
                                    tracing::warn!("Executor stuck detected");
                                }
                                _ => {}
                            }
                        }
                    }

                    // Planner tick (slower)
                    _ = planner_interval.tick() => {
                        if !self.paused.load(Ordering::SeqCst) {
                            let should_continue = self.planner.tick_planner().await;

                            if !should_continue && !self.planner.has_pending_input() {
                                tracing::info!("Planner has no more work, waiting for input...");
                            }
                        }
                    }

                    // Check if we should exit
                    else => {
                        if !self.running.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                }

                // Check running flag
                if !self.running.load(Ordering::SeqCst) {
                    break;
                }
            }

            tracing::info!("Dual loop stopped");
        });

        handle
    }

    /// Run the dual loop with a simple synchronous API.
    /// This is a convenience method that blocks until the loop completes.
    pub async fn run_blocking(mut self) {
        self.running.store(true, Ordering::SeqCst);
        self.planner.start();

        let mut planner_interval = interval(Duration::from_millis(self.config.planner_interval_ms));
        let mut executor_interval =
            interval(Duration::from_millis(self.config.executor_interval_ms));

        loop {
            tokio::select! {
                // Executor tick
                _ = executor_interval.tick() => {
                    if !self.paused.load(Ordering::SeqCst) {
                        let feedback = self.planner.tick_executor().await;

                        if let Some(ref callback) = self.feedback_callback {
                            callback(&feedback);
                        }
                    }
                }

                // Planner tick
                _ = planner_interval.tick() => {
                    if !self.paused.load(Ordering::SeqCst) {
                        let should_continue = self.planner.tick_planner().await;

                        if !should_continue && !self.planner.has_pending_input() {
                            // All done
                            break;
                        }
                    }
                }
            }

            if !self.running.load(Ordering::SeqCst) {
                break;
            }
        }

        self.planner.stop();
        tracing::info!("Dual loop completed");
    }
}

/// Builder for creating a dual loop setup.
pub struct DualLoopBuilder {
    planner: Option<PlannerAgent>,
    config: DualLoopConfig,
    feedback_callback: Option<FeedbackCallback>,
}

impl DualLoopBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            planner: None,
            config: DualLoopConfig::default(),
            feedback_callback: None,
        }
    }

    /// Set the planner.
    pub fn with_planner(mut self, planner: PlannerAgent) -> Self {
        self.planner = Some(planner);
        self
    }

    /// Set the configuration.
    pub fn with_config(mut self, config: DualLoopConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the planner interval.
    pub fn with_planner_interval(mut self, ms: u64) -> Self {
        self.config.planner_interval_ms = ms;
        self
    }

    /// Set the executor interval.
    pub fn with_executor_interval(mut self, ms: u64) -> Self {
        self.config.executor_interval_ms = ms;
        self
    }

    /// Set a feedback callback.
    pub fn with_feedback_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ExecutorFeedback) + Send + Sync + 'static,
    {
        self.feedback_callback = Some(Box::new(callback));
        self
    }

    /// Build the dual loop runner.
    pub fn build(self) -> Result<DualLoopRunner, &'static str> {
        let planner = self.planner.ok_or("Planner is required")?;
        let mut runner = DualLoopRunner::new(planner, self.config);
        runner.feedback_callback = self.feedback_callback;
        Ok(runner)
    }
}

impl Default for DualLoopBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_loop_config_default() {
        let config = DualLoopConfig::default();
        assert_eq!(config.planner_interval_ms, 2000);
        assert_eq!(config.executor_interval_ms, 500);
        assert!(config.auto_start);
    }

    #[test]
    fn test_dual_loop_config_builder() {
        let config = DualLoopConfig::default()
            .with_planner_interval(3000)
            .with_executor_interval(1000)
            .with_auto_start(false);

        assert_eq!(config.planner_interval_ms, 3000);
        assert_eq!(config.executor_interval_ms, 1000);
        assert!(!config.auto_start);
    }
}
