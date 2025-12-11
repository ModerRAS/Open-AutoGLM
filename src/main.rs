//! Phone Agent - AI-powered Android phone automation
//!
//! This is the main entry point for the phone-agent CLI tool.

use phone_agent::{AgentConfig, ModelConfig, PhoneAgent, DEFAULT_COORDINATE_SCALE};
use phone_agent::calibration::{CalibrationConfig, CoordinateCalibrator};
use phone_agent::model::{ModelClient, DEFAULT_MAX_RETRIES, DEFAULT_RETRY_DELAY_SECS};
use std::env;
use std::io::{self, BufRead, Write};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (ignore errors if file doesn't exist)
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    // Get configuration from environment or use defaults
    let base_url = env::var("MODEL_BASE_URL").unwrap_or_else(|_| "http://localhost:8000/v1".to_string());
    let api_key = env::var("MODEL_API_KEY").unwrap_or_else(|_| "EMPTY".to_string());
    let model_name = env::var("MODEL_NAME").unwrap_or_else(|_| "autoglm-phone-9b".to_string());
    let device_id = env::var("ADB_DEVICE_ID").ok();
    let lang = env::var("AGENT_LANG").unwrap_or_else(|_| "cn".to_string());
    
    // Get retry configuration from environment
    let max_retries: u32 = env::var("MODEL_MAX_RETRIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_RETRIES);
    let retry_delay: u64 = env::var("MODEL_RETRY_DELAY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_RETRY_DELAY_SECS);
    
    // Get coordinate scale factors from environment (default: 1.61)
    let scale_x: f64 = env::var("COORDINATE_SCALE_X")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_COORDINATE_SCALE);
    let scale_y: f64 = env::var("COORDINATE_SCALE_Y")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_COORDINATE_SCALE);
    // Allow setting both X and Y with a single variable
    let (scale_x, scale_y) = if let Ok(uniform_scale) = env::var("COORDINATE_SCALE") {
        let scale: f64 = uniform_scale.parse().unwrap_or(DEFAULT_COORDINATE_SCALE);
        (scale, scale)
    } else {
        (scale_x, scale_y)
    };

    // Check if calibration is requested
    let enable_calibration = env::var("ENABLE_CALIBRATION")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);
    let calibration_only = args.iter().any(|arg| arg == "--calibrate");

    // Build model config
    let model_config = ModelConfig::default()
        .with_base_url(&base_url)
        .with_api_key(&api_key)
        .with_model_name(&model_name)
        .with_max_retries(max_retries)
        .with_retry_delay(retry_delay);

    // Build agent config
    let mut agent_config = AgentConfig::default()
        .with_lang(&lang)
        .with_scale(scale_x, scale_y);
    let device_id_clone = device_id.clone();
    if let Some(id) = device_id {
        agent_config = agent_config.with_device_id(id);
    }

    println!("🤖 Phone Agent - AI-powered Android Automation");
    println!("================================================");
    println!("Model: {} @ {}", model_name, base_url);
    println!("Language: {}", lang);
    println!("Coordinate Scale: X={:.2}, Y={:.2}", scale_x, scale_y);
    println!("Retry: max {} attempts, {}s delay", max_retries, retry_delay);
    if let Some(ref id) = agent_config.device_id {
        println!("Device: {}", id);
    }
    if enable_calibration || calibration_only {
        println!("Calibration: enabled");
    }
    println!("================================================\n");

    // Run calibration if requested
    let (scale_x, scale_y) = if enable_calibration || calibration_only {
        println!("🎯 Starting coordinate calibration...\n");
        
        // Build calibration config - screen size will be auto-detected from device screenshot
        let mut calibration_config = CalibrationConfig::default()
            .with_lang(&lang);
        
        if let Some(ref id) = device_id_clone {
            calibration_config = calibration_config.with_device_id(id);
        }
        
        let calibrator = CoordinateCalibrator::new(calibration_config);
        let model_client = ModelClient::new(model_config.clone());
        
        let result = calibrator.calibrate(&model_client).await;
        
        if result.success {
            println!("\n🎯 Detected screen size: {}x{}", result.screen_width, result.screen_height);
            println!("🎯 Using calibrated scale factors: X={:.4}, Y={:.4}\n", result.scale_x, result.scale_y);
            (result.scale_x, result.scale_y)
        } else {
            println!("\n⚠️ Calibration failed: {:?}", result.error);
            println!("   Using default scale factors: X={:.4}, Y={:.4}\n", scale_x, scale_y);
            (scale_x, scale_y)
        }
    } else {
        (scale_x, scale_y)
    };

    // Exit if calibration-only mode
    if calibration_only {
        println!("Calibration complete. Suggested environment variables:");
        println!("  COORDINATE_SCALE_X={:.4}", scale_x);
        println!("  COORDINATE_SCALE_Y={:.4}", scale_y);
        println!("\nOr use unified scale:");
        let avg_scale = (scale_x + scale_y) / 2.0;
        println!("  COORDINATE_SCALE={:.4}", avg_scale);
        return Ok(());
    }

    // Update agent config with calibrated scale factors
    let agent_config = agent_config.with_scale(scale_x, scale_y);

    // Create agent
    let mut agent = PhoneAgent::new(model_config, agent_config, None, None);

    // Check if task is provided as argument
    if args.len() > 1 {
        let task = args[1..].join(" ");
        println!("📝 Task: {}\n", task);
        
        match agent.run(&task).await {
            Ok(result) => {
                println!("\n✅ Result: {}", result);
            }
            Err(e) => {
                eprintln!("\n❌ Error: {}", e);
            }
        }
    } else {
        // Interactive mode
        println!("Interactive mode. Type your task and press Enter.");
        println!("Type 'quit' or 'exit' to exit.\n");

        let stdin = io::stdin();
        loop {
            print!("📝 Task: ");
            io::stdout().flush()?;

            let mut line = String::new();
            stdin.lock().read_line(&mut line)?;
            let task = line.trim();

            if task.is_empty() {
                continue;
            }

            if task == "quit" || task == "exit" {
                println!("Goodbye! 👋");
                break;
            }

            agent.reset();
            match agent.run(task).await {
                Ok(result) => {
                    println!("\n✅ Result: {}\n", result);
                }
                Err(e) => {
                    eprintln!("\n❌ Error: {}\n", e);
                }
            }
        }
    }

    Ok(())
}
