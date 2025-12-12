//! Coordinate calibration for automatic scale factor detection.
//!
//! This module provides functionality to automatically calibrate coordinate
//! scale factors by generating test images with known marker positions and
//! asking the LLM to identify those positions.
//!
//! Two calibration modes are available:
//! - **Simple mode**: Uses colored markers at specific positions
//! - **Complex mode**: Simulates real UI layouts (comment lists, etc.)

use ab_glyph::{FontRef, PxScale};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use std::io::Cursor;

use crate::adb::get_screenshot;
use crate::model::{MessageBuilder, ModelClient};

/// Calibration mode
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CalibrationMode {
    /// Simple mode: colored markers at specific positions
    #[default]
    Simple,
    /// Complex mode: simulates real UI with comment-like layouts
    Complex,
}

/// Default calibration points as (x_ratio, y_ratio) where 0.0-1.0 represents screen percentage
pub const DEFAULT_CALIBRATION_POINTS: [(f64, f64); 5] = [
    (0.5, 0.5),   // Center
    (0.25, 0.25), // Top-left quadrant
    (0.75, 0.25), // Top-right quadrant
    (0.25, 0.75), // Bottom-left quadrant
    (0.75, 0.75), // Bottom-right quadrant
];

/// A simulated comment for complex calibration
#[derive(Debug, Clone)]
pub struct MockComment {
    pub username: String,
    pub time: String,
    pub content: String,
    pub likes: u32,
    pub has_reply_button: bool,
}

impl MockComment {
    fn random_comments_cn() -> Vec<Self> {
        vec![
            MockComment {
                username: "张三".to_string(),
                time: "3分钟前".to_string(),
                content: "这个功能太好用了，感谢分享！".to_string(),
                likes: 128,
                has_reply_button: true,
            },
            MockComment {
                username: "李四".to_string(),
                time: "15分钟前".to_string(),
                content: "请问这个在哪里可以下载？".to_string(),
                likes: 45,
                has_reply_button: true,
            },
            MockComment {
                username: "王五".to_string(),
                time: "1小时前".to_string(),
                content: "已收藏，回头慢慢看".to_string(),
                likes: 23,
                has_reply_button: true,
            },
            MockComment {
                username: "赵六".to_string(),
                time: "2小时前".to_string(),
                content: "这个教程讲得很清楚，新手也能看懂，强烈推荐给大家！".to_string(),
                likes: 89,
                has_reply_button: true,
            },
            MockComment {
                username: "钱七".to_string(),
                time: "3小时前".to_string(),
                content: "学到了".to_string(),
                likes: 12,
                has_reply_button: false,
            },
            MockComment {
                username: "孙八".to_string(),
                time: "昨天".to_string(),
                content: "能不能出一个进阶版的教程？".to_string(),
                likes: 67,
                has_reply_button: true,
            },
            MockComment {
                username: "周九".to_string(),
                time: "昨天".to_string(),
                content: "支持！期待更多内容".to_string(),
                likes: 34,
                has_reply_button: true,
            },
            MockComment {
                username: "吴十".to_string(),
                time: "2天前".to_string(),
                content: "这是我见过最详细的教程了，博主用心了".to_string(),
                likes: 156,
                has_reply_button: true,
            },
        ]
    }

    fn random_comments_en() -> Vec<Self> {
        vec![
            MockComment {
                username: "John".to_string(),
                time: "3 min ago".to_string(),
                content: "This feature is amazing, thanks for sharing!".to_string(),
                likes: 128,
                has_reply_button: true,
            },
            MockComment {
                username: "Alice".to_string(),
                time: "15 min ago".to_string(),
                content: "Where can I download this?".to_string(),
                likes: 45,
                has_reply_button: true,
            },
            MockComment {
                username: "Bob".to_string(),
                time: "1 hour ago".to_string(),
                content: "Saved for later".to_string(),
                likes: 23,
                has_reply_button: true,
            },
            MockComment {
                username: "Charlie".to_string(),
                time: "2 hours ago".to_string(),
                content: "This tutorial is very clear, even beginners can understand it!"
                    .to_string(),
                likes: 89,
                has_reply_button: true,
            },
            MockComment {
                username: "David".to_string(),
                time: "3 hours ago".to_string(),
                content: "Learned a lot".to_string(),
                likes: 12,
                has_reply_button: false,
            },
            MockComment {
                username: "Eve".to_string(),
                time: "Yesterday".to_string(),
                content: "Can you make an advanced version?".to_string(),
                likes: 67,
                has_reply_button: true,
            },
        ]
    }
}

/// Configuration for calibration process.
#[derive(Debug, Clone)]
pub struct CalibrationConfig {
    /// Calibration mode
    pub mode: CalibrationMode,
    /// Calibration points as (x_ratio, y_ratio) pairs (for simple mode)
    pub calibration_points: Vec<(f64, f64)>,
    /// Number of calibration rounds for complex mode
    pub complex_rounds: usize,
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
            mode: CalibrationMode::Simple,
            calibration_points: DEFAULT_CALIBRATION_POINTS.to_vec(),
            complex_rounds: 5,
            lang: "cn".to_string(),
            marker_size_ratio: 0.05,
            device_id: None,
        }
    }
}

impl CalibrationConfig {
    pub fn with_mode(mut self, mode: CalibrationMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn with_complex_rounds(mut self, rounds: usize) -> Self {
        self.complex_rounds = rounds;
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
    /// Calibration mode used
    pub mode: CalibrationMode,
}

/// Result for a single calibration point.
#[derive(Debug, Clone)]
pub struct PointCalibrationResult {
    /// Description of what was being located
    pub description: String,
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

/// Target element in complex calibration
#[derive(Debug, Clone)]
struct ComplexTarget {
    description: String,
    element_type: String,
    x: i32,
    y: i32,
}

/// Coordinate calibrator for automatic scale factor detection.
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
    pub async fn calibrate(&self, model_client: &ModelClient) -> CalibrationResult {
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
                    mode: self.config.mode,
                };
            }
        };

        match self.config.mode {
            CalibrationMode::Simple => {
                self.calibrate_simple(model_client, screen_width, screen_height)
                    .await
            }
            CalibrationMode::Complex => {
                self.calibrate_complex(model_client, screen_width, screen_height)
                    .await
            }
        }
    }

    /// Simple calibration with colored markers
    async fn calibrate_simple(
        &self,
        model_client: &ModelClient,
        screen_width: u32,
        screen_height: u32,
    ) -> CalibrationResult {
        println!("\n🎯 Running SIMPLE calibration mode...\n");

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

            let image_base64 = self.generate_simple_calibration_image(
                expected_x,
                expected_y,
                i + 1,
                screen_width,
                screen_height,
                marker_size,
            );

            match self
                .ask_llm_for_simple_position(
                    model_client,
                    &image_base64,
                    i + 1,
                    screen_width,
                    screen_height,
                )
                .await
            {
                Ok((reported_x, reported_y)) => {
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

                    if ratio_x > 0.5 && ratio_x < 2.0 && ratio_y > 0.5 && ratio_y < 2.0 {
                        total_ratio_x += ratio_x;
                        total_ratio_y += ratio_y;
                        valid_points += 1;
                    } else {
                        println!("   ⚠️ Ratio out of reasonable range, skipping this point");
                    }

                    point_results.push(PointCalibrationResult {
                        description: format!(
                            "Point {} ({:.0}%, {:.0}%)",
                            i + 1,
                            x_ratio * 100.0,
                            y_ratio * 100.0
                        ),
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
                        description: format!("Point {} (failed)", i + 1),
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

        self.build_result(
            point_results,
            total_ratio_x,
            total_ratio_y,
            valid_points,
            screen_width,
            screen_height,
        )
    }

    /// Complex calibration with simulated UI layouts
    async fn calibrate_complex(
        &self,
        model_client: &ModelClient,
        screen_width: u32,
        screen_height: u32,
    ) -> CalibrationResult {
        println!("\n🎯 Running COMPLEX calibration mode (comment list simulation)...\n");

        let mut point_results = Vec::new();
        let mut total_ratio_x = 0.0;
        let mut total_ratio_y = 0.0;
        let mut valid_points = 0;

        let comments = if self.config.lang == "cn" {
            MockComment::random_comments_cn()
        } else {
            MockComment::random_comments_en()
        };

        for round in 0..self.config.complex_rounds {
            println!("📋 Round {}/{}:", round + 1, self.config.complex_rounds);

            // Generate complex UI image and get a random target
            let (image_base64, target) = self.generate_complex_calibration_image(
                screen_width,
                screen_height,
                &comments,
                round,
            );

            println!(
                "   Target: {} \"{}\" at ({}, {})",
                target.element_type, target.description, target.x, target.y
            );

            match self
                .ask_llm_for_complex_position(
                    model_client,
                    &image_base64,
                    &target,
                    screen_width,
                    screen_height,
                )
                .await
            {
                Ok((reported_x, reported_y)) => {
                    let ratio_x = if reported_x != 0 {
                        target.x as f64 / reported_x as f64
                    } else {
                        1.0
                    };
                    let ratio_y = if reported_y != 0 {
                        target.y as f64 / reported_y as f64
                    } else {
                        1.0
                    };

                    println!(
                        "   LLM reported: ({}, {}), ratios: X={:.3}, Y={:.3}",
                        reported_x, reported_y, ratio_x, ratio_y
                    );

                    if ratio_x > 0.5 && ratio_x < 2.0 && ratio_y > 0.5 && ratio_y < 2.0 {
                        total_ratio_x += ratio_x;
                        total_ratio_y += ratio_y;
                        valid_points += 1;
                    } else {
                        println!("   ⚠️ Ratio out of reasonable range, skipping");
                    }

                    point_results.push(PointCalibrationResult {
                        description: format!("{}: {}", target.element_type, target.description),
                        expected_x: target.x,
                        expected_y: target.y,
                        reported_x,
                        reported_y,
                        ratio_x,
                        ratio_y,
                    });
                }
                Err(e) => {
                    println!("   ❌ Failed: {}", e);
                    point_results.push(PointCalibrationResult {
                        description: format!(
                            "{}: {} (failed)",
                            target.element_type, target.description
                        ),
                        expected_x: target.x,
                        expected_y: target.y,
                        reported_x: 0,
                        reported_y: 0,
                        ratio_x: 1.0,
                        ratio_y: 1.0,
                    });
                }
            }
        }

        self.build_result(
            point_results,
            total_ratio_x,
            total_ratio_y,
            valid_points,
            screen_width,
            screen_height,
        )
    }

    /// Build calibration result from collected data
    fn build_result(
        &self,
        point_results: Vec<PointCalibrationResult>,
        total_ratio_x: f64,
        total_ratio_y: f64,
        valid_points: usize,
        screen_width: u32,
        screen_height: u32,
    ) -> CalibrationResult {
        if valid_points == 0 {
            return CalibrationResult {
                scale_x: 1.0,
                scale_y: 1.0,
                screen_width,
                screen_height,
                point_results,
                success: false,
                error: Some("No valid calibration points".to_string()),
                mode: self.config.mode,
            };
        }

        let scale_x = total_ratio_x / valid_points as f64;
        let scale_y = total_ratio_y / valid_points as f64;

        println!("\n✅ Calibration complete!");
        println!("   Mode: {:?}", self.config.mode);
        println!("   Screen size: {}x{}", screen_width, screen_height);
        println!("   Valid points: {}/{}", valid_points, point_results.len());
        println!(
            "   Calculated scale factors: X={:.4}, Y={:.4}",
            scale_x, scale_y
        );

        CalibrationResult {
            scale_x,
            scale_y,
            screen_width,
            screen_height,
            point_results,
            success: true,
            error: None,
            mode: self.config.mode,
        }
    }

    /// Generate a simple calibration image with a marker at the specified position.
    fn generate_simple_calibration_image(
        &self,
        x: i32,
        y: i32,
        point_num: usize,
        width: u32,
        height: u32,
        marker_size: u32,
    ) -> String {
        let mut img = RgbImage::from_fn(width, height, |_, _| Rgb([30u8, 30u8, 30u8]));

        // Draw grid lines
        let grid_color = Rgb([60u8, 60u8, 60u8]);
        for i in 1..4 {
            let x_line = (width * i / 4) as i32;
            let y_line = (height * i / 4) as i32;
            draw_filled_rect_mut(&mut img, Rect::at(x_line, 0).of_size(2, height), grid_color);
            draw_filled_rect_mut(&mut img, Rect::at(0, y_line).of_size(width, 2), grid_color);
        }

        // Draw the main marker
        let marker_x = (x - marker_size as i32 / 2).max(0);
        let marker_y = (y - marker_size as i32 / 2).max(0);

        // Red filled marker
        draw_filled_rect_mut(
            &mut img,
            Rect::at(marker_x, marker_y).of_size(marker_size, marker_size),
            Rgb([255u8, 50u8, 50u8]),
        );

        // White border
        for offset in 0..3 {
            draw_hollow_rect_mut(
                &mut img,
                Rect::at(marker_x - offset as i32, marker_y - offset as i32)
                    .of_size(marker_size + offset * 2, marker_size + offset * 2),
                Rgb([255u8, 255u8, 255u8]),
            );
        }

        // Yellow crosshair
        let cross_thickness = 4;
        draw_filled_rect_mut(
            &mut img,
            Rect::at(x - cross_thickness as i32 / 2, marker_y)
                .of_size(cross_thickness, marker_size),
            Rgb([255u8, 255u8, 0u8]),
        );
        draw_filled_rect_mut(
            &mut img,
            Rect::at(marker_x, y - cross_thickness as i32 / 2)
                .of_size(marker_size, cross_thickness),
            Rgb([255u8, 255u8, 0u8]),
        );

        // Black center dot
        let dot_size = 8;
        draw_filled_rect_mut(
            &mut img,
            Rect::at(x - dot_size as i32 / 2, y - dot_size as i32 / 2).of_size(dot_size, dot_size),
            Rgb([0u8, 0u8, 0u8]),
        );

        // Point number indicator
        let indicator_y = 50;
        let indicator_x_start = 50;
        for i in 0..point_num {
            let dot_x = indicator_x_start + (i as i32 * 30);
            draw_filled_rect_mut(
                &mut img,
                Rect::at(dot_x, indicator_y).of_size(20, 20),
                Rgb([0u8, 255u8, 0u8]),
            );
        }

        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Png).unwrap();
        STANDARD.encode(buffer.into_inner())
    }

    /// Generate a complex calibration image simulating a comment list UI
    fn generate_complex_calibration_image(
        &self,
        width: u32,
        height: u32,
        comments: &[MockComment],
        round: usize,
    ) -> (String, ComplexTarget) {
        let mut img = RgbImage::from_fn(width, height, |_, _| Rgb([255u8, 255u8, 255u8]));

        // UI dimensions based on screen size
        let padding = (width as f64 * 0.04) as i32;
        let avatar_size = (width as f64 * 0.10) as u32;
        let font_size_username = (width as f64 * 0.035) as f32;
        let font_size_time = (width as f64 * 0.028) as f32;
        let font_size_content = (width as f64 * 0.038) as f32;
        let font_size_button = (width as f64 * 0.030) as f32;
        let line_height = (height as f64 * 0.022) as i32;
        let comment_spacing = (height as f64 * 0.025) as i32;

        // Load font
        let font_data = include_bytes!("../../resources/NotoSansSC-Regular.ttf");
        let font = FontRef::try_from_slice(font_data).ok();

        // Draw header bar
        draw_filled_rect_mut(
            &mut img,
            Rect::at(0, 0).of_size(width, 120),
            Rgb([245u8, 245u8, 245u8]),
        );

        if let Some(ref f) = font {
            let title = if self.config.lang == "cn" {
                "评论区"
            } else {
                "Comments"
            };
            draw_text_mut(
                &mut img,
                Rgb([33u8, 33u8, 33u8]),
                padding,
                40,
                PxScale::from(font_size_username * 1.2),
                f,
                title,
            );
        }

        // Store all clickable targets
        let mut targets: Vec<ComplexTarget> = Vec::new();

        // Draw comments
        let mut y_offset = 140;

        for (idx, comment) in comments.iter().take(6).enumerate() {
            // Comment background (alternating)
            if idx % 2 == 0 {
                draw_filled_rect_mut(
                    &mut img,
                    Rect::at(0, y_offset).of_size(
                        width,
                        avatar_size + comment_spacing as u32 * 2 + line_height as u32 * 3,
                    ),
                    Rgb([250u8, 250u8, 250u8]),
                );
            }

            // Avatar
            let avatar_x = padding;
            let avatar_y = y_offset + comment_spacing;
            let avatar_center_x = avatar_x + avatar_size as i32 / 2;
            let avatar_center_y = avatar_y + avatar_size as i32 / 2;

            // Draw avatar as colored square
            let avatar_colors = [
                Rgb([66u8, 133u8, 244u8]),
                Rgb([219u8, 68u8, 55u8]),
                Rgb([244u8, 180u8, 0u8]),
                Rgb([15u8, 157u8, 88u8]),
                Rgb([171u8, 71u8, 188u8]),
                Rgb([255u8, 112u8, 67u8]),
            ];
            draw_filled_rect_mut(
                &mut img,
                Rect::at(avatar_x, avatar_y).of_size(avatar_size, avatar_size),
                avatar_colors[idx % avatar_colors.len()],
            );

            targets.push(ComplexTarget {
                description: comment.username.clone(),
                element_type: if self.config.lang == "cn" {
                    "头像".to_string()
                } else {
                    "Avatar".to_string()
                },
                x: avatar_center_x,
                y: avatar_center_y,
            });

            let text_x = avatar_x + avatar_size as i32 + padding;
            let mut text_y = avatar_y;

            if let Some(ref f) = font {
                // Username
                draw_text_mut(
                    &mut img,
                    Rgb([33u8, 33u8, 33u8]),
                    text_x,
                    text_y,
                    PxScale::from(font_size_username),
                    f,
                    &comment.username,
                );

                let username_center_x = text_x
                    + (comment.username.chars().count() as i32 * font_size_username as i32 / 3);
                targets.push(ComplexTarget {
                    description: comment.username.clone(),
                    element_type: if self.config.lang == "cn" {
                        "用户名".to_string()
                    } else {
                        "Username".to_string()
                    },
                    x: username_center_x,
                    y: text_y + font_size_username as i32 / 2,
                });

                // Time
                let time_x = width as i32
                    - padding
                    - (comment.time.chars().count() as i32 * font_size_time as i32 / 2);
                draw_text_mut(
                    &mut img,
                    Rgb([150u8, 150u8, 150u8]),
                    time_x,
                    text_y,
                    PxScale::from(font_size_time),
                    f,
                    &comment.time,
                );

                text_y += line_height + 10;

                // Content
                draw_text_mut(
                    &mut img,
                    Rgb([66u8, 66u8, 66u8]),
                    text_x,
                    text_y,
                    PxScale::from(font_size_content),
                    f,
                    &comment.content,
                );

                targets.push(ComplexTarget {
                    description: if comment.content.chars().count() > 10 {
                        format!(
                            "{}...",
                            comment.content.chars().take(10).collect::<String>()
                        )
                    } else {
                        comment.content.clone()
                    },
                    element_type: if self.config.lang == "cn" {
                        "评论内容".to_string()
                    } else {
                        "Comment".to_string()
                    },
                    x: text_x + 100,
                    y: text_y + font_size_content as i32 / 2,
                });

                text_y += line_height + 15;

                // Like button
                let like_text = format!("👍 {}", comment.likes);
                draw_text_mut(
                    &mut img,
                    Rgb([100u8, 100u8, 100u8]),
                    text_x,
                    text_y,
                    PxScale::from(font_size_button),
                    f,
                    &like_text,
                );

                let like_center_x = text_x + 30;
                targets.push(ComplexTarget {
                    description: format!("{}", comment.likes),
                    element_type: if self.config.lang == "cn" {
                        "点赞按钮".to_string()
                    } else {
                        "Like button".to_string()
                    },
                    x: like_center_x,
                    y: text_y + font_size_button as i32 / 2,
                });

                // Reply button
                if comment.has_reply_button {
                    let reply_text = if self.config.lang == "cn" {
                        "回复"
                    } else {
                        "Reply"
                    };
                    let reply_x = text_x + 150;
                    draw_text_mut(
                        &mut img,
                        Rgb([100u8, 100u8, 100u8]),
                        reply_x,
                        text_y,
                        PxScale::from(font_size_button),
                        f,
                        reply_text,
                    );

                    targets.push(ComplexTarget {
                        description: comment.username.clone(),
                        element_type: if self.config.lang == "cn" {
                            "回复按钮".to_string()
                        } else {
                            "Reply button".to_string()
                        },
                        x: reply_x + 30,
                        y: text_y + font_size_button as i32 / 2,
                    });
                }
            }

            // Draw separator line
            y_offset += avatar_size as i32 + comment_spacing * 2 + line_height * 3;
            draw_filled_rect_mut(
                &mut img,
                Rect::at(padding, y_offset).of_size(width - padding as u32 * 2, 1),
                Rgb([230u8, 230u8, 230u8]),
            );
            y_offset += 10;
        }

        // Select a target based on round number
        let target_idx = round % targets.len().max(1);
        let target = if targets.is_empty() {
            ComplexTarget {
                description: "fallback".to_string(),
                element_type: "Unknown".to_string(),
                x: (width / 2) as i32,
                y: (height / 2) as i32,
            }
        } else {
            targets[target_idx].clone()
        };

        // Highlight the target with a red dot
        draw_filled_rect_mut(
            &mut img,
            Rect::at(target.x - 5, target.y - 5).of_size(10, 10),
            Rgb([255u8, 0u8, 0u8]),
        );

        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, image::ImageFormat::Png).unwrap();
        (STANDARD.encode(buffer.into_inner()), target)
    }

    /// Ask the LLM to identify the marker position in a simple calibration image.
    async fn ask_llm_for_simple_position(
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
                point_num, point_num, screen_width, screen_height
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
                point_num, point_num, screen_width, screen_height
            )
        };

        let messages = vec![MessageBuilder::create_user_message(
            &prompt,
            Some(image_base64),
        )];
        let response = model_client
            .request(&messages)
            .await
            .map_err(|e| e.to_string())?;
        self.parse_coordinates(&response.raw_content)
    }

    /// Ask the LLM to find a specific element in a complex UI image.
    async fn ask_llm_for_complex_position(
        &self,
        model_client: &ModelClient,
        image_base64: &str,
        target: &ComplexTarget,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<(i32, i32), String> {
        let prompt = if self.config.lang == "cn" {
            format!(
                "这是一个评论区界面的截图。屏幕尺寸为 {}x{} 像素（宽x高）。\n\n\
                界面中有一个红色小圆点标记了目标位置。\n\
                目标是: {} - \"{}\"\n\n\
                请找到这个红色标记点的精确像素坐标。\n\
                坐标原点在左上角，X向右增加，Y向下增加。\n\n\
                只需要输出坐标，格式为: [x, y]\n\
                例如: [540, 960]",
                screen_width, screen_height, target.element_type, target.description
            )
        } else {
            format!(
                "This is a screenshot of a comment section UI. Screen size is {}x{} pixels (width x height).\n\n\
                There is a small red dot marking the target position.\n\
                Target: {} - \"{}\"\n\n\
                Please find the exact pixel coordinates of this red marker.\n\
                Origin is at top-left, X increases rightward, Y increases downward.\n\n\
                Only output the coordinates in format: [x, y]\n\
                Example: [540, 960]",
                screen_width, screen_height, target.element_type, target.description
            )
        };

        let messages = vec![MessageBuilder::create_user_message(
            &prompt,
            Some(image_base64),
        )];
        let response = model_client
            .request(&messages)
            .await
            .map_err(|e| e.to_string())?;
        self.parse_coordinates(&response.raw_content)
    }

    /// Parse coordinates from LLM response.
    fn parse_coordinates(&self, response: &str) -> Result<(i32, i32), String> {
        // Try [x, y] pattern
        let re = regex::Regex::new(r"\[(\d+)\s*,\s*(\d+)\]").unwrap();
        if let Some(captures) = re.captures(response) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y")?;
            return Ok((x, y));
        }

        // Try element=[x, y] pattern
        let re2 = regex::Regex::new(r"element\s*=\s*\[(\d+)\s*,\s*(\d+)\]").unwrap();
        if let Some(captures) = re2.captures(response) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y")?;
            return Ok((x, y));
        }

        // Try (x, y) pattern
        let re3 = regex::Regex::new(r"\((\d+)\s*,\s*(\d+)\)").unwrap();
        if let Some(captures) = re3.captures(response) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y")?;
            return Ok((x, y));
        }

        // Try x=... y=... pattern
        let re4 = regex::Regex::new(r"x\s*[=:]\s*(\d+).*y\s*[=:]\s*(\d+)").unwrap();
        if let Some(captures) = re4.captures(&response.to_lowercase()) {
            let x: i32 = captures
                .get(1)
                .ok_or("Missing X")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid X")?;
            let y: i32 = captures
                .get(2)
                .ok_or("Missing Y")?
                .as_str()
                .parse()
                .map_err(|_| "Invalid Y")?;
            return Ok((x, y));
        }

        Err(format!("Could not parse coordinates from: {}", response))
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
    fn test_calibration_config_simple() {
        let config = CalibrationConfig::default()
            .with_mode(CalibrationMode::Simple)
            .with_lang("en");
        assert_eq!(config.mode, CalibrationMode::Simple);
        assert_eq!(config.lang, "en");
    }

    #[test]
    fn test_calibration_config_complex() {
        let config = CalibrationConfig::default()
            .with_mode(CalibrationMode::Complex)
            .with_complex_rounds(10)
            .with_device_id("device123");
        assert_eq!(config.mode, CalibrationMode::Complex);
        assert_eq!(config.complex_rounds, 10);
        assert_eq!(config.device_id, Some("device123".to_string()));
    }
}
