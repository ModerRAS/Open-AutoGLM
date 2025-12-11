//! Coordinate calibration for automatic scale factor detection.
//!
//! This module provides functionality to automatically calibrate coordinate
//! scale factors by generating test images with known marker positions and
//! asking the LLM to identify those positions.

use base64::{engine::general_purpose::STANDARD, Engine};
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut};
use imageproc::rect::Rect;
use std::io::Cursor;

use crate::adb::get_screenshot;
use crate::model::{MessageBuilder, ModelClient};

/// Default calibration points as (x_ratio, y_ratio) where 0.0-1.0 represents screen percentage
pub const DEFAULT_CALIBRATION_POINTS: [(f64, f64); 5] = [
    (0.5, 0.5),   // Center
    (0.25, 0.25), // Top-left quadrant
    (0.75, 0.25), // Top-right quadrant
    (0.25, 0.75), // Bottom-left quadrant
    (0.75, 0.75), // Bottom-right quadrant
];

/// Configuration for calibration process.
#[derive(Debug, Clone)]
pub struct CalibrationConfig {
    /// Calibration points as (x_ratio, y_ratio) pairs
    pub calibration_points: Vec<(f64, f64)>,
    /// Language for prompts ("cn" or "en")
    pub lang: String,
    /// Marker size in pixels (will be scaled based on screen size)
    pub marker_size_ratio: f64,
    /// ADB device ID (optional)
    pub device_id: Option<String>,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            calibration_points: DEFAULT_CALIBRATION_POINTS.to_vec(),
            lang: "cn".to_string(),
            marker_size_ratio: 0.05, // 5% of screen width
            device_id: None,
        }
    }
}

impl CalibrationConfig {
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }
}

/// Result of a calibration process.
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    /// Calculated X scale factor
    pub scale_x: f64,
    /// Calculated Y scale factor
    pub scale_y: f64,
    /// Detected screen width
    pub screen_width: u32,
    /// Detected screen height
    pub screen_height: u32,
    /// Individual point results for debugging
    pub point_results: Vec<PointCalibrationResult>,
    /// Whether calibration was successful
    pub success: bool,
    /// Error message if calibration failed
    pub error: Option<String>,
}

/// Result for a single calibration point.
#[derive(Debug, Clone)]
pub struct PointCalibrationResult {
    /// Expected X coordinate (actual pixel)
    pub expected_x: i32,
    /// Expected Y coordinate (actual pixel)
    pub expected_y: i32,
    /// LLM reported X coordinate
    pub reported_x: i32,
    /// LLM reported Y coordinate
    pub reported_y: i32,
    /// X ratio (expected / reported)
    pub ratio_x: f64,
    /// Y ratio (expected / reported)
    pub ratio_y: f64,
}

/// Coordinator calibrator for automatic scale factor detection.
pub struct CoordinateCalibrator {
    config: CalibrationConfig,
}

impl CoordinateCalibrator {
    /// Create a new calibrator with the given configuration.
    pub fn new(config: CalibrationConfig) -> Self {
        Self { config }
    }

    /// Get screen dimensions by taking a screenshot from the device.
    fn get_screen_dimensions(&self) -> Result<(u32, u32), String> {
        println!("📱 Taking screenshot to detect screen dimensions...");
        
        let screenshot = get_screenshot(self.config.device_id.as_deref());
        
        if screenshot.is_sensitive {
            return Err("Could not capture screenshot (sensitive screen)".to_string());
        }
        
        let width = screenshot.width;
        let height = screenshot.height;
        
        if width == 0 || height == 0 {
            return Err("Invalid screen dimensions".to_string());
        }
        
        println!("   Detected screen size: {}x{}", width, height);
        
        Ok((width, height))
    }

    /// Run the calibration process.
    ///
    /// This first takes a screenshot to detect screen dimensions, then
    /// generates test images, sends them to the LLM, and calculates
    /// the appropriate scale factors based on the LLM's responses.
    pub async fn calibrate(&self, model_client: &ModelClient) -> CalibrationResult {
        // Step 1: Get screen dimensions from actual device screenshot
        let (screen_width, screen_height) = match self.get_screen_dimensions() {
            Ok(dims) => dims,
            Err(e) => {
                return CalibrationResult {
                    scale_x: 1.0,
                    scale_y: 1.0,
                    screen_width: 0,
                    screen_height: 0,
                    point_results: Vec::new(),
                    success: false,
                    error: Some(format!("Failed to get screen dimensions: {}", e)),
                };
            }
        };

        // Calculate marker size based on screen width
        let marker_size = (screen_width as f64 * self.config.marker_size_ratio) as u32;
        
        let mut point_results = Vec::new();
        let mut total_ratio_x = 0.0;
        let mut total_ratio_y = 0.0;
        let mut valid_points = 0;

        for (i, &(x_ratio, y_ratio)) in self.config.calibration_points.iter().enumerate() {
            let expected_x = (x_ratio * screen_width as f64) as i32;
            let expected_y = (y_ratio * screen_height as f64) as i32;

            println!(
                "📍 Calibrating point {}/{}: expected ({}, {})",
                i + 1,
                self.config.calibration_points.len(),
                expected_x,
                expected_y
            );

            // Generate calibration image
            let image_base64 = self.generate_calibration_image(
                expected_x, expected_y, i + 1,
                screen_width, screen_height, marker_size
            );

            // Ask LLM to identify the marker position
            match self.ask_llm_for_position(
                model_client, &image_base64, i + 1,
                screen_width, screen_height
            ).await {
                Ok((reported_x, reported_y)) => {
                    // Calculate ratios (expected / reported = scale factor needed)
                    let ratio_x = if reported_x != 0 {
                        expected_x as f64 / reported_x as f64
                    } else {
                        1.0
                    };
                    let ratio_y = if reported_y != 0 {
                        expected_y as f64 / reported_y as f64
                    } else {
                        1.0
                    };

                    println!(
                        "   LLM reported: ({}, {}), ratios: X={:.3}, Y={:.3}",
                        reported_x, reported_y, ratio_x, ratio_y
                    );

                    // Filter out obviously wrong results (ratio should be reasonable)
                    if ratio_x > 0.5 && ratio_x < 2.0 && ratio_y > 0.5 && ratio_y < 2.0 {
                        total_ratio_x += ratio_x;
                        total_ratio_y += ratio_y;
                        valid_points += 1;
                    } else {
                        println!("   ⚠️ Ratio out of reasonable range, skipping this point");
                    }

                    point_results.push(PointCalibrationResult {
                        expected_x,
                        expected_y,
                        reported_x,
                        reported_y,
                        ratio_x,
                        ratio_y,
                    });
                }
                Err(e) => {
                    println!("   ❌ Failed to get LLM response: {}", e);
                    point_results.push(PointCalibrationResult {
                        expected_x,
                        expected_y,
                        reported_x: 0,
                        reported_y: 0,
                        ratio_x: 1.0,
                        ratio_y: 1.0,
                    });
                }
            }
        }

        if valid_points == 0 {
            return CalibrationResult {
                scale_x: 1.0,
                scale_y: 1.0,
                screen_width,
                screen_height,
                point_results,
                success: false,
                error: Some("No valid calibration points".to_string()),
            };
        }

        // Calculate average scale factors
        let scale_x = total_ratio_x / valid_points as f64;
        let scale_y = total_ratio_y / valid_points as f64;

        println!("\n✅ Calibration complete!");
        println!("   Screen size: {}x{}", screen_width, screen_height);
        println!("   Valid points: {}/{}", valid_points, self.config.calibration_points.len());
        println!("   Calculated scale factors: X={:.4}, Y={:.4}", scale_x, scale_y);

        CalibrationResult {
            scale_x,
            scale_y,
            screen_width,
            screen_height,
            point_results,
            success: true,
            error: None,
        }
    }

    /// Generate a calibration image with a marker at the specified position.
    fn generate_calibration_image(
        &self,
        x: i32,
        y: i32,
        point_num: usize,
        width: u32,
        height: u32,
        marker_size: u32,
    ) -> String {
        // Create a dark gray background image
        let mut img = RgbImage::from_fn(width, height, |_, _| Rgb([30u8, 30u8, 30u8]));

        // Draw grid lines for reference (every 25%)
        let grid_color = Rgb([60u8, 60u8, 60u8]);
        for i in 1..4 {
            let x_line = (width * i / 4) as i32;
            let y_line = (height * i / 4) as i32;
            // Vertical line
            draw_filled_rect_mut(
                &mut img,
                Rect::at(x_line, 0).of_size(2, height),
                grid_color,
            );
            // Horizontal line
            draw_filled_rect_mut(
                &mut img,
                Rect::at(0, y_line).of_size(width, 2),
                grid_color,
            );
        }

        // Draw axis labels using simple shapes
        // Draw coordinate indicators at edges
        let label_color = Rgb([100u8, 100u8, 100u8]);
        
        // X-axis markers
        for i in 0..=4 {
            let x_pos = (width * i / 4) as i32;
            draw_filled_rect_mut(
                &mut img,
                Rect::at(x_pos - 1, 0).of_size(3, 20),
                label_color,
            );
            draw_filled_rect_mut(
                &mut img,
                Rect::at(x_pos - 1, height as i32 - 20).of_size(3, 20),
                label_color,
            );
        }

        // Y-axis markers
        for i in 0..=4 {
            let y_pos = (height * i / 4) as i32;
            draw_filled_rect_mut(
                &mut img,
                Rect::at(0, y_pos - 1).of_size(20, 3),
                label_color,
            );
            draw_filled_rect_mut(
                &mut img,
                Rect::at(width as i32 - 20, y_pos - 1).of_size(20, 3),
                label_color,
            );
        }

        // Draw the main marker (a filled rectangle with border and crosshair)
        let marker_x = (x - marker_size as i32 / 2).max(0);
        let marker_y = (y - marker_size as i32 / 2).max(0);

        // Red filled marker
        draw_filled_rect_mut(
            &mut img,
            Rect::at(marker_x, marker_y).of_size(marker_size, marker_size),
            Rgb([255u8, 50u8, 50u8]),
        );

        // White border (thick)
        for offset in 0..3 {
            draw_hollow_rect_mut(
                &mut img,
                Rect::at(marker_x - offset as i32, marker_y - offset as i32)
                    .of_size(marker_size + offset * 2, marker_size + offset * 2),
                Rgb([255u8, 255u8, 255u8]),
            );
        }

        // Yellow crosshair at exact center
        let cross_color = Rgb([255u8, 255u8, 0u8]);
        let cross_thickness = 4;
        // Vertical line of crosshair
        draw_filled_rect_mut(
            &mut img,
            Rect::at(x - cross_thickness as i32 / 2, marker_y).of_size(cross_thickness, marker_size),
            cross_color,
        );
        // Horizontal line of crosshair
        draw_filled_rect_mut(
            &mut img,
            Rect::at(marker_x, y - cross_thickness as i32 / 2).of_size(marker_size, cross_thickness),
            cross_color,
        );

        // Draw center dot (black for contrast)
        let dot_size = 8;
        draw_filled_rect_mut(
            &mut img,
            Rect::at(x - dot_size as i32 / 2, y - dot_size as i32 / 2).of_size(dot_size, dot_size),
            Rgb([0u8, 0u8, 0u8]),
        );

        // Draw point number indicator using dots pattern
        // Simple visual indicator for point number (1-5 dots in a row)
        let indicator_y = 50;
        let indicator_x_start = 50;
        let dot_spacing = 30;
        for i in 0..point_num {
            let dot_x = indicator_x_start + (i as i32 * dot_spacing);
            draw_filled_rect_mut(
                &mut img,
                Rect::at(dot_x, indicator_y).of_size(20, 20),
                Rgb([0u8, 255u8, 0u8]),
            );
        }

        // Draw screen dimension indicators at corners
        // Top-left corner indicator
        draw_filled_rect_mut(
            &mut img,
            Rect::at(5, 5).of_size(50, 3),
            Rgb([150u8, 150u8, 150u8]),
        );
        draw_filled_rect_mut(
            &mut img,
            Rect::at(5, 5).of_size(3, 50),
            Rgb([150u8, 150u8, 150u8]),
        );

        // Bottom-right corner indicator
        draw_filled_rect_mut(
            &mut img,
            Rect::at(width as i32 - 55, height as i32 - 8).of_size(50, 3),
            Rgb([150u8, 150u8, 150u8]),
        );
        draw_filled_rect_mut(
            &mut img,
            Rect::at(width as i32 - 8, height as i32 - 55).of_size(3, 50),
            Rgb([150u8, 150u8, 150u8]),
        );

        // Encode to PNG and base64
        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Png).unwrap();
        STANDARD.encode(buffer.into_inner())
    }

    /// Ask the LLM to identify the marker position in the image.
    async fn ask_llm_for_position(
        &self,
        model_client: &ModelClient,
        image_base64: &str,
        point_num: usize,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<(i32, i32), String> {
        let prompt = if self.config.lang == "cn" {
            format!(
                "这是一张坐标校准图片。图片中有一个红色方块，方块中间有一个黄色十字和黑色中心点。\n\
                这是第 {} 个校准点（图片左上角有 {} 个绿色方块表示）。\n\
                屏幕尺寸为 {}x{} 像素（宽x高）。\n\n\
                请仔细观察红色方块中心（黑色点）的位置，输出其精确的像素坐标。\n\
                坐标原点在左上角，X向右增加，Y向下增加。\n\n\
                只需要输出坐标，格式为: [x, y]\n\
                例如: [540, 960]",
                point_num,
                point_num,
                screen_width,
                screen_height
            )
        } else {
            format!(
                "This is a coordinate calibration image. There is a red square with a yellow crosshair and a black center dot.\n\
                This is calibration point {} (indicated by {} green squares at top-left).\n\
                Screen size is {}x{} pixels (width x height).\n\n\
                Please observe the center of the red marker (black dot) carefully and report its exact pixel coordinates.\n\
                Origin is at top-left, X increases rightward, Y increases downward.\n\n\
                Only output the coordinates in format: [x, y]\n\
                Example: [540, 960]",
                point_num,
                point_num,
                screen_width,
                screen_height
            )
        };

        let messages = vec![
            MessageBuilder::create_user_message(&prompt, Some(image_base64)),
        ];

        let response = model_client
            .request(&messages)
            .await
            .map_err(|e| e.to_string())?;

        // Parse the coordinates from response
        self.parse_coordinates(&response.raw_content)
    }

    /// Parse coordinates from LLM response.
    fn parse_coordinates(&self, response: &str) -> Result<(i32, i32), String> {
        // Try to find [x, y] pattern
        let re = regex::Regex::new(r"\[(\d+)\s*,\s*(\d+)\]").unwrap();
        
        if let Some(captures) = re.captures(response) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X coordinate")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y coordinate")?;
            return Ok((x, y));
        }

        // Try to find element=[x, y] pattern (from action format)
        let re2 = regex::Regex::new(r"element\s*=\s*\[(\d+)\s*,\s*(\d+)\]").unwrap();
        if let Some(captures) = re2.captures(response) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X coordinate")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y coordinate")?;
            return Ok((x, y));
        }

        // Try to find x=... y=... pattern
        let re3 = regex::Regex::new(r"x\s*[=:]\s*(\d+).*y\s*[=:]\s*(\d+)").unwrap();
        if let Some(captures) = re3.captures(&response.to_lowercase()) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X coordinate")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y coordinate")?;
            return Ok((x, y));
        }

        // Try to find (x, y) pattern
        let re4 = regex::Regex::new(r"\((\d+)\s*,\s*(\d+)\)").unwrap();
        if let Some(captures) = re4.captures(response) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X coordinate")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y coordinate")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y coordinate")?;
            return Ok((x, y));
        }

        Err(format!("Could not parse coordinates from response: {}", response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coordinates_bracket() {
        let calibrator = CoordinateCalibrator::new(CalibrationConfig::default());
        
        let result = calibrator.parse_coordinates("[540, 960]");
        assert!(result.is_ok());
        let (x, y) = result.unwrap();
        assert_eq!(x, 540);
        assert_eq!(y, 960);
    }

    #[test]
    fn test_parse_coordinates_element() {
        let calibrator = CoordinateCalibrator::new(CalibrationConfig::default());
        
        let result = calibrator.parse_coordinates("do(action=\"Tap\", element=[270, 480])");
        assert!(result.is_ok());
        let (x, y) = result.unwrap();
        assert_eq!(x, 270);
        assert_eq!(y, 480);
    }

    #[test]
    fn test_parse_coordinates_parenthesis() {
        let calibrator = CoordinateCalibrator::new(CalibrationConfig::default());
        
        let result = calibrator.parse_coordinates("The marker is at (300, 500)");
        assert!(result.is_ok());
        let (x, y) = result.unwrap();
        assert_eq!(x, 300);
        assert_eq!(y, 500);
    }

    #[test]
    fn test_calibration_config() {
        let config = CalibrationConfig::default()
            .with_lang("en")
            .with_device_id("device123");
        
        assert_eq!(config.lang, "en");
        assert_eq!(config.device_id, Some("device123".to_string()));
        assert_eq!(config.calibration_points.len(), 5);
    }
}
