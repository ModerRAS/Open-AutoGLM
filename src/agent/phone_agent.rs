//! Main PhoneAgent class for orchestrating phone automation.

use serde_json::Value;
use thiserror::Error;

use crate::actions::{
    parse_action, ActionHandler, ConfirmationCallback, CoordinateSystem, TakeoverCallback,
};
use crate::adb::{get_current_app, get_screenshot};
use crate::config::{
    get_messages, get_system_prompt, get_system_prompt_relative, get_system_prompt_with_resolution,
};
use crate::model::{MessageBuilder, ModelClient, ModelConfig};

/// Agent errors.
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Model error: {0}")]
    ModelError(String),
    #[error("Action error: {0}")]
    ActionError(String),
    #[error("Task required for first step")]
    TaskRequired,
    #[error("Max steps reached")]
    MaxStepsReached,
}

/// Configuration for the PhoneAgent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of steps before stopping.
    pub max_steps: u32,
    /// Optional ADB device ID for multi-device setups.
    pub device_id: Option<String>,
    /// Language code ("cn" for Chinese, "en" for English).
    pub lang: String,
    /// Custom system prompt (if None, uses default based on lang and coordinate system).
    pub system_prompt: Option<String>,
    /// Whether to print verbose output.
    pub verbose: bool,
    /// Scale factor for X coordinates (LLM output * scale = actual coordinate).
    /// Only used when coordinate_system is Absolute.
    pub scale_x: f64,
    /// Scale factor for Y coordinates (LLM output * scale = actual coordinate).
    /// Only used when coordinate_system is Absolute.
    pub scale_y: f64,
    /// Coordinate system mode (Relative 0-999 or Absolute pixel coordinates).
    pub coordinate_system: CoordinateSystem,
}

impl Default for AgentConfig {
    fn default() -> Self {
        use crate::actions::DEFAULT_COORDINATE_SCALE;
        Self {
            max_steps: 100,
            device_id: None,
            lang: "cn".to_string(),
            system_prompt: None,
            verbose: true,
            scale_x: DEFAULT_COORDINATE_SCALE,
            scale_y: DEFAULT_COORDINATE_SCALE,
            coordinate_system: CoordinateSystem::Absolute,
        }
    }
}

impl AgentConfig {
    /// Create a default config with relative coordinate system (0-999 range).
    /// This is the original AutoGLM-Phone coordinate system.
    pub fn relative() -> Self {
        Self {
            max_steps: 100,
            device_id: None,
            lang: "cn".to_string(),
            system_prompt: None,
            verbose: true,
            scale_x: 1.0,
            scale_y: 1.0,
            coordinate_system: CoordinateSystem::Relative,
        }
    }

    /// Create a new AgentConfig with custom device ID.
    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    /// Create a new AgentConfig with custom language.
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    /// Create a new AgentConfig with custom max steps.
    pub fn with_max_steps(mut self, max_steps: u32) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Create a new AgentConfig with verbose output disabled.
    pub fn quiet(mut self) -> Self {
        self.verbose = false;
        self
    }

    /// Set the coordinate scale factors (only used for Absolute coordinate system).
    /// LLM output coordinates will be multiplied by these factors.
    pub fn with_scale(mut self, scale_x: f64, scale_y: f64) -> Self {
        self.scale_x = scale_x;
        self.scale_y = scale_y;
        self
    }

    /// Set both X and Y scale factors to the same value.
    pub fn with_uniform_scale(mut self, scale: f64) -> Self {
        self.scale_x = scale;
        self.scale_y = scale;
        self
    }

    /// Set the coordinate system mode.
    pub fn with_coordinate_system(mut self, coordinate_system: CoordinateSystem) -> Self {
        self.coordinate_system = coordinate_system;
        self
    }

    /// Use relative coordinate system (0-999 range, original AutoGLM-Phone style).
    pub fn with_relative_coordinates(mut self) -> Self {
        self.coordinate_system = CoordinateSystem::Relative;
        self.scale_x = 1.0;
        self.scale_y = 1.0;
        self
    }

    /// Use absolute coordinate system (pixel coordinates with optional scaling).
    pub fn with_absolute_coordinates(mut self) -> Self {
        use crate::actions::DEFAULT_COORDINATE_SCALE;
        self.coordinate_system = CoordinateSystem::Absolute;
        self.scale_x = DEFAULT_COORDINATE_SCALE;
        self.scale_y = DEFAULT_COORDINATE_SCALE;
        self
    }

    /// Get the system prompt (custom or default based on language and coordinate system).
    /// This version doesn't include screen resolution information.
    pub fn get_system_prompt(&self) -> String {
        self.system_prompt
            .clone()
            .unwrap_or_else(|| match self.coordinate_system {
                CoordinateSystem::Relative => get_system_prompt_relative(&self.lang),
                CoordinateSystem::Absolute => get_system_prompt(&self.lang),
            })
    }

    /// Get the system prompt with screen resolution information.
    /// This is the preferred method when screen dimensions are known.
    pub fn get_system_prompt_with_resolution(&self, width: u32, height: u32) -> String {
        self.system_prompt
            .clone()
            .unwrap_or_else(|| match self.coordinate_system {
                CoordinateSystem::Relative => get_system_prompt_relative(&self.lang),
                CoordinateSystem::Absolute => {
                    get_system_prompt_with_resolution(&self.lang, width, height)
                }
            })
    }
}

/// Result of a single agent step.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Whether the action was successful.
    pub success: bool,
    /// Whether the task is finished.
    pub finished: bool,
    /// The action that was executed.
    pub action: Option<Value>,
    /// The thinking process from the model.
    pub thinking: String,
    /// Optional message (e.g., finish message).
    pub message: Option<String>,
}

/// AI-powered agent for automating Android phone interactions.
///
/// The agent uses a vision-language model to understand screen content
/// and decide on actions to complete user tasks.
///
/// # Example
///
/// ```rust,no_run
/// use phone_agent::{PhoneAgent, AgentConfig, ModelConfig};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let model_config = ModelConfig::default();
///     let agent_config = AgentConfig::default();
///     
///     let mut agent = PhoneAgent::new(model_config, agent_config, None, None);
///     let result = agent.run("打开微信").await?;
///     
///     println!("Task result: {}", result);
///     Ok(())
/// }
/// ```
pub struct PhoneAgent {
    model_client: ModelClient,
    agent_config: AgentConfig,
    action_handler: ActionHandler,
    context: Vec<Value>,
    step_count: u32,
}

impl PhoneAgent {
    /// Create a new PhoneAgent.
    ///
    /// # Arguments
    /// * `model_config` - Configuration for the AI model.
    /// * `agent_config` - Configuration for the agent behavior.
    /// * `confirmation_callback` - Optional callback for sensitive action confirmation.
    /// * `takeover_callback` - Optional callback for takeover requests.
    pub fn new(
        model_config: ModelConfig,
        agent_config: AgentConfig,
        confirmation_callback: Option<ConfirmationCallback>,
        takeover_callback: Option<TakeoverCallback>,
    ) -> Self {
        let action_handler = ActionHandler::with_options(
            agent_config.device_id.clone(),
            confirmation_callback,
            takeover_callback,
            agent_config.scale_x,
            agent_config.scale_y,
            agent_config.coordinate_system,
        );

        Self {
            model_client: ModelClient::new(model_config),
            agent_config,
            action_handler,
            context: Vec::new(),
            step_count: 0,
        }
    }

    /// Run the agent to complete a task.
    ///
    /// # Arguments
    /// * `task` - Natural language description of the task.
    ///
    /// # Returns
    /// Final message from the agent.
    pub async fn run(&mut self, task: &str) -> Result<String, AgentError> {
        self.reset();

        // First step with user prompt
        let result = self.execute_step(Some(task), true).await?;

        if result.finished {
            return Ok(result
                .message
                .unwrap_or_else(|| "Task completed".to_string()));
        }

        // Continue until finished or max steps reached
        while self.step_count < self.agent_config.max_steps {
            let result = self.execute_step(None, false).await?;

            if result.finished {
                return Ok(result
                    .message
                    .unwrap_or_else(|| "Task completed".to_string()));
            }
        }

        Err(AgentError::MaxStepsReached)
    }

    /// Execute a single step of the agent.
    ///
    /// Useful for manual control or debugging.
    ///
    /// # Arguments
    /// * `task` - Task description (only needed for first step).
    ///
    /// # Returns
    /// StepResult with step details.
    pub async fn step(&mut self, task: Option<&str>) -> Result<StepResult, AgentError> {
        let is_first = self.context.is_empty();

        if is_first && task.is_none() {
            return Err(AgentError::TaskRequired);
        }

        self.execute_step(task, is_first).await
    }

    /// Reset the agent state for a new task.
    pub fn reset(&mut self) {
        self.context.clear();
        self.step_count = 0;
    }

    /// Execute a single step of the agent loop.
    async fn execute_step(
        &mut self,
        user_prompt: Option<&str>,
        is_first: bool,
    ) -> Result<StepResult, AgentError> {
        self.step_count += 1;

        // Capture current screen state
        let screenshot = get_screenshot(self.agent_config.device_id.as_deref());
        let current_app = get_current_app(self.agent_config.device_id.as_deref());

        // Build messages
        if is_first {
            // Use system prompt with screen resolution for absolute coordinate system
            self.context.push(MessageBuilder::create_system_message(
                &self
                    .agent_config
                    .get_system_prompt_with_resolution(screenshot.width, screenshot.height),
            ));

            let screen_info = MessageBuilder::build_screen_info(&current_app);
            let text_content = format!("{}\n\n{}", user_prompt.unwrap_or(""), screen_info);

            self.context.push(MessageBuilder::create_user_message(
                &text_content,
                Some(&screenshot.base64_data),
            ));
        } else {
            let screen_info = MessageBuilder::build_screen_info(&current_app);
            // Include injected prompt if provided
            let text_content = if let Some(prompt) = user_prompt {
                format!(
                    "** 用户补充指令 **\n{}\n\n** Screen Info **\n\n{}",
                    prompt, screen_info
                )
            } else {
                format!("** Screen Info **\n\n{}", screen_info)
            };

            self.context.push(MessageBuilder::create_user_message(
                &text_content,
                Some(&screenshot.base64_data),
            ));
        }

        // Get model response
        let response = match self.model_client.request(&self.context).await {
            Ok(resp) => resp,
            Err(e) => {
                if self.agent_config.verbose {
                    eprintln!("Model error: {}", e);
                }
                return Ok(StepResult {
                    success: false,
                    finished: true,
                    action: None,
                    thinking: String::new(),
                    message: Some(format!("Model error: {}", e)),
                });
            }
        };

        // Parse action from response
        let (action, is_parse_error) = match parse_action(&response.action) {
            Ok(a) => (a, false),
            Err(e) => {
                if self.agent_config.verbose {
                    eprintln!("Failed to parse action: {}", response.action);
                    eprintln!("Parse error: {}", e);
                }
                // Return a retry action instead of finishing
                // This will prompt the model to continue/retry in the next step
                // Safe truncation for UTF-8
                let truncated_action = if response.action.chars().count() > 150 {
                    format!(
                        "{}...",
                        response.action.chars().take(150).collect::<String>()
                    )
                } else {
                    response.action.clone()
                };
                (
                    serde_json::json!({
                        "_metadata": "error",
                        "error": "parse_failed",
                        "message": format!("无法解析动作指令，请重新输出完整的 do(...) 或 finish(...) 指令。原始输出: {}", truncated_action)
                    }),
                    true,
                )
            }
        };

        if self.agent_config.verbose {
            let msgs = get_messages(&self.agent_config.lang);
            println!("\n{}", "=".repeat(50));
            println!("💭 {}:", msgs.thinking);
            println!("{}", "-".repeat(50));
            println!("{}", response.thinking);
            println!("{}", "-".repeat(50));
            if is_parse_error {
                println!("⚠️ 解析错误，将提示模型重试");
            } else {
                println!("🎯 {}:", msgs.action);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&action).unwrap_or_default()
            );
            println!("{}\n", "=".repeat(50));
        }

        // Remove image from context to save space
        if let Some(last_msg) = self.context.last_mut() {
            MessageBuilder::remove_images_from_message(last_msg);
        }

        // If parse error, add the incomplete response and a retry prompt
        if is_parse_error {
            // Add the incomplete assistant response to context
            self.context
                .push(MessageBuilder::create_assistant_message(&response.action));

            // Add a user message prompting the model to continue/retry
            self.context.push(MessageBuilder::create_user_message(
                "你的回复不完整或格式不正确，没有包含有效的 do(...) 或 finish(...) 指令。请继续输出或重新输出完整的动作指令。",
                None,
            ));

            return Ok(StepResult {
                success: false,
                finished: false,
                action: Some(action),
                thinking: response.thinking,
                message: Some("解析失败，等待模型重试".to_string()),
            });
        }

        // Execute action
        let result = self
            .action_handler
            .execute(&action, screenshot.width, screenshot.height);

        // Add assistant response to context
        self.context
            .push(MessageBuilder::create_assistant_message(&format!(
                "<think>{}</think><answer>{}</answer>",
                response.thinking, response.action
            )));

        // Check if finished
        let finished = action
            .get("_metadata")
            .and_then(|v| v.as_str())
            .map(|s| s == "finish")
            .unwrap_or(false)
            || result.should_finish;

        if finished && self.agent_config.verbose {
            let msgs = get_messages(&self.agent_config.lang);
            println!("\n🎉 {}", "=".repeat(48));
            println!(
                "✅ {}: {}",
                msgs.task_completed,
                result.message.as_deref().unwrap_or_else(|| {
                    action
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or(msgs.done)
                })
            );
            println!("{}\n", "=".repeat(50));
        }

        Ok(StepResult {
            success: result.success,
            finished,
            action: Some(action.clone()),
            thinking: response.thinking,
            message: result.message.or_else(|| {
                action
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }),
        })
    }

    /// Get the current conversation context.
    pub fn context(&self) -> &[Value] {
        &self.context
    }

    /// Get the current step count.
    pub fn step_count(&self) -> u32 {
        self.step_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_steps, 100);
        assert_eq!(config.lang, "cn");
        assert!(config.verbose);
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::default()
            .with_device_id("device123")
            .with_lang("en")
            .with_max_steps(50)
            .quiet();

        assert_eq!(config.device_id, Some("device123".to_string()));
        assert_eq!(config.lang, "en");
        assert_eq!(config.max_steps, 50);
        assert!(!config.verbose);
    }
}
