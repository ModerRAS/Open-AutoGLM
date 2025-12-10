//! Basic usage example for Phone Agent.

use phone_agent::{AgentConfig, ModelConfig, PhoneAgent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    // Configure the model client
    // You can customize these settings based on your setup
    let model_config = ModelConfig::default()
        .with_base_url("http://localhost:8000/v1")
        .with_model_name("autoglm-phone-9b");

    // Configure the agent
    let agent_config = AgentConfig::default()
        .with_lang("cn")        // Use Chinese prompts
        .with_max_steps(50);    // Maximum 50 steps

    // Create the agent
    let mut agent = PhoneAgent::new(model_config, agent_config, None, None);

    // Run a simple task
    println!("🤖 Starting Phone Agent...\n");
    
    let task = "打开微信";
    println!("📝 Task: {}\n", task);

    match agent.run(task).await {
        Ok(result) => {
            println!("\n✅ Task completed: {}", result);
        }
        Err(e) => {
            eprintln!("\n❌ Task failed: {}", e);
        }
    }

    Ok(())
}
