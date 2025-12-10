//! Configuration module for Phone Agent.

mod apps;
mod i18n;
mod prompts;

pub use apps::APP_PACKAGES;
pub use i18n::{get_message, get_messages, Messages};
pub use prompts::{get_system_prompt, SYSTEM_PROMPT_EN, SYSTEM_PROMPT_ZH};
