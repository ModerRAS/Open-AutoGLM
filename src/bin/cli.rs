//! Phone Agent - AI-powered Android phone automation
//!
//! This is the CLI entry point for the phone-agent tool.
//! Run with: cargo run --bin phone-agent

use phone_agent::calibration::{CalibrationConfig, CalibrationMode, CoordinateCalibrator};
use phone_agent::model::{ModelClient, DEFAULT_MAX_RETRIES, DEFAULT_RETRY_DELAY_SECS};
use phone_agent::{AgentConfig, CoordinateSystem, ModelConfig, PhoneAgent, DEFAULT_COORDINATE_SCALE};
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
    let base_url =
        env::var("MODEL_BASE_URL").unwrap_or_else(|_| "http://localhost:8000/v1".to_string());
    let api_key = env::var("MODEL_API_KEY").unwrap_or_else(|_| "EMPTY".to_string());
    let model_name = env::var("MODEL_NAME").unwrap_or_else(|_| "autoglm-phone-9b".to_string());
    let device_id = env::var("ADB_DEVICE_ID").ok();
    let lang = env::var("AGENT_LANG").unwrap_or_else(|_| "cn".to_string());

    // Get coordinate system from environment (default: relative)
    // "relative" or "rel" for relative coordinates (0-999 range, original AutoGLM-Phone style, default)
    // "absolute" or "abs" for absolute pixel coordinates
    let coordinate_system = match env::var("COORDINATE_SYSTEM")
        .unwrap_or_else(|_| "relative".to_string())
        .to_lowercase()
        .as_str()
    {
        "absolute" | "abs" => CoordinateSystem::Absolute,
        _ => CoordinateSystem::Relative,
    };

    // Get retry configuration from environment
    let max_retries: u32 = env::var("MODEL_MAX_RETRIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_RETRIES);
    let retry_delay: u64 = env::var("MODEL_RETRY_DELAY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_RETRY_DELAY_SECS);

    // Get coordinate scale factors from environment (only used for absolute mode)
    // Default is 1.0 for relative mode, 1.61 for absolute mode
    let default_scale = match coordinate_system {
        CoordinateSystem::Relative => 1.0,
        CoordinateSystem::Absolute => DEFAULT_COORDINATE_SCALE,
    };
    let scale_x: f64 = env::var("COORDINATE_SCALE_X")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_scale);
    let scale_y: f64 = env::var("COORDINATE_SCALE_Y")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_scale);
    // Allow setting both X and Y with a single variable
    let (scale_x, scale_y) = if let Ok(uniform_scale) = env::var("COORDINATE_SCALE") {
        let scale: f64 = uniform_scale.parse().unwrap_or(default_scale);
        (scale, scale)
    } else {
        (scale_x, scale_y)
    };

    // Check if calibration is requested
    let enable_calibration = env::var("ENABLE_CALIBRATION")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);
    let calibration_simple = args.iter().any(|arg| arg == "--calibrate");
    let calibration_complex = args.iter().any(|arg| arg == "--calibrate-complex");
    let calibration_only = calibration_simple || calibration_complex;

    // Determine calibration mode
    let calibration_mode = if calibration_complex {
        CalibrationMode::Complex
    } else {
        // Check environment variable for mode
        let mode_env = env::var("CALIBRATION_MODE").unwrap_or_default();
        if mode_env.to_lowercase() == "complex" {
            CalibrationMode::Complex
        } else {
            CalibrationMode::Simple
        }
    };

    // Get complex calibration rounds from environment
    let complex_rounds: usize = env::var("CALIBRATION_COMPLEX_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    // Build model config
    let model_config = ModelConfig::default()
        .with_base_url(&base_url)
        .with_api_key(&api_key)
        .with_model_name(&model_name)
        .with_max_retries(max_retries)
        .with_retry_delay(retry_delay);

    // Build agent config with coordinate system
    let mut agent_config = AgentConfig::default()
        .with_lang(&lang)
        .with_coordinate_system(coordinate_system)
        .with_scale(scale_x, scale_y);
    let device_id_clone = device_id.clone();
    if let Some(id) = device_id {
        agent_config = agent_config.with_device_id(id);
    }

    let coord_system_name = match coordinate_system {
        CoordinateSystem::Relative => "Relative (0-999)",
        CoordinateSystem::Absolute => "Absolute (pixels)",
    };

    println!("🤖 Phone Agent - AI-powered Android Automation");
    println!("================================================");
    println!("Model: {} @ {}", model_name, base_url);
    println!("Language: {}", lang);
    println!("Coordinate System: {}", coord_system_name);
    if coordinate_system == CoordinateSystem::Absolute {
        println!("Coordinate Scale: X={:.2}, Y={:.2}", scale_x, scale_y);
    }
    println!(
        "Retry: max {} attempts, {}s delay",
        max_retries, retry_delay
    );
    if let Some(ref id) = agent_config.device_id {
        println!("Device: {}", id);
    }
    if enable_calibration || calibration_only {
        println!("Calibration: enabled ({:?})", calibration_mode);
    }
    println!("================================================\n");

    // Run calibration if requested
    let (scale_x, scale_y) = if enable_calibration || calibration_only {
        println!(
            "🎯 Starting coordinate calibration ({:?} mode)...\n",
            calibration_mode
        );

        // Build calibration config - screen size will be auto-detected from device screenshot
        let mut calibration_config = CalibrationConfig::default()
            .with_mode(calibration_mode)
            .with_lang(&lang)
            .with_complex_rounds(complex_rounds);

        if let Some(ref id) = device_id_clone {
            calibration_config = calibration_config.with_device_id(id);
        }

        let calibrator = CoordinateCalibrator::new(calibration_config);
        let model_client = ModelClient::new(model_config.clone());

        let result = calibrator.calibrate(&model_client).await;

        if result.success {
            println!("\n🎯 Calibration mode: {:?}", result.mode);
            println!(
                "🎯 Detected screen size: {}x{}",
                result.screen_width, result.screen_height
            );
            println!(
                "🎯 Using calibrated scale factors: X={:.4}, Y={:.4}\n",
                result.scale_x, result.scale_y
            );
            (result.scale_x, result.scale_y)
        } else {
            println!("\n⚠️ Calibration failed: {:?}", result.error);
            println!(
                "   Using default scale factors: X={:.4}, Y={:.4}\n",
                scale_x, scale_y
            );
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

    // Check for dual loop mode
    let dual_loop_mode = env::var("DUAL_LOOP_MODE")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    if dual_loop_mode {
        // Dual loop mode
        run_dual_loop_mode(model_config, agent_config).await?;
    } else {
        // Single loop mode (original)
        run_single_loop_mode(model_config, agent_config, args).await?;
    }

    Ok(())
}

/// Run single loop mode (original behavior).
async fn run_single_loop_mode(
    model_config: phone_agent::ModelConfig,
    agent_config: phone_agent::AgentConfig,
    args: Vec<String>,
) -> anyhow::Result<()> {
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

/// Run dual loop mode (new architecture).
async fn run_dual_loop_mode(
    executor_model_config: phone_agent::ModelConfig,
    executor_agent_config: phone_agent::AgentConfig,
) -> anyhow::Result<()> {
    use phone_agent::{
        DualLoopConfig, DualLoopRunner, PlannerAgent, PlannerConfig,
    };

    println!("\n🔄 Dual Loop Mode Enabled");
    println!("================================================\n");

    // Get planner configuration from environment
    let planner_base_url = env::var("PLANNER_MODEL_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string());
    let planner_api_key = env::var("PLANNER_MODEL_API_KEY")
        .unwrap_or_else(|_| "EMPTY".to_string());
    let planner_model_name = env::var("PLANNER_MODEL_NAME")
        .unwrap_or_else(|_| "deepseek-chat".to_string());

    // Get dual loop configuration from environment
    let max_feedback_history: usize = env::var("MAX_EXECUTOR_FEEDBACK_HISTORY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let stuck_threshold: u32 = env::var("STUCK_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let prompt_memory_path = env::var("PROMPT_MEMORY_PATH")
        .unwrap_or_else(|_| "prompt_memory.json".to_string());
    let planner_interval: u64 = env::var("PLANNER_INTERVAL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let executor_interval: u64 = env::var("EXECUTOR_INTERVAL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    let lang = env::var("AGENT_LANG").unwrap_or_else(|_| "cn".to_string());

    println!("Planner Model: {} @ {}", planner_model_name, planner_base_url);
    println!("Feedback History: {} entries", max_feedback_history);
    println!("Stuck Threshold: {} consecutive", stuck_threshold);
    println!("Prompt Memory: {}", prompt_memory_path);
    println!("Intervals: Planner={}ms, Executor={}ms", planner_interval, executor_interval);
    println!("================================================\n");

    // Build planner config
    let planner_model_config = phone_agent::ModelConfig::default()
        .with_base_url(&planner_base_url)
        .with_api_key(&planner_api_key)
        .with_model_name(&planner_model_name);

    let planner_config = PlannerConfig::default()
        .with_model_config(planner_model_config)
        .with_max_feedback_history(max_feedback_history)
        .with_stuck_threshold(stuck_threshold)
        .with_prompt_memory_path(&prompt_memory_path)
        .with_lang(&lang);

    // Create planner
    let planner = PlannerAgent::new(
        planner_config,
        executor_model_config,
        executor_agent_config,
    );

    // Create dual loop runner
    let loop_config = DualLoopConfig::default()
        .with_planner_interval(planner_interval)
        .with_executor_interval(executor_interval);

    // Track last status to avoid duplicate prints
    use std::sync::{Arc, Mutex};
    let last_status: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let last_status_clone = last_status.clone();

    let runner = DualLoopRunner::new(planner, loop_config)
        .with_feedback_callback(move |feedback| {
            // Only print on status change
            let status_str = format!("{:?}", feedback.status);
            let mut last = last_status_clone.lock().unwrap();
            
            if last.as_ref() != Some(&status_str) {
                // Status changed, print it
                if matches!(feedback.status, 
                    phone_agent::ExecutorStatus::Completed | 
                    phone_agent::ExecutorStatus::Failed(_) |
                    phone_agent::ExecutorStatus::Stuck |
                    phone_agent::ExecutorStatus::Running
                ) {
                    println!("📡 Executor: {:?} (step {})", feedback.status, feedback.step_count);
                }
                *last = Some(status_str);
            }
        });

    // Run the dual loop
    let handle = runner.run().await;

    // Interactive input loop
    println!("Dual Loop Interactive Mode");
    println!("Type your task and press Enter. User input is queued to Planner.");
    println!("Type 'quit' or 'exit' to stop.\n");

    let stdin = io::stdin();
    loop {
        print!("📝 Input: ");
        io::stdout().flush()?;

        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let input = line.trim();

        if input.is_empty() {
            continue;
        }

        if input == "quit" || input == "exit" {
            println!("Stopping dual loop...");
            let _ = handle.stop().await;
            println!("Goodbye! 👋");
            break;
        }

        if input == "pause" {
            let _ = handle.pause().await;
            println!("⏸️ Dual loop paused");
            continue;
        }

        if input == "resume" {
            let _ = handle.resume().await;
            println!("▶️ Dual loop resumed");
            continue;
        }

        // Send input to planner
        match handle.send_user_input(input.to_string()).await {
            Ok(_) => println!("✅ Input queued to Planner\n"),
            Err(e) => eprintln!("❌ Failed to send input: {}\n", e),
        }
    }

    Ok(())
}
