//! Phone Agent - AI-powered Android phone automation
//!
//! This is the main entry point for the phone-agent CLI tool.

use phone_agent::{AgentConfig, ModelConfig, PhoneAgent};
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

    // Build model config
    let model_config = ModelConfig::default()
        .with_base_url(&base_url)
        .with_api_key(&api_key)
        .with_model_name(&model_name);

    // Build agent config
    let mut agent_config = AgentConfig::default().with_lang(&lang);
    if let Some(id) = device_id {
        agent_config = agent_config.with_device_id(id);
    }

    println!("🤖 Phone Agent - AI-powered Android Automation");
    println!("================================================");
    println!("Model: {} @ {}", model_name, base_url);
    println!("Language: {}", lang);
    if let Some(ref id) = agent_config.device_id {
        println!("Device: {}", id);
    }
    println!("================================================\n");

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
