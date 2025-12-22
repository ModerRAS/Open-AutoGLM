//! Phone Agent - AI-powered Android phone automation
//!
//! This is the CLI entry point for the phone-agent tool.
//! Run with: cargo run --bin phone-agent

use anyhow::anyhow;
use phone_agent::calibration::{CalibrationConfig, CalibrationMode, CoordinateCalibrator};
use phone_agent::model::ModelClient;
use phone_agent::{
    AgentConfig, AppSettings, CoordinateSystem, ModelConfig, PhoneAgent, DEFAULT_COORDINATE_SCALE,
};
use std::env;
use std::io::{self, BufRead, Write};

/// Merge stored settings with environment overrides.
fn load_settings_with_env() -> AppSettings {
    let mut settings = AppSettings::load();

    if let Ok(v) = env::var("MODEL_BASE_URL") {
        settings.base_url = v;
    }
    if let Ok(v) = env::var("MODEL_API_KEY") {
        settings.api_key = v;
    }
    if let Ok(v) = env::var("MODEL_NAME") {
        settings.model_name = v;
    }
    if let Ok(v) = env::var("ADB_DEVICE_ID") {
        settings.device_id = v;
    }
    if let Ok(v) = env::var("AGENT_LANG") {
        settings.lang = v;
    }
    if let Ok(v) = env::var("COORDINATE_SYSTEM") {
        settings.coordinate_system = v;
    }

    // Numbers with fallbacks to current settings value
    if let Ok(v) = env::var("MODEL_MAX_RETRIES") {
        if let Ok(parsed) = v.parse() {
            settings.max_retries = parsed;
        }
    }
    if let Ok(v) = env::var("MODEL_RETRY_DELAY") {
        if let Ok(parsed) = v.parse() {
            settings.retry_delay = parsed;
        }
    }
    if let Ok(v) = env::var("MAX_STEPS") {
        if let Ok(parsed) = v.parse() {
            settings.max_steps = parsed;
        }
    }
    if let Ok(v) = env::var("COORDINATE_SCALE_X") {
        if let Ok(parsed) = v.parse() {
            settings.scale_x = parsed;
        }
    }
    if let Ok(v) = env::var("COORDINATE_SCALE_Y") {
        if let Ok(parsed) = v.parse() {
            settings.scale_y = parsed;
        }
    }

    // Unified scale overrides individual values
    if let Ok(v) = env::var("COORDINATE_SCALE") {
        if let Ok(parsed) = v.parse() {
            settings.scale_x = parsed;
            settings.scale_y = parsed;
        }
    }

    if let Ok(v) = env::var("ENABLE_CALIBRATION") {
        settings.enable_calibration = v == "1" || v.to_lowercase() == "true";
    }
    if let Ok(v) = env::var("CALIBRATION_MODE") {
        settings.calibration_mode = v;
    }
    if let Ok(v) = env::var("CALIBRATION_COMPLEX_ROUNDS") {
        if let Ok(parsed) = v.parse() {
            settings.calibration_rounds = parsed;
        }
    }

    // Planner model settings
    if let Ok(v) = env::var("PLANNER_MODEL_BASE_URL") {
        settings.planner_base_url = v;
    }
    if let Ok(v) = env::var("PLANNER_MODEL_API_KEY") {
        settings.planner_api_key = v;
    }
    if let Ok(v) = env::var("PLANNER_MODEL_NAME") {
        settings.planner_model_name = v;
    }
    if let Ok(v) = env::var("MAX_EXECUTOR_FEEDBACK_HISTORY") {
        if let Ok(parsed) = v.parse() {
            settings.max_executor_feedback_history = parsed;
        }
    }
    if let Ok(v) = env::var("STUCK_THRESHOLD") {
        if let Ok(parsed) = v.parse() {
            settings.stuck_threshold = parsed;
        }
    }
    if let Ok(v) = env::var("PROMPT_MEMORY_PATH") {
        settings.prompt_memory_path = v;
    }
    if let Ok(v) = env::var("PLANNER_INTERVAL_MS") {
        if let Ok(parsed) = v.parse() {
            settings.planner_interval_ms = parsed;
        }
    }
    if let Ok(v) = env::var("EXECUTOR_INTERVAL_MS") {
        if let Ok(parsed) = v.parse() {
            settings.executor_interval_ms = parsed;
        }
    }

    if let Ok(v) = env::var("DUAL_LOOP_MODE") {
        settings.dual_loop_mode = v == "1" || v.to_lowercase() == "true";
    }

    settings
}

fn prompt_with_default(label: &str, default: &str) -> anyhow::Result<String> {
    print!("{} [{}]: ", label, default);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let value = input.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

fn prompt_bool(label: &str, default: bool) -> anyhow::Result<bool> {
    let default_str = if default { "y" } else { "n" };
    let input = prompt_with_default(label, default_str)?;
    let normalized = input.to_lowercase();
    Ok(matches!(normalized.as_str(), "y" | "yes" | "true" | "1"))
}

fn prompt_number<T>(label: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr + ToString + Copy,
{
    let input = prompt_with_default(label, &default.to_string())?;
    Ok(input.parse().unwrap_or(default))
}

/// Interactive configuration wizard shared with the GUI config file.
fn run_config_wizard(mut settings: AppSettings) -> anyhow::Result<()> {
    println!("Phone Agent CLI Setup (shared with GUI)");
    if let Some(path) = AppSettings::settings_path() {
        println!("Config file: {}", path.display());
    }
    println!("Press Enter to keep the current value in brackets.\n");

    settings.base_url = prompt_with_default("Model base URL", &settings.base_url)?;
    settings.api_key = prompt_with_default("Model API key", &settings.api_key)?;
    settings.model_name = prompt_with_default("Model name", &settings.model_name)?;
    settings.device_id = prompt_with_default("ADB device ID (optional)", &settings.device_id)?;

    let lang_input = prompt_with_default("Language (cn/en)", &settings.lang)?;
    settings.lang = if lang_input.to_lowercase() == "en" {
        "en".to_string()
    } else {
        "cn".to_string()
    };

    let coord_input = prompt_with_default(
        "Coordinate system (relative/absolute)",
        &settings.coordinate_system,
    )?;
    settings.coordinate_system = match coord_input.to_lowercase().as_str() {
        "absolute" | "abs" => "absolute".to_string(),
        _ => "relative".to_string(),
    };

    if settings.coordinate_system == "absolute" {
        let default_scale = if settings.scale_x == 1.0 && settings.scale_y == 1.0 {
            DEFAULT_COORDINATE_SCALE
        } else {
            settings.scale_x
        };
        settings.scale_x = prompt_number("Coordinate scale X", default_scale)?;
        settings.scale_y = prompt_number("Coordinate scale Y", default_scale)?;
    } else {
        settings.scale_x = 1.0;
        settings.scale_y = 1.0;
        println!("Using relative coordinates: scale fixed to 1.0");
    }

    settings.max_retries = prompt_number("Max retries", settings.max_retries)?;
    settings.retry_delay = prompt_number("Retry delay (seconds)", settings.retry_delay)?;
    settings.max_steps = prompt_number("Max steps", settings.max_steps)?;

    settings.enable_calibration =
        prompt_bool("Enable calibration? (y/n)", settings.enable_calibration)?;
    let mode_input = prompt_with_default(
        "Calibration mode (simple/complex)",
        &settings.calibration_mode,
    )?;
    settings.calibration_mode = if mode_input.to_lowercase() == "complex" {
        "complex".to_string()
    } else {
        "simple".to_string()
    };
    settings.calibration_rounds = prompt_number(
        "Calibration rounds (complex mode)",
        settings.calibration_rounds,
    )?;

    println!("\nPlanner (dual-loop) settings");
    settings.planner_base_url =
        prompt_with_default("Planner model base URL", &settings.planner_base_url)?;
    settings.planner_api_key =
        prompt_with_default("Planner model API key", &settings.planner_api_key)?;
    settings.planner_model_name =
        prompt_with_default("Planner model name", &settings.planner_model_name)?;
    settings.max_executor_feedback_history = prompt_number(
        "Max executor feedback history",
        settings.max_executor_feedback_history,
    )?;
    settings.stuck_threshold = prompt_number("Stuck threshold", settings.stuck_threshold)?;
    settings.prompt_memory_path =
        prompt_with_default("Prompt memory path", &settings.prompt_memory_path)?;
    settings.planner_interval_ms =
        prompt_number("Planner interval (ms)", settings.planner_interval_ms)?;
    settings.executor_interval_ms =
        prompt_number("Executor interval (ms)", settings.executor_interval_ms)?;

    settings.dual_loop_mode = prompt_bool(
        "Enable dual-loop mode by default? (y/n)",
        settings.dual_loop_mode,
    )?;

    settings.save().map_err(|e| anyhow!(e))?;

    println!("\n✅ Settings saved. They will be used by both CLI and GUI.");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (ignore errors if file doesn't exist)
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    // Allow running interactive setup before anything else
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "config" | "--config" | "--setup" | "setup"))
    {
        run_config_wizard(AppSettings::load())?;
        return Ok(());
    }

    // Merge stored settings with environment overrides
    let settings = load_settings_with_env();

    let coordinate_system = match settings.coordinate_system.to_lowercase().as_str() {
        "absolute" | "abs" => CoordinateSystem::Absolute,
        _ => CoordinateSystem::Relative,
    };

    let lang = settings.lang.clone();

    let default_scale = match coordinate_system {
        CoordinateSystem::Relative => 1.0,
        CoordinateSystem::Absolute => DEFAULT_COORDINATE_SCALE,
    };

    let mut scale_x = if settings.scale_x == 0.0 {
        default_scale
    } else {
        settings.scale_x
    };
    let mut scale_y = if settings.scale_y == 0.0 {
        default_scale
    } else {
        settings.scale_y
    };

    // If coordinate system changed from stored value, reset to sensible defaults
    if coordinate_system == CoordinateSystem::Relative {
        scale_x = 1.0;
        scale_y = 1.0;
    } else if coordinate_system == CoordinateSystem::Absolute
        && (settings.scale_x == 1.0 && settings.scale_y == 1.0)
    {
        scale_x = DEFAULT_COORDINATE_SCALE;
        scale_y = DEFAULT_COORDINATE_SCALE;
    }

    // Check if calibration is requested
    let enable_calibration = settings.enable_calibration;
    let calibration_simple = args.iter().any(|arg| arg == "--calibrate");
    let calibration_complex = args.iter().any(|arg| arg == "--calibrate-complex");
    let calibration_only = calibration_simple || calibration_complex;

    // Determine calibration mode
    let calibration_mode = if calibration_complex {
        CalibrationMode::Complex
    } else {
        let mode_env = settings.calibration_mode.to_lowercase();
        if mode_env == "complex" {
            CalibrationMode::Complex
        } else {
            CalibrationMode::Simple
        }
    };

    let complex_rounds: usize = settings.calibration_rounds;

    // Build model config
    let model_config = ModelConfig::default()
        .with_base_url(&settings.base_url)
        .with_api_key(&settings.api_key)
        .with_model_name(&settings.model_name)
        .with_max_retries(settings.max_retries)
        .with_retry_delay(settings.retry_delay);

    // Build agent config with coordinate system
    let mut agent_config = AgentConfig::default()
        .with_lang(&lang)
        .with_coordinate_system(coordinate_system)
        .with_scale(scale_x, scale_y)
        .with_max_steps(settings.max_steps);

    let device_id = if settings.device_id.trim().is_empty() {
        None
    } else {
        Some(settings.device_id.clone())
    };
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
    println!("Model: {} @ {}", settings.model_name, settings.base_url);
    println!("Language: {}", lang);
    println!("Coordinate System: {}", coord_system_name);
    if coordinate_system == CoordinateSystem::Absolute {
        println!("Coordinate Scale: X={:.2}, Y={:.2}", scale_x, scale_y);
    }
    println!(
        "Retry: max {} attempts, {}s delay",
        settings.max_retries, settings.retry_delay
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
    let dual_loop_mode = settings.dual_loop_mode;

    if dual_loop_mode {
        // Dual loop mode
        run_dual_loop_mode(model_config, agent_config, lang.clone(), settings.clone()).await?;
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
    lang: String,
    settings: AppSettings,
) -> anyhow::Result<()> {
    use phone_agent::{DualLoopConfig, DualLoopRunner, PlannerAgent, PlannerConfig};

    println!("\n🔄 Dual Loop Mode Enabled");
    println!("================================================\n");

    // Planner and dual-loop configuration from shared settings
    let planner_base_url = settings.planner_base_url;
    let planner_api_key = settings.planner_api_key;
    let planner_model_name = settings.planner_model_name;

    let max_feedback_history: usize = settings.max_executor_feedback_history;
    let stuck_threshold: u32 = settings.stuck_threshold;
    let prompt_memory_path = settings.prompt_memory_path;
    let planner_interval: u64 = settings.planner_interval_ms;
    let executor_interval: u64 = settings.executor_interval_ms;

    println!(
        "Planner Model: {} @ {}",
        planner_model_name, planner_base_url
    );
    println!("Feedback History: {} entries", max_feedback_history);
    println!("Stuck Threshold: {} consecutive", stuck_threshold);
    println!("Prompt Memory: {}", prompt_memory_path);
    println!(
        "Intervals: Planner={}ms, Executor={}ms",
        planner_interval, executor_interval
    );
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
    let planner = PlannerAgent::new(planner_config, executor_model_config, executor_agent_config);

    // Create dual loop runner
    let loop_config = DualLoopConfig::default()
        .with_planner_interval(planner_interval)
        .with_executor_interval(executor_interval);

    // Track last status to avoid duplicate prints
    use std::sync::{Arc, Mutex};
    let last_status: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let last_status_clone = last_status.clone();

    let runner =
        DualLoopRunner::new(planner, loop_config).with_feedback_callback(move |feedback| {
            // Only print on status change
            let status_str = format!("{:?}", feedback.status);
            let mut last = last_status_clone.lock().unwrap();

            if last.as_ref() != Some(&status_str) {
                // Status changed, print it
                if matches!(
                    feedback.status,
                    phone_agent::ExecutorStatus::Completed
                        | phone_agent::ExecutorStatus::Failed(_)
                        | phone_agent::ExecutorStatus::Stuck
                        | phone_agent::ExecutorStatus::Running
                ) {
                    println!(
                        "📡 Executor: {:?} (step {})",
                        feedback.status, feedback.step_count
                    );
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
