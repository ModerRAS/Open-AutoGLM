//! Action handler for processing AI model outputs.

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::thread;
use std::time::Duration;
use thiserror::Error;

use crate::adb::{
    back, clear_text, detect_and_set_adb_keyboard, double_tap, home, launch_app, long_press,
    restore_keyboard, swipe, tap, type_text,
};

/// Action handler errors.
#[derive(Error, Debug)]
pub enum ActionError {
    #[error("Unknown action type: {0}")]
    UnknownActionType(String),
    #[error("Unknown action: {0}")]
    UnknownAction(String),
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    #[error("Action failed: {0}")]
    ExecutionFailed(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result of an action execution.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub success: bool,
    pub should_finish: bool,
    pub message: Option<String>,
    pub requires_confirmation: bool,
}

impl ActionResult {
    /// Create a successful result.
    pub fn success() -> Self {
        Self {
            success: true,
            should_finish: false,
            message: None,
            requires_confirmation: false,
        }
    }

    /// Create a failure result.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            should_finish: false,
            message: Some(message.into()),
            requires_confirmation: false,
        }
    }

    /// Create a finish result.
    pub fn finish(message: Option<String>) -> Self {
        Self {
            success: true,
            should_finish: true,
            message,
            requires_confirmation: false,
        }
    }
}

/// Callback type for confirmation requests.
pub type ConfirmationCallback = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// Callback type for takeover requests.
pub type TakeoverCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Default coordinate scale factor for LLM output mapping.
/// LLM coordinates will be multiplied by this factor before mapping to actual screen coordinates.
pub const DEFAULT_COORDINATE_SCALE: f64 = 1.61;

/// Handles execution of actions from AI model output.
pub struct ActionHandler {
    device_id: Option<String>,
    confirmation_callback: ConfirmationCallback,
    takeover_callback: TakeoverCallback,
    /// Scale factor for X coordinates (LLM output * scale = actual coordinate)
    scale_x: f64,
    /// Scale factor for Y coordinates (LLM output * scale = actual coordinate)
    scale_y: f64,
}

impl ActionHandler {
    /// Create a new ActionHandler with default scale factors.
    ///
    /// # Arguments
    /// * `device_id` - Optional ADB device ID for multi-device setups.
    /// * `confirmation_callback` - Optional callback for sensitive action confirmation.
    /// * `takeover_callback` - Optional callback for takeover requests (login, captcha).
    pub fn new(
        device_id: Option<String>,
        confirmation_callback: Option<ConfirmationCallback>,
        takeover_callback: Option<TakeoverCallback>,
    ) -> Self {
        Self::with_scale(
            device_id,
            confirmation_callback,
            takeover_callback,
            DEFAULT_COORDINATE_SCALE,
            DEFAULT_COORDINATE_SCALE,
        )
    }

    /// Create a new ActionHandler with custom scale factors.
    ///
    /// # Arguments
    /// * `device_id` - Optional ADB device ID for multi-device setups.
    /// * `confirmation_callback` - Optional callback for sensitive action confirmation.
    /// * `takeover_callback` - Optional callback for takeover requests (login, captcha).
    /// * `scale_x` - Scale factor for X coordinates.
    /// * `scale_y` - Scale factor for Y coordinates.
    pub fn with_scale(
        device_id: Option<String>,
        confirmation_callback: Option<ConfirmationCallback>,
        takeover_callback: Option<TakeoverCallback>,
        scale_x: f64,
        scale_y: f64,
    ) -> Self {
        Self {
            device_id,
            confirmation_callback: confirmation_callback
                .unwrap_or_else(|| Box::new(default_confirmation)),
            takeover_callback: takeover_callback.unwrap_or_else(|| Box::new(default_takeover)),
            scale_x,
            scale_y,
        }
    }

    /// Set the coordinate scale factors.
    pub fn set_scale(&mut self, scale_x: f64, scale_y: f64) {
        self.scale_x = scale_x;
        self.scale_y = scale_y;
    }

    /// Get the current coordinate scale factors.
    pub fn get_scale(&self) -> (f64, f64) {
        (self.scale_x, self.scale_y)
    }

    /// Execute an action from the AI model.
    ///
    /// # Arguments
    /// * `action` - The action dictionary from the model.
    /// * `screen_width` - Current screen width in pixels.
    /// * `screen_height` - Current screen height in pixels.
    ///
    /// # Returns
    /// ActionResult indicating success and whether to finish.
    pub fn execute(&self, action: &Value, screen_width: u32, screen_height: u32) -> ActionResult {
        let action_type = action
            .get("_metadata")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match action_type {
            "finish" => {
                let message = action
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                ActionResult::finish(message)
            }
            "do" => {
                let action_name = action.get("action").and_then(|v| v.as_str()).unwrap_or("");

                self.handle_action(action_name, action, screen_width, screen_height)
            }
            _ => ActionResult::failure(format!("Unknown action type: {}", action_type)),
        }
    }

    fn handle_action(
        &self,
        action_name: &str,
        action: &Value,
        screen_width: u32,
        screen_height: u32,
    ) -> ActionResult {
        match action_name {
            "Launch" => self.handle_launch(action),
            "Tap" => self.handle_tap(action, screen_width, screen_height),
            "Type" | "Type_Name" => self.handle_type(action),
            "Swipe" => self.handle_swipe(action, screen_width, screen_height),
            "Back" => self.handle_back(),
            "Home" => self.handle_home(),
            "Double Tap" => self.handle_double_tap(action, screen_width, screen_height),
            "Long Press" => self.handle_long_press(action, screen_width, screen_height),
            "Wait" => self.handle_wait(action),
            "Take_over" => self.handle_takeover(action),
            "Note" => ActionResult::success(),
            "Call_API" => ActionResult::success(),
            "Interact" => ActionResult {
                success: true,
                should_finish: false,
                message: Some("User interaction required".to_string()),
                requires_confirmation: false,
            },
            _ => ActionResult::failure(format!("Unknown action: {}", action_name)),
        }
    }

    /// Apply scale factor and validate coordinates are within screen bounds.
    ///
    /// LLM output coordinates are multiplied by scale factors before validation.
    fn validate_absolute_coordinates(
        &self,
        element: &[i64],
        screen_width: u32,
        screen_height: u32,
    ) -> Result<(i32, i32), String> {
        let raw_x = element[0];
        let raw_y = element[1];

        // Apply scale factors to LLM output coordinates
        let scaled_x = (raw_x as f64 * self.scale_x).round() as i64;
        let scaled_y = (raw_y as f64 * self.scale_y).round() as i64;

        // Check if scaled coordinates are within valid screen range
        if scaled_x < 0 || scaled_x >= screen_width as i64 {
            return Err(format!(
                "Scaled X coordinate {} (raw: {} × {:.2}) is out of bounds. Valid range is [0, {}). \
                Please provide coordinates within the screen area.",
                scaled_x, raw_x, self.scale_x, screen_width
            ));
        }
        if scaled_y < 0 || scaled_y >= screen_height as i64 {
            return Err(format!(
                "Scaled Y coordinate {} (raw: {} × {:.2}) is out of bounds. Valid range is [0, {}). \
                Please provide coordinates within the screen area.",
                scaled_y, raw_y, self.scale_y, screen_height
            ));
        }

        Ok((scaled_x as i32, scaled_y as i32))
    }

    /// Validate absolute pixel coordinates with detailed error messages.
    fn validate_coordinates(
        &self,
        coords: &[i64],
        coord_name: &str,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<(i32, i32), ActionResult> {
        match self.validate_absolute_coordinates(coords, screen_width, screen_height) {
            Ok((x, y)) => Ok((x, y)),
            Err(msg) => Err(ActionResult::failure(format!(
                "Coordinate error for {}: {}",
                coord_name, msg
            ))),
        }
    }

    fn handle_launch(&self, action: &Value) -> ActionResult {
        let app_name = match action.get("app").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => return ActionResult::failure("No app name specified"),
        };

        if launch_app(app_name, self.device_id.as_deref(), None) {
            ActionResult::success()
        } else {
            ActionResult::failure(format!("App not found: {}", app_name))
        }
    }

    fn handle_tap(&self, action: &Value, screen_width: u32, screen_height: u32) -> ActionResult {
        let element = match action.get("element").and_then(|v| v.as_array()) {
            Some(arr) => {
                let coords: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                if coords.len() < 2 {
                    return ActionResult::failure("Invalid element coordinates");
                }
                coords
            }
            None => return ActionResult::failure("No element coordinates"),
        };

        // Check for sensitive operation
        if let Some(message) = action.get("message").and_then(|v| v.as_str()) {
            if !(self.confirmation_callback)(message) {
                return ActionResult {
                    success: false,
                    should_finish: true,
                    message: Some("User cancelled sensitive operation".to_string()),
                    requires_confirmation: true,
                };
            }
        }

        let (x, y) =
            match self.validate_coordinates(&element, "element", screen_width, screen_height) {
                Ok(coords) => coords,
                Err(result) => return result,
            };
        tap(x, y, self.device_id.as_deref(), None);
        ActionResult::success()
    }

    fn handle_type(&self, action: &Value) -> ActionResult {
        let text = action.get("text").and_then(|v| v.as_str()).unwrap_or("");

        // Switch to ADB keyboard
        let original_ime = detect_and_set_adb_keyboard(self.device_id.as_deref());
        thread::sleep(Duration::from_secs(1));

        // Clear existing text and type new text
        clear_text(self.device_id.as_deref());
        thread::sleep(Duration::from_secs(1));

        type_text(text, self.device_id.as_deref());
        thread::sleep(Duration::from_secs(1));

        // Restore original keyboard
        restore_keyboard(&original_ime, self.device_id.as_deref());
        thread::sleep(Duration::from_secs(1));

        ActionResult::success()
    }

    fn handle_swipe(&self, action: &Value, screen_width: u32, screen_height: u32) -> ActionResult {
        let start = match action.get("start").and_then(|v| v.as_array()) {
            Some(arr) => {
                let coords: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                if coords.len() < 2 {
                    return ActionResult::failure("Invalid start coordinates");
                }
                coords
            }
            None => return ActionResult::failure("Missing start coordinates"),
        };

        let end = match action.get("end").and_then(|v| v.as_array()) {
            Some(arr) => {
                let coords: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                if coords.len() < 2 {
                    return ActionResult::failure("Invalid end coordinates");
                }
                coords
            }
            None => return ActionResult::failure("Missing end coordinates"),
        };

        let (start_x, start_y) =
            match self.validate_coordinates(&start, "start", screen_width, screen_height) {
                Ok(coords) => coords,
                Err(result) => return result,
            };
        let (end_x, end_y) =
            match self.validate_coordinates(&end, "end", screen_width, screen_height) {
                Ok(coords) => coords,
                Err(result) => return result,
            };

        swipe(
            start_x,
            start_y,
            end_x,
            end_y,
            None,
            self.device_id.as_deref(),
            None,
        );
        ActionResult::success()
    }

    fn handle_back(&self) -> ActionResult {
        back(self.device_id.as_deref(), None);
        ActionResult::success()
    }

    fn handle_home(&self) -> ActionResult {
        home(self.device_id.as_deref(), None);
        ActionResult::success()
    }

    fn handle_double_tap(
        &self,
        action: &Value,
        screen_width: u32,
        screen_height: u32,
    ) -> ActionResult {
        let element = match action.get("element").and_then(|v| v.as_array()) {
            Some(arr) => {
                let coords: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                if coords.len() < 2 {
                    return ActionResult::failure("Invalid element coordinates");
                }
                coords
            }
            None => return ActionResult::failure("No element coordinates"),
        };

        let (x, y) =
            match self.validate_coordinates(&element, "element", screen_width, screen_height) {
                Ok(coords) => coords,
                Err(result) => return result,
            };
        double_tap(x, y, self.device_id.as_deref(), None);
        ActionResult::success()
    }

    fn handle_long_press(
        &self,
        action: &Value,
        screen_width: u32,
        screen_height: u32,
    ) -> ActionResult {
        let element = match action.get("element").and_then(|v| v.as_array()) {
            Some(arr) => {
                let coords: Vec<i64> = arr.iter().filter_map(|v| v.as_i64()).collect();
                if coords.len() < 2 {
                    return ActionResult::failure("Invalid element coordinates");
                }
                coords
            }
            None => return ActionResult::failure("No element coordinates"),
        };

        let (x, y) =
            match self.validate_coordinates(&element, "element", screen_width, screen_height) {
                Ok(coords) => coords,
                Err(result) => return result,
            };
        long_press(x, y, None, self.device_id.as_deref(), None);
        ActionResult::success()
    }

    fn handle_wait(&self, action: &Value) -> ActionResult {
        let duration_str = action
            .get("duration")
            .and_then(|v| v.as_str())
            .unwrap_or("1 seconds");

        let duration: f64 = duration_str
            .replace("seconds", "")
            .trim()
            .parse()
            .unwrap_or(1.0);

        thread::sleep(Duration::from_secs_f64(duration));
        ActionResult::success()
    }

    fn handle_takeover(&self, action: &Value) -> ActionResult {
        let message = action
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("User intervention required");

        (self.takeover_callback)(message);
        ActionResult::success()
    }
}

/// Default confirmation callback using console input.
fn default_confirmation(message: &str) -> bool {
    print!("Sensitive operation: {}\nConfirm? (Y/N): ", message);
    io::stdout().flush().unwrap();

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap();

    line.trim().eq_ignore_ascii_case("y")
}

/// Default takeover callback using console input.
fn default_takeover(message: &str) {
    print!(
        "{}\nPress Enter after completing manual operation...",
        message
    );
    io::stdout().flush().unwrap();

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).unwrap();
}

/// Parse action from model response.
///
/// # Arguments
/// * `response` - Raw response string from the model.
///
/// # Returns
/// Parsed action as JSON Value.
pub fn parse_action(response: &str) -> Result<Value, ActionError> {
    let response = response.trim();

    // Try to parse as a do() action
    if response.starts_with("do(") {
        return parse_do_action(response);
    }

    // Try to parse as a finish() action
    if response.starts_with("finish(") {
        return parse_finish_action(response);
    }

    Err(ActionError::ParseError(format!(
        "Failed to parse action: {}",
        response
    )))
}

/// Parse a do() action string into a JSON Value.
fn parse_do_action(response: &str) -> Result<Value, ActionError> {
    // Extract the content between do( and )
    let content = response
        .strip_prefix("do(")
        .and_then(|s| s.strip_suffix(")"))
        .ok_or_else(|| ActionError::ParseError("Invalid do() format".to_string()))?;

    let mut result = json!({
        "_metadata": "do"
    });

    // Parse key-value pairs using a state machine approach
    let mut key = String::new();
    let mut value = String::new();
    let mut in_string = false;
    let mut in_list = false;
    let mut list_depth = 0;
    let mut string_char = '"';
    let mut parsing_key = true;

    for c in content.chars() {
        if in_string {
            if c == string_char {
                in_string = false;
            } else {
                value.push(c);
            }
            continue;
        }

        if in_list {
            value.push(c);
            if c == '[' {
                list_depth += 1;
            } else if c == ']' {
                list_depth -= 1;
                if list_depth == 0 {
                    in_list = false;
                }
            }
            continue;
        }

        match c {
            '"' | '\'' => {
                in_string = true;
                string_char = c;
            }
            '[' => {
                in_list = true;
                list_depth = 1;
                value.push(c);
            }
            '=' => {
                parsing_key = false;
            }
            ',' => {
                // Save current key-value pair
                let trimmed_key = key.trim().to_string();
                if !trimmed_key.is_empty() {
                    let parsed_value = parse_value(value.trim())?;
                    result[&trimmed_key] = parsed_value;
                }
                key.clear();
                value.clear();
                parsing_key = true;
            }
            _ if c.is_whitespace() => {
                // Skip whitespace outside strings
            }
            _ => {
                if parsing_key {
                    key.push(c);
                } else {
                    value.push(c);
                }
            }
        }
    }

    // Save the last key-value pair
    let trimmed_key = key.trim().to_string();
    if !trimmed_key.is_empty() {
        let parsed_value = parse_value(value.trim())?;
        result[&trimmed_key] = parsed_value;
    }

    Ok(result)
}

/// Parse a finish() action string into a JSON Value.
fn parse_finish_action(response: &str) -> Result<Value, ActionError> {
    let content = response
        .strip_prefix("finish(")
        .and_then(|s| s.strip_suffix(")"))
        .ok_or_else(|| ActionError::ParseError("Invalid finish() format".to_string()))?;

    let mut result = json!({
        "_metadata": "finish"
    });

    // Try to extract message
    if content.contains("message=") {
        let msg_start = content.find("message=").unwrap() + 8;
        let remaining = &content[msg_start..];

        // Find the message value (handle both quoted and unquoted)
        let message = if remaining.starts_with('"') || remaining.starts_with('\'') {
            let quote_char = remaining.chars().next().unwrap();
            let end = remaining[1..]
                .find(quote_char)
                .map(|i| i + 1)
                .unwrap_or(remaining.len());
            &remaining[1..end]
        } else {
            remaining.split(',').next().unwrap_or(remaining).trim()
        };

        result["message"] = json!(message);
    }

    Ok(result)
}

/// Parse a value string into a JSON Value.
fn parse_value(value_str: &str) -> Result<Value, ActionError> {
    let trimmed = value_str.trim();

    // Check if it's a list
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let inner = &trimmed[1..trimmed.len() - 1];
        let items: Vec<Value> = inner
            .split(',')
            .map(|s| {
                let s = s.trim();
                if let Ok(n) = s.parse::<i64>() {
                    json!(n)
                } else if let Ok(f) = s.parse::<f64>() {
                    json!(f)
                } else {
                    json!(s)
                }
            })
            .collect();
        return Ok(json!(items));
    }

    // Try to parse as number
    if let Ok(n) = trimmed.parse::<i64>() {
        return Ok(json!(n));
    }
    if let Ok(f) = trimmed.parse::<f64>() {
        return Ok(json!(f));
    }

    // Return as string
    Ok(json!(trimmed))
}

/// Helper function for creating 'do' actions.
pub fn do_action(action: &str, params: &[(&str, Value)]) -> Value {
    let mut result = json!({
        "_metadata": "do",
        "action": action
    });

    for (key, value) in params {
        result[*key] = value.clone();
    }

    result
}

/// Helper function for creating 'finish' actions.
pub fn finish_action(message: Option<&str>) -> Value {
    let mut result = json!({
        "_metadata": "finish"
    });

    if let Some(msg) = message {
        result["message"] = json!(msg);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_do_action() {
        let response = r#"do(action="Tap", element=[100, 200])"#;
        let result = parse_action(response).unwrap();
        assert_eq!(result["_metadata"], "do");
        assert_eq!(result["action"], "Tap");
    }

    #[test]
    fn test_parse_finish_action() {
        let response = r#"finish(message="Task completed")"#;
        let result = parse_action(response).unwrap();
        assert_eq!(result["_metadata"], "finish");
        assert_eq!(result["message"], "Task completed");
    }

    #[test]
    fn test_do_action_helper() {
        let action = do_action("Tap", &[("element", json!([100, 200]))]);
        assert_eq!(action["_metadata"], "do");
        assert_eq!(action["action"], "Tap");
    }

    #[test]
    fn test_finish_action_helper() {
        let action = finish_action(Some("Done"));
        assert_eq!(action["_metadata"], "finish");
        assert_eq!(action["message"], "Done");
    }

    #[test]
    fn test_action_result() {
        let success = ActionResult::success();
        assert!(success.success);
        assert!(!success.should_finish);

        let finish = ActionResult::finish(Some("Done".to_string()));
        assert!(finish.success);
        assert!(finish.should_finish);
    }

    #[test]
    fn test_coordinate_bounds_check_valid() {
        // Use scale factor of 1.0 for testing direct pixel mapping
        let handler = ActionHandler::with_scale(None, None, None, 1.0, 1.0);

        // Valid absolute coordinates within screen bounds (no scaling)
        let result = handler.validate_absolute_coordinates(&[500, 500], 1080, 1920);
        assert!(result.is_ok());
        let (x, y) = result.unwrap();
        assert_eq!(x, 500); // Direct pixel value
        assert_eq!(y, 500); // Direct pixel value

        // Edge cases - boundaries
        let result = handler.validate_absolute_coordinates(&[0, 0], 1080, 1920);
        assert!(result.is_ok());

        let result = handler.validate_absolute_coordinates(&[1079, 1919], 1080, 1920);
        assert!(result.is_ok());
    }

    #[test]
    fn test_coordinate_scaling() {
        // Test with default scale factor (1.61)
        let handler = ActionHandler::new(None, None, None);

        // 300 * 1.61 = 483, 400 * 1.61 = 644
        let result = handler.validate_absolute_coordinates(&[300, 400], 1080, 1920);
        assert!(result.is_ok());
        let (x, y) = result.unwrap();
        assert_eq!(x, 483); // 300 * 1.61 rounded
        assert_eq!(y, 644); // 400 * 1.61 rounded
    }

    #[test]
    fn test_custom_scale_factor() {
        // Test with custom scale factor
        let handler = ActionHandler::with_scale(None, None, None, 2.0, 1.5);

        // 100 * 2.0 = 200, 200 * 1.5 = 300
        let result = handler.validate_absolute_coordinates(&[100, 200], 1080, 1920);
        assert!(result.is_ok());
        let (x, y) = result.unwrap();
        assert_eq!(x, 200); // 100 * 2.0
        assert_eq!(y, 300); // 200 * 1.5
    }

    #[test]
    fn test_coordinate_bounds_check_invalid_x() {
        // Use scale factor of 1.0 for testing bounds
        let handler = ActionHandler::with_scale(None, None, None, 1.0, 1.0);

        // X coordinate out of bounds (negative)
        let result = handler.validate_absolute_coordinates(&[-10, 500], 1080, 1920);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("X coordinate"));

        // X coordinate out of bounds (>= screen width)
        let result = handler.validate_absolute_coordinates(&[1080, 500], 1080, 1920);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("X coordinate"));
    }

    #[test]
    fn test_coordinate_bounds_check_invalid_y() {
        // Use scale factor of 1.0 for testing bounds
        let handler = ActionHandler::with_scale(None, None, None, 1.0, 1.0);

        // Y coordinate out of bounds (negative)
        let result = handler.validate_absolute_coordinates(&[500, -10], 1080, 1920);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Y coordinate"));

        // Y coordinate out of bounds (>= screen height)
        let result = handler.validate_absolute_coordinates(&[500, 1920], 1080, 1920);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Y coordinate"));
    }

    #[test]
    fn test_tap_action_with_invalid_coordinates() {
        // Use scale factor of 1.0 for testing bounds
        let handler = ActionHandler::with_scale(None, None, None, 1.0, 1.0);

        // Tap with out-of-bounds coordinates (>= screen width)
        let action = json!({
            "_metadata": "do",
            "action": "Tap",
            "element": [1500, 500]
        });

        let result = handler.execute(&action, 1080, 1920);
        assert!(!result.success);
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("out of bounds"));
    }

    #[test]
    fn test_swipe_action_with_invalid_coordinates() {
        // Use scale factor of 1.0 for testing bounds
        let handler = ActionHandler::with_scale(None, None, None, 1.0, 1.0);

        // Swipe with out-of-bounds start coordinates (negative)
        let action = json!({
            "_metadata": "do",
            "action": "Swipe",
            "start": [-100, 500],
            "end": [500, 500]
        });

        let result = handler.execute(&action, 1080, 1920);
        assert!(!result.success);
        assert!(result.message.is_some());
        assert!(result.message.unwrap().contains("out of bounds"));
    }
}
