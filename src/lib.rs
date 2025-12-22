// Copyright 2025 Zhipu AI (Original Python implementation)
// Copyright 2025 ModerRAS (Rust implementation)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Phone Agent
//!
//! AI-powered agent for automating Android phone interactions.
//!
//! This library provides a Rust implementation of the AutoGLM phone agent,
//! which uses vision-language models to understand screen content and
//! automate Android device interactions via ADB.
//!
//! This is a Rust rewrite of [Open-AutoGLM](https://github.com/zai-org/Open-AutoGLM)
//! by Zhipu AI.
//!
//! ## Single Loop Example (Original)
//!
//! ```rust,no_run
//! use phone_agent::{PhoneAgent, AgentConfig, ModelConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let model_config = ModelConfig::default();
//!     let agent_config = AgentConfig::default();
//!     
//!     let mut agent = PhoneAgent::new(model_config, agent_config, None, None);
//!     let result = agent.run("打开微信").await?;
//!
//!     println!("Task result: {}", result);
//!     Ok(())
//! }
//! ```
//!
//! ## Dual Loop Example (New)
//!
//! ```rust,no_run
//! use phone_agent::{
//!     AgentConfig, ModelConfig, PlannerAgent, PlannerConfig,
//!     DualLoopRunner, DualLoopConfig,
//! };
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Configure planner (outer loop) with DeepSeek or similar
//!     let planner_config = PlannerConfig::default()
//!         .with_model_config(
//!             ModelConfig::default()
//!                 .with_base_url("https://api.deepseek.com/v1")
//!                 .with_model_name("deepseek-chat")
//!         );
//!     
//!     // Configure executor (inner loop) with AutoGLM
//!     let executor_model_config = ModelConfig::default()
//!         .with_base_url("http://localhost:8000/v1")
//!         .with_model_name("autoglm-phone-9b");
//!     let executor_agent_config = AgentConfig::default();
//!     
//!     // Create planner
//!     let planner = PlannerAgent::new(
//!         planner_config,
//!         executor_model_config,
//!         executor_agent_config,
//!     );
//!     
//!     // Create and run dual loop
//!     let config = DualLoopConfig::default();
//!     let runner = DualLoopRunner::new(planner, config);
//!     let handle = runner.run().await;
//!     
//!     // Send user input
//!     handle.send_user_input("打开微信并发送消息".to_string()).await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod actions;
pub mod adb;
pub mod agent;
pub mod calibration;
pub mod config;
pub mod gui;
pub mod model;
pub mod settings;

pub use actions::{CoordinateSystem, DEFAULT_COORDINATE_SCALE, RELATIVE_COORDINATE_MAX};

// Single loop exports (original)
pub use agent::{AgentConfig, AgentError, PhoneAgent, StepResult};

// Dual loop exports (new)
pub use agent::{
    DualLoopBuilder, DualLoopConfig, DualLoopError, DualLoopHandle, DualLoopRunner,
    ExecutorCommand, ExecutorFeedback, ExecutorStatus, ExecutorWrapper,
    PlannerAction, PlannerAgent, PlannerConfig,
    PromptEntry, PromptMemory, PromptMemoryError,
    TodoItem, TodoList, TodoStats, TodoStatus,
    create_default_prompt_memory,
};

pub use calibration::{
    CalibrationConfig, CalibrationMode, CalibrationResult, CoordinateCalibrator,
};
pub use model::{ModelClient, ModelConfig, ModelResponse};
pub use settings::AppSettings;
