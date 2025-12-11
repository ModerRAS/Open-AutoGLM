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
//! ## Example
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

pub mod actions;
pub mod adb;
pub mod agent;
pub mod calibration;
pub mod config;
pub mod model;

pub use actions::DEFAULT_COORDINATE_SCALE;
pub use agent::{AgentConfig, PhoneAgent, StepResult};
pub use calibration::{CalibrationConfig, CalibrationMode, CalibrationResult, CoordinateCalibrator};
pub use model::{ModelClient, ModelConfig, ModelResponse};
