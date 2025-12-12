//! Model client for AI inference using OpenAI-compatible API.

use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

/// Default number of retry attempts for failed requests.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default delay between retry attempts in seconds.
pub const DEFAULT_RETRY_DELAY_SECS: u64 = 2;

/// Model client errors.
#[derive(Error, Debug)]
pub enum ModelError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Max retries exceeded after {0} attempts: {1}")]
    MaxRetriesExceeded(u32, String),
}

/// Configuration for the AI model.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub frequency_penalty: f32,
    pub extra_body: HashMap<String, Value>,
    /// Maximum number of retry attempts for failed requests.
    pub max_retries: u32,
    /// Delay between retry attempts in seconds.
    pub retry_delay_secs: u64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        let mut extra_body = HashMap::new();
        extra_body.insert("skip_special_tokens".to_string(), json!(false));

        Self {
            base_url: "http://localhost:8000/v1".to_string(),
            api_key: "EMPTY".to_string(),
            model_name: "autoglm-phone-9b".to_string(),
            max_tokens: 3000,
            temperature: 0.0,
            top_p: 0.85,
            frequency_penalty: 0.2,
            extra_body,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_delay_secs: DEFAULT_RETRY_DELAY_SECS,
        }
    }
}

impl ModelConfig {
    /// Create a new ModelConfig with custom base URL.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Create a new ModelConfig with custom API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = api_key.into();
        self
    }

    /// Create a new ModelConfig with custom model name.
    pub fn with_model_name(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = model_name.into();
        self
    }

    /// Set the maximum number of retry attempts for failed requests.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the delay between retry attempts in seconds.
    pub fn with_retry_delay(mut self, delay_secs: u64) -> Self {
        self.retry_delay_secs = delay_secs;
        self
    }
}

/// Response from the AI model.
#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub thinking: String,
    pub action: String,
    pub raw_content: String,
}

/// Client for interacting with OpenAI-compatible vision-language models.
pub struct ModelClient {
    config: ModelConfig,
    client: Client,
}

impl ModelClient {
    /// Create a new ModelClient with the given configuration.
    pub fn new(config: ModelConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    /// Create a new ModelClient with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ModelConfig::default())
    }

    /// Send a request to the model.
    ///
    /// # Arguments
    /// * `messages` - List of message dictionaries in OpenAI format.
    ///
    /// # Returns
    /// ModelResponse containing thinking and action.
    pub async fn request(&self, messages: &[Value]) -> Result<ModelResponse, ModelError> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let mut body = json!({
            "messages": messages,
            "model": self.config.model_name,
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "top_p": self.config.top_p,
            "frequency_penalty": self.config.frequency_penalty,
        });

        // Merge extra_body
        if let Value::Object(ref mut map) = body {
            for (key, value) in &self.config.extra_body {
                map.insert(key.clone(), value.clone());
            }
            // Explicitly disable streaming to get complete JSON response
            map.insert("stream".to_string(), json!(false));
        }

        let mut last_error: Option<ModelError> = None;
        let max_attempts = self.config.max_retries + 1; // +1 for the initial attempt

        for attempt in 1..=max_attempts {
            match self.send_request(&url, &body).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    let is_retryable = Self::is_retryable_error(&e);

                    if attempt < max_attempts && is_retryable {
                        eprintln!(
                            "⚠️  Request failed (attempt {}/{}): {}",
                            attempt, max_attempts, e
                        );
                        eprintln!("   Retrying in {} seconds...", self.config.retry_delay_secs);
                        sleep(Duration::from_secs(self.config.retry_delay_secs)).await;
                        last_error = Some(e);
                    } else if !is_retryable {
                        // Non-retryable error, return immediately
                        return Err(e);
                    } else {
                        last_error = Some(e);
                    }
                }
            }
        }

        // All retries exhausted
        Err(ModelError::MaxRetriesExceeded(
            self.config.max_retries,
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string()),
        ))
    }

    /// Check if an error is retryable (network errors, timeouts, etc.)
    fn is_retryable_error(error: &ModelError) -> bool {
        match error {
            ModelError::RequestFailed(_) => true, // Network errors are retryable
            ModelError::ApiError(msg) => {
                // Retry on server errors (5xx) or rate limits (429)
                msg.contains("500")
                    || msg.contains("502")
                    || msg.contains("503")
                    || msg.contains("504")
                    || msg.contains("429")
                    || msg.to_lowercase().contains("timeout")
                    || msg.to_lowercase().contains("rate limit")
            }
            ModelError::ParseError(_) => false, // Parse errors are not retryable
            ModelError::MaxRetriesExceeded(_, _) => false,
        }
    }

    /// Send a single request to the API.
    async fn send_request(&self, url: &str, body: &Value) -> Result<ModelResponse, ModelError> {
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ModelError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        // Get the raw response text first for debugging
        let response_text = response.text().await?;

        // Check if this is a streaming response (multiple JSON objects concatenated)
        // Streaming responses have format: {"...chunk1"}{"...chunk2"}...
        let json_value =
            if response_text.contains("}{") || response_text.contains("chat.completion.chunk") {
                // Handle streaming response - parse all chunks and combine content
                self.parse_streaming_response(&response_text)?
            } else {
                // Regular JSON response
                serde_json::from_str(&response_text).map_err(|e| {
                    ModelError::ParseError(format!(
                        "Failed to parse JSON: {}. Response: {}",
                        e,
                        Self::truncate_for_error(&response_text)
                    ))
                })?
            };

        // Extract the content - try multiple possible paths
        let raw_content = self.extract_content(&json_value).ok_or_else(|| {
            ModelError::ParseError(format!(
                "Could not find content in response. Structure: {}",
                Self::truncate_for_error(&response_text)
            ))
        })?;

        let (thinking, action) = Self::parse_response(&raw_content);

        Ok(ModelResponse {
            thinking,
            action,
            raw_content: raw_content.clone(),
        })
    }

    /// Truncate response text for error messages
    fn truncate_for_error(text: &str) -> String {
        if text.len() > 500 {
            format!("{}...(truncated)", &text[..500])
        } else {
            text.to_string()
        }
    }

    /// Parse streaming response (multiple JSON chunks concatenated)
    fn parse_streaming_response(&self, response_text: &str) -> Result<Value, ModelError> {
        let mut combined_content = String::new();

        // Simple approach: split by }{ and reconstruct valid JSON objects
        let chunks: Vec<String> = if response_text.contains("}{") {
            let fixed = response_text.replace("}{", "}\n{");
            fixed
                .lines()
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            // Maybe newline separated
            response_text
                .lines()
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string())
                .collect()
        };

        for chunk_str in &chunks {
            if chunk_str.is_empty() || chunk_str == "[DONE]" {
                continue;
            }

            // Remove "data: " prefix if present (SSE format)
            let json_str = chunk_str.strip_prefix("data: ").unwrap_or(chunk_str);
            if json_str.is_empty() || json_str == "[DONE]" {
                continue;
            }

            // Try to parse this chunk
            if let Ok(chunk_json) = serde_json::from_str::<Value>(json_str) {
                // Extract delta content from streaming chunk
                if let Some(content) = chunk_json
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("content"))
                    .and_then(|c| c.as_str())
                {
                    combined_content.push_str(content);
                }
                // Also check message.content for final chunks
                else if let Some(content) = chunk_json
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("message"))
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                {
                    combined_content.push_str(content);
                }
            }
        }

        if combined_content.is_empty() {
            return Err(ModelError::ParseError(format!(
                "No content found in streaming response. Chunks: {}",
                Self::truncate_for_error(response_text)
            )));
        }

        // Return as a simple JSON with the combined content
        Ok(json!({
            "choices": [{
                "message": {
                    "content": combined_content
                }
            }]
        }))
    }

    /// Extract content from various API response formats.
    /// Supports OpenAI, vLLM, and other compatible formats.
    fn extract_content(&self, json: &Value) -> Option<String> {
        // Standard OpenAI format: choices[0].message.content
        if let Some(content) = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
        {
            return Some(content.to_string());
        }

        // Alternative: choices[0].text (older completions API)
        if let Some(content) = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|c| c.as_str())
        {
            return Some(content.to_string());
        }

        // Some APIs return content directly in message
        if let Some(content) = json
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
        {
            return Some(content.to_string());
        }

        // Some APIs return content directly
        if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
            return Some(content.to_string());
        }

        // vLLM specific: output or outputs
        if let Some(content) = json.get("output").and_then(|c| c.as_str()) {
            return Some(content.to_string());
        }
        if let Some(content) = json
            .get("outputs")
            .and_then(|o| o.get(0))
            .and_then(|o| o.get("text"))
            .and_then(|c| c.as_str())
        {
            return Some(content.to_string());
        }

        // Anthropic format: content[0].text
        if let Some(content) = json
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
        {
            return Some(content.to_string());
        }

        // response.text (some custom APIs)
        if let Some(content) = json
            .get("response")
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
        {
            return Some(content.to_string());
        }

        // result.text or result.content
        if let Some(content) = json
            .get("result")
            .and_then(|r| r.get("text").or(r.get("content")))
            .and_then(|t| t.as_str())
        {
            return Some(content.to_string());
        }

        None
    }

    /// Parse the model response into thinking and action parts.
    ///
    /// Handles both standard format with tags and untagged format:
    /// - Standard: `<think>thinking</think><answer>action</answer>`
    /// - Untagged: `thinking text... do(action="...")`
    fn parse_response(content: &str) -> (String, String) {
        // Standard format with <answer> tag
        if content.contains("<answer>") {
            let parts: Vec<&str> = content.splitn(2, "<answer>").collect();
            let thinking = parts[0]
                .replace("<think>", "")
                .replace("</think>", "")
                .trim()
                .to_string();
            let action = parts
                .get(1)
                .map(|s| s.replace("</answer>", "").trim().to_string())
                .unwrap_or_default();

            return (thinking, action);
        }

        // Untagged format: try to extract do(...) or finish(...) from the content
        // and treat everything before it as thinking
        let content_trimmed = content.trim();

        // Find do() or finish() pattern
        if let Some(do_pos) = content_trimmed.find("do(") {
            let thinking = content_trimmed[..do_pos].trim().to_string();
            let action = content_trimmed[do_pos..].trim().to_string();
            return (thinking, action);
        }

        if let Some(finish_pos) = content_trimmed.find("finish(") {
            let thinking = content_trimmed[..finish_pos].trim().to_string();
            let action = content_trimmed[finish_pos..].trim().to_string();
            return (thinking, action);
        }

        // No recognizable pattern, return as-is (action only)
        (String::new(), content.to_string())
    }
}

/// Helper class for building conversation messages.
pub struct MessageBuilder;

impl MessageBuilder {
    /// Create a system message.
    pub fn create_system_message(content: &str) -> Value {
        json!({
            "role": "system",
            "content": content
        })
    }

    /// Create a user message with optional image.
    ///
    /// # Arguments
    /// * `text` - Text content.
    /// * `image_base64` - Optional base64-encoded image.
    ///
    /// # Returns
    /// Message as JSON Value.
    pub fn create_user_message(text: &str, image_base64: Option<&str>) -> Value {
        let mut content = Vec::new();

        if let Some(img_data) = image_base64 {
            content.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", img_data)
                }
            }));
        }

        content.push(json!({
            "type": "text",
            "text": text
        }));

        json!({
            "role": "user",
            "content": content
        })
    }

    /// Create an assistant message.
    pub fn create_assistant_message(content: &str) -> Value {
        json!({
            "role": "assistant",
            "content": content
        })
    }

    /// Remove image content from a message to save context space.
    pub fn remove_images_from_message(message: &mut Value) {
        if let Some(Value::Array(arr)) = message.get_mut("content") {
            arr.retain(|item| {
                item.get("type")
                    .and_then(|t| t.as_str())
                    .map(|t| t == "text")
                    .unwrap_or(false)
            });
        }
    }

    /// Build screen info string for the model.
    ///
    /// # Arguments
    /// * `current_app` - Current app name.
    ///
    /// # Returns
    /// JSON string with screen info.
    pub fn build_screen_info(current_app: &str) -> String {
        json!({
            "current_app": current_app
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert_eq!(config.base_url, "http://localhost:8000/v1");
        assert_eq!(config.model_name, "autoglm-phone-9b");
    }

    #[test]
    fn test_parse_response() {
        let content = "<think>I need to tap the button</think><answer>do(action=\"Tap\", element=[100, 200])</answer>";
        let (thinking, action) = ModelClient::parse_response(content);
        assert_eq!(thinking, "I need to tap the button");
        assert_eq!(action, "do(action=\"Tap\", element=[100, 200])");
    }

    #[test]
    fn test_parse_response_no_answer() {
        let content = "some raw content";
        let (thinking, action) = ModelClient::parse_response(content);
        assert_eq!(thinking, "");
        assert_eq!(action, "some raw content");
    }

    #[test]
    fn test_parse_response_untagged_do() {
        let content = r#"用户想要我帮他刷小红书，具体要求是：
1. 刷十几个帖子
2. 看帖子的内容和评论

首先启动小红书。
do(action="Launch", app="小红书")"#;
        let (thinking, action) = ModelClient::parse_response(content);
        assert!(thinking.contains("刷小红书"));
        assert!(thinking.contains("首先启动小红书"));
        assert_eq!(action, r#"do(action="Launch", app="小红书")"#);
    }

    #[test]
    fn test_parse_response_untagged_finish() {
        let content = r#"任务已完成，总结如下：
1. 浏览了15个帖子
2. 内容主要是美食和旅游

finish(message="已完成浏览")"#;
        let (thinking, action) = ModelClient::parse_response(content);
        assert!(thinking.contains("任务已完成"));
        assert_eq!(action, r#"finish(message="已完成浏览")"#);
    }

    #[test]
    fn test_message_builder() {
        let system_msg = MessageBuilder::create_system_message("You are an assistant");
        assert_eq!(system_msg["role"], "system");

        let user_msg = MessageBuilder::create_user_message("Hello", None);
        assert_eq!(user_msg["role"], "user");

        let user_msg_with_image =
            MessageBuilder::create_user_message("Look at this", Some("base64data"));
        assert_eq!(user_msg_with_image["content"][0]["type"], "image_url");
    }
}
