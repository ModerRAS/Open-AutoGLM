//! Agent module for orchestrating phone automation.
//!
//! This module provides both single-loop (original) and dual-loop architectures:
//!
//! ## Single Loop (Original)
//! - `PhoneAgent`: Direct agent for phone automation
//!
//! ## Dual Loop (New)
//! - `ExecutorWrapper`: Wraps PhoneAgent with control interfaces
//! - `PlannerAgent`: Outer loop for task planning and supervision
//! - `DualLoopRunner`: Coordinates both loops
//! - `TodoList`: Task management
//! - `PromptMemory`: Optimized prompt storage by task type

mod dual_loop;
mod executor;
mod phone_agent;
mod planner;
mod prompt_memory;
mod todo;

// Single loop exports (original)
pub use phone_agent::{AgentConfig, AgentError, PhoneAgent, StepResult};

// Dual loop exports (new)
pub use dual_loop::{
    DualLoopBuilder, DualLoopConfig, DualLoopError, DualLoopHandle, DualLoopRunner,
};
pub use executor::{
    ExecutorCommand, ExecutorFeedback, ExecutorStatus, ExecutorWrapper, StepResultSummary,
    DEFAULT_STUCK_THRESHOLD,
};
pub use planner::{PlannerAction, PlannerAgent, PlannerConfig};
pub use prompt_memory::{
    create_default_prompt_memory, PromptEntry, PromptMemory, PromptMemoryError,
};
pub use todo::{TodoItem, TodoList, TodoStats, TodoStatus};
