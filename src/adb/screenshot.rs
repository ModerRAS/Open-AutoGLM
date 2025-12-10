//! Screenshot utilities for capturing Android device screen.

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, RgbImage};
use std::io::Cursor;
use std::process::Command;
use std::{env, fs};
use thiserror::Error;
use uuid::Uuid;

use super::connection::get_adb_prefix;

/// Screenshot errors.
#[derive(Error, Debug)]
pub enum ScreenshotError {
    #[error("Failed to capture screenshot: {0}")]
    CaptureFailed(String),
    #[error("Failed to pull screenshot: {0}")]
    PullFailed(String),
    #[error("Failed to read image: {0}")]
    ImageReadFailed(String),
    #[error("Screenshot timeout")]
    Timeout,
}

/// Represents a captured screenshot.
#[derive(Debug, Clone)]
pub struct Screenshot {
    pub base64_data: String,
    pub width: u32,
    pub height: u32,
    pub is_sensitive: bool,
}

impl Screenshot {
    /// Create a new screenshot.
    pub fn new(base64_data: String, width: u32, height: u32, is_sensitive: bool) -> Self {
        Self {
            base64_data,
            width,
            height,
            is_sensitive,
        }
    }

    /// Create a fallback black screenshot.
    pub fn fallback(is_sensitive: bool) -> Self {
        create_fallback_screenshot(is_sensitive)
    }
}

/// Capture a screenshot from the connected Android device.
///
/// # Arguments
/// * `device_id` - Optional ADB device ID for multi-device setups.
///
/// # Returns
/// Screenshot object containing base64 data and dimensions.
///
/// # Note
/// If the screenshot fails (e.g., on sensitive screens like payment pages),
/// a black fallback image is returned with is_sensitive=True.
pub fn get_screenshot(device_id: Option<&str>) -> Screenshot {
    let temp_dir = env::temp_dir();
    let temp_filename = format!("screenshot_{}.png", Uuid::new_v4());
    let temp_path = temp_dir.join(&temp_filename);
    let prefix = get_adb_prefix(device_id);

    // Execute screenshot command
    let result = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "screencap", "-p", "/sdcard/tmp.png"])
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);

            // Check for screenshot failure (sensitive screen)
            if combined.contains("Status: -1") || combined.contains("Failed") {
                return create_fallback_screenshot(true);
            }
        }
        Err(e) => {
            tracing::error!("Screenshot command failed: {}", e);
            return create_fallback_screenshot(false);
        }
    }

    // Pull screenshot to local temp path
    let pull_result = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["pull", "/sdcard/tmp.png", temp_path.to_str().unwrap_or("")])
        .output();

    if pull_result.is_err() {
        return create_fallback_screenshot(false);
    }

    // Check if file exists and read it
    if !temp_path.exists() {
        return create_fallback_screenshot(false);
    }

    // Read and encode image
    match image::open(&temp_path) {
        Ok(img) => {
            let width = img.width();
            let height = img.height();

            // Encode to PNG and then to base64
            let mut buffer = Cursor::new(Vec::new());
            if img.write_to(&mut buffer, image::ImageFormat::Png).is_err() {
                let _ = fs::remove_file(&temp_path);
                return create_fallback_screenshot(false);
            }

            let base64_data = STANDARD.encode(buffer.into_inner());

            // Cleanup temp file
            let _ = fs::remove_file(&temp_path);

            Screenshot::new(base64_data, width, height, false)
        }
        Err(e) => {
            tracing::error!("Failed to read screenshot: {}", e);
            let _ = fs::remove_file(&temp_path);
            create_fallback_screenshot(false)
        }
    }
}

/// Create a black fallback image when screenshot fails.
fn create_fallback_screenshot(is_sensitive: bool) -> Screenshot {
    let default_width: u32 = 1080;
    let default_height: u32 = 2400;

    // Create black image
    let black_img = RgbImage::from_fn(default_width, default_height, |_, _| {
        image::Rgb([0u8, 0u8, 0u8])
    });
    let dynamic_img = DynamicImage::ImageRgb8(black_img);

    // Encode to PNG and then to base64
    let mut buffer = Cursor::new(Vec::new());
    let _ = dynamic_img.write_to(&mut buffer, image::ImageFormat::Png);
    let base64_data = STANDARD.encode(buffer.into_inner());

    Screenshot::new(base64_data, default_width, default_height, is_sensitive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_screenshot() {
        let screenshot = create_fallback_screenshot(true);
        assert_eq!(screenshot.width, 1080);
        assert_eq!(screenshot.height, 2400);
        assert!(screenshot.is_sensitive);
        assert!(!screenshot.base64_data.is_empty());
    }
}
