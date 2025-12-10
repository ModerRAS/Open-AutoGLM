//! # Phone Agent
//!
//! AI-powered agent for automating Android phone interactions.
//!
//! This library provides a Rust implementation of the AutoGLM phone agent,
//! which uses vision-language models to understand screen content and
//! automate Android device interactions via ADB.
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
//!     let agent = PhoneAgent::new(model_config, agent_config);
//!     let result = agent.run("打开微信").await?;
//!     
//!     println!("Task result: {}", result);
//!     Ok(())
//! }
//! ```

pub mod actions;
pub mod adb;
pub mod agent;
pub mod config;
pub mod model;

pub use agent::{AgentConfig, PhoneAgent, StepResult};
pub use model::{ModelClient, ModelConfig, ModelResponse};
