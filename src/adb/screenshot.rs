//! Screenshot utilities for capturing Android device screen.

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, RgbImage};
use std::io::Cursor;
use std::process::Command;
use thiserror::Error;

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
/// This function uses `adb exec-out screencap -p` to capture the screenshot
/// directly to stdout, avoiding disk I/O on both the device and host.
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
    let prefix = get_adb_prefix(device_id);

    // Use exec-out to get screenshot directly to stdout (no disk I/O)
    // This is much faster than shell screencap + pull
    let result = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["exec-out", "screencap", "-p"])
        .output();

    match result {
        Ok(output) => {
            // Check for errors in stderr
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Status: -1") || stderr.contains("Failed") || stderr.contains("error") {
                tracing::warn!("Screenshot may have failed (sensitive screen): {}", stderr);
                return create_fallback_screenshot(true);
            }

            // Check if we got valid PNG data
            let png_data = &output.stdout;
            if png_data.len() < 8 {
                tracing::error!("Screenshot data too small: {} bytes", png_data.len());
                return create_fallback_screenshot(false);
            }

            // Verify PNG magic bytes
            if &png_data[0..8] != b"\x89PNG\r\n\x1a\n" {
                tracing::error!("Invalid PNG header, got: {:?}", &png_data[0..8.min(png_data.len())]);
                return create_fallback_screenshot(false);
            }

            // Parse the image to get dimensions
            match image::load_from_memory(png_data) {
                Ok(img) => {
                    let width = img.width();
                    let height = img.height();
                    let base64_data = STANDARD.encode(png_data);

                    Screenshot::new(base64_data, width, height, false)
                }
                Err(e) => {
                    tracing::error!("Failed to parse screenshot image: {}", e);
                    create_fallback_screenshot(false)
                }
            }
        }
        Err(e) => {
            tracing::error!("Screenshot command failed: {}", e);
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
