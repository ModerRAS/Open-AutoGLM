//! Demo showing the thinking process of the agent.
//!
//! This example demonstrates step-by-step execution to observe
//! the agent's reasoning process.

use phone_agent::{AgentConfig, ModelConfig, PhoneAgent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Configure with verbose output
    let model_config = ModelConfig::default().with_base_url("http://localhost:8000/v1");

    let agent_config = AgentConfig::default().with_lang("cn").with_max_steps(10); // Limit steps for demo

    let mut agent = PhoneAgent::new(model_config, agent_config, None, None);

    println!("🤖 Phone Agent - Thinking Process Demo");
    println!("======================================\n");

    let task = "打开设置应用，查看Wi-Fi状态";
    println!("📝 Task: {}\n", task);

    // Execute step by step
    println!("Executing step by step to observe thinking process...\n");

    // First step with task
    let result = agent.step(Some(task)).await?;
    println!("Step 1 completed:");
    println!("  - Success: {}", result.success);
    println!("  - Finished: {}", result.finished);
    println!(
        "  - Thinking: {}",
        &result.thinking[..result.thinking.len().min(100)]
    );
    if result.finished {
        println!("  - Message: {:?}", result.message);
        return Ok(());
    }

    // Continue with more steps
    for step in 2..=5 {
        if agent.step_count() >= 10 {
            println!("\nMax steps reached for demo.");
            break;
        }

        let result = agent.step(None).await?;
        println!("\nStep {} completed:", step);
        println!("  - Success: {}", result.success);
        println!("  - Finished: {}", result.finished);
        println!(
            "  - Thinking preview: {}...",
            &result.thinking[..result.thinking.len().min(80)]
        );

        if result.finished {
            println!("  - Final message: {:?}", result.message);
            break;
        }

        // Small delay between steps
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    println!("\n======================================");
    println!("Demo completed. Total steps: {}", agent.step_count());

    Ok(())
}
