//! Model client module for AI inference.

mod client;

pub use client::{
    MessageBuilder, ModelClient, ModelConfig, ModelResponse, DEFAULT_MAX_RETRIES,
    DEFAULT_RETRY_DELAY_SECS,
};
