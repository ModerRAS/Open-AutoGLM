# Open-AutoGLM (Rust)

AI-powered agent for automating Android phone interactions - Rust implementation.

[中文版](README_zh.md)

## Overview

This is a Rust rewrite of the [Open-AutoGLM](https://github.com/THUDM/Open-AutoGLM) phone agent. It uses vision-language models to understand screen content and automate Android device interactions via ADB.

## Features

- 🤖 AI-powered phone automation using vision-language models
- 📱 Control Android devices via ADB
- 🔧 Support for various actions: tap, swipe, type, launch apps, etc.
- 🌐 Multi-language support (Chinese & English)
- ⚡ Async/await based architecture
- 🛡️ Type-safe Rust implementation

## Prerequisites

- Rust 1.70 or later
- ADB (Android Debug Bridge) installed and in PATH
- Android device with USB debugging enabled
- [ADB Keyboard](https://github.com/nicnocquee/AdbKeyboard) installed on device (for text input)
- A running OpenAI-compatible API server with the AutoGLM model

## Installation

### From Source

```bash
git clone https://github.com/ModerRAS/Open-AutoGLM.git
cd Open-AutoGLM
cargo build --release
```

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
phone-agent = { git = "https://github.com/ModerRAS/Open-AutoGLM.git" }
```

## Usage

### CLI

```bash
# Set environment variables (optional)
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="EMPTY"
export MODEL_NAME="autoglm-phone-9b"
export AGENT_LANG="cn"  # or "en"
export ADB_DEVICE_ID="your-device-id"  # optional for single device

# Run with a task
cargo run --release -- "打开微信发送消息给张三"

# Or run in interactive mode
cargo run --release
```

### As a Library

```rust
use phone_agent::{AgentConfig, ModelConfig, PhoneAgent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_config = ModelConfig::default()
        .with_base_url("http://localhost:8000/v1");
    
    let agent_config = AgentConfig::default()
        .with_lang("cn")
        .with_max_steps(50);
    
    let mut agent = PhoneAgent::new(model_config, agent_config, None, None);
    
    let result = agent.run("打开微信").await?;
    println!("Result: {}", result);
    
    Ok(())
}
```

## Configuration

### Model Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `base_url` | `http://localhost:8000/v1` | API endpoint |
| `api_key` | `EMPTY` | API key for authentication |
| `model_name` | `autoglm-phone-9b` | Model name |
| `max_tokens` | `3000` | Maximum tokens in response |
| `temperature` | `0.0` | Sampling temperature |

### Agent Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `max_steps` | `100` | Maximum steps before stopping |
| `device_id` | `None` | ADB device ID (optional) |
| `lang` | `cn` | Language for prompts and messages |
| `verbose` | `true` | Print detailed output |

## Project Structure

```
src/
├── lib.rs              # Library entry point
├── main.rs             # CLI entry point
├── agent/              # Core agent logic
│   └── phone_agent.rs  # PhoneAgent implementation
├── actions/            # Action handling
│   └── handler.rs      # Action parser and executor
├── adb/                # ADB utilities
│   ├── connection.rs   # ADB connection management
│   ├── device.rs       # Device control (tap, swipe, etc.)
│   ├── input.rs        # Text input utilities
│   └── screenshot.rs   # Screenshot capture
├── config/             # Configuration
│   ├── apps.rs         # App package mappings
│   ├── i18n.rs         # Internationalization
│   └── prompts.rs      # System prompts
└── model/              # Model client
    └── client.rs       # OpenAI-compatible API client
```

## Supported Actions

| Action | Description |
|--------|-------------|
| `Launch` | Launch an app by name |
| `Tap` | Tap at coordinates |
| `Type` | Input text |
| `Swipe` | Swipe gesture |
| `Back` | Press back button |
| `Home` | Press home button |
| `Long Press` | Long press at coordinates |
| `Double Tap` | Double tap at coordinates |
| `Wait` | Wait for specified duration |
| `Take_over` | Request user intervention |

## Examples

See the `examples/` directory for more usage examples:

```bash
# Basic usage
cargo run --example basic_usage

# Demo thinking process
cargo run --example demo_thinking
```

## License

Apache-2.0 License

## Acknowledgments

- Original Python implementation: [Open-AutoGLM](https://github.com/THUDM/Open-AutoGLM)
- AutoGLM model by THUDM
