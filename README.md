# Open-AutoGLM (Rust)

[![CI](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/ci.yml/badge.svg)](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/ci.yml)
[![Release](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/release.yml/badge.svg)](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

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
# Option 1: Use a .env file (recommended)
# Create a .env file in the project root
cat > .env << EOF
MODEL_BASE_URL=http://localhost:8000/v1
MODEL_API_KEY=EMPTY
MODEL_NAME=autoglm-phone-9b
AGENT_LANG=cn
ADB_DEVICE_ID=your-device-id

# Coordinate system: "relative" (0-999) or "absolute" (pixels)
# Use "relative" for autoglm-phone model, "absolute" for other models
COORDINATE_SYSTEM=relative

# Coordinate scale factor (only for absolute mode)
# COORDINATE_SCALE=1.61
EOF

# Option 2: Set environment variables
# Linux/macOS:
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="EMPTY"
export MODEL_NAME="autoglm-phone-9b"
export AGENT_LANG="cn"  # or "en"
export ADB_DEVICE_ID="your-device-id"  # optional for single device
export COORDINATE_SYSTEM="relative"  # or "absolute"

# Windows PowerShell:
$env:MODEL_BASE_URL="http://localhost:8000/v1"
$env:MODEL_API_KEY="EMPTY"
$env:MODEL_NAME="autoglm-phone-9b"
$env:AGENT_LANG="cn"
$env:ADB_DEVICE_ID="your-device-id"
$env:COORDINATE_SYSTEM="relative"

# Run with a task
cargo run --release -- "打开微信发送消息给张三"

# Or run in interactive mode
cargo run --release
```

### As a Library

```rust
use phone_agent::{AgentConfig, CoordinateSystem, ModelConfig, PhoneAgent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_config = ModelConfig::default()
        .with_base_url("http://localhost:8000/v1");
    
    // Use relative coordinates (0-999) for autoglm-phone model
    let agent_config = AgentConfig::default()
        .with_lang("cn")
        .with_coordinate_system(CoordinateSystem::Relative)
        .with_max_steps(50);
    
    // Or use the shorthand:
    // let agent_config = AgentConfig::relative().with_lang("cn");
    
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
| `max_retries` | `3` | Maximum retry attempts for failed requests |
| `retry_delay_secs` | `2` | Delay between retry attempts (seconds) |

### Agent Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `max_steps` | `100` | Maximum steps before stopping |
| `device_id` | `None` | ADB device ID (optional) |
| `lang` | `cn` | Language for prompts and messages |
| `verbose` | `true` | Print detailed output |
| `coordinate_system` | `Absolute` | Coordinate system mode |
| `scale_x` | `1.61` | X coordinate scale factor (absolute mode only) |
| `scale_y` | `1.61` | Y coordinate scale factor (absolute mode only) |

### Coordinate System Configuration

The agent supports two coordinate systems:

| Mode | Range | Description | Use Case |
|------|-------|-------------|----------|
| **Relative** | 0-999 | Coordinates are normalized to 0-999 range, automatically mapped to actual screen size | `autoglm-phone` model |
| **Absolute** | Pixels | Coordinates are actual screen pixels, optionally scaled | Other vision models |

**Environment Variable**:
- `COORDINATE_SYSTEM` - Set to `relative` (or `rel`) or `absolute` (or `abs`, default)

**Example** (in `.env` file):
```bash
# For autoglm-phone model (uses 0-999 relative coordinates)
COORDINATE_SYSTEM=relative

# For other models (uses pixel coordinates)
COORDINATE_SYSTEM=absolute
COORDINATE_SCALE=1.61
```

**As a Library**:
```rust
use phone_agent::{AgentConfig, CoordinateSystem};

// Relative coordinates (0-999) - for autoglm-phone model
let config = AgentConfig::default()
    .with_coordinate_system(CoordinateSystem::Relative);
// Or use shorthand:
let config = AgentConfig::relative();

// Absolute coordinates (pixels) - for other models
let config = AgentConfig::default()
    .with_coordinate_system(CoordinateSystem::Absolute)
    .with_scale(1.61, 1.61);
```

### Retry Configuration

The model client automatically retries failed requests for network errors, timeouts, and server errors (5xx, 429).

**Environment Variables**:
- `MODEL_MAX_RETRIES` - Maximum number of retry attempts (default: 3)
- `MODEL_RETRY_DELAY` - Delay between retries in seconds (default: 2)

**Example** (in `.env` file):
```bash
MODEL_MAX_RETRIES=5
MODEL_RETRY_DELAY=3
```

### Coordinate Scale Configuration (Absolute Mode Only)

The coordinate scale factors are used to adjust LLM output coordinates to actual screen coordinates. This is only used when `COORDINATE_SYSTEM=absolute`.

**Formula**: `actual_coordinate = llm_output × scale_factor`

**Environment Variables**:
- `COORDINATE_SCALE` - Set both X and Y scale factors (takes precedence)
- `COORDINATE_SCALE_X` - Set X scale factor only
- `COORDINATE_SCALE_Y` - Set Y scale factor only

**Example** (in `.env` file):
```bash
COORDINATE_SYSTEM=absolute

# Set uniform scale for both X and Y
COORDINATE_SCALE=1.61

# Or set different scales for X and Y
COORDINATE_SCALE_X=1.61
COORDINATE_SCALE_Y=1.61
```

**As a Library**:
```rust
let agent_config = AgentConfig::default()
    .with_coordinate_system(CoordinateSystem::Absolute)
    .with_uniform_scale(1.61)  // Set both X and Y to 1.61
    // or
    .with_scale(1.61, 1.61);   // Set X=1.61, Y=1.61 separately
```

### Automatic Coordinate Calibration

The phone agent includes a built-in calibration feature that automatically determines the optimal coordinate scale factors by generating test images and asking the LLM to identify marker positions.

**Calibration Modes**:
- **Simple Mode** (default): Uses colored markers at specific positions - fast and straightforward
- **Complex Mode**: Simulates real phone UI layouts (comment sections with usernames, timestamps, content, buttons) - tests LLM's ability to locate elements in realistic scenarios

**How it works**:
1. Takes a screenshot from the connected device to detect actual screen dimensions
2. Generates test images with visual markers at known pixel coordinates (matching screen size)
3. Sends these images to the LLM and asks it to report the marker positions
4. Compares LLM-reported coordinates with actual coordinates
5. Calculates the scale factor from the ratio of expected/reported coordinates

**CLI Usage**:
```bash
# Run simple calibration only (outputs recommended scale factors)
cargo run --release -- --calibrate

# Run complex calibration (simulates real UI layouts)
cargo run --release -- --calibrate-complex

# Enable calibration before each session
ENABLE_CALIBRATION=true cargo run --release

# Use complex mode via environment variable
CALIBRATION_MODE=complex ENABLE_CALIBRATION=true cargo run --release

# Adjust complex calibration rounds (default: 5)
CALIBRATION_COMPLEX_ROUNDS=10 cargo run --release -- --calibrate-complex
```

**Environment Variables**:
- `ENABLE_CALIBRATION` - Set to `true` or `1` to enable calibration at startup
- `CALIBRATION_MODE` - Set to `simple` (default) or `complex`
- `CALIBRATION_COMPLEX_ROUNDS` - Number of test rounds for complex mode (default: 5)

**As a Library**:
```rust
use phone_agent::calibration::{CalibrationConfig, CalibrationMode, CoordinateCalibrator};
use phone_agent::model::ModelClient;

async fn calibrate(model_client: &ModelClient) -> (f64, f64) {
    // Screen size is automatically detected from device screenshot
    let config = CalibrationConfig::default()
        .with_mode(CalibrationMode::Complex)  // Use complex UI simulation
        .with_complex_rounds(10)               // 10 calibration rounds
        .with_lang("cn")
        .with_device_id("your-device-id");    // Optional
    
    let calibrator = CoordinateCalibrator::new(config);
    let result = calibrator.calibrate(model_client).await;
    
    if result.success {
        println!("Mode: {:?}", result.mode);
        println!("Screen: {}x{}", result.screen_width, result.screen_height);
        (result.scale_x, result.scale_y)
    } else {
        (1.61, 1.61)  // fallback to default
    }
}
```

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
├── calibration/        # Coordinate calibration
│   └── calibrator.rs   # Auto scale factor detection
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

This project is licensed under the [Apache License 2.0](LICENSE).

This project is a Rust rewrite of [Open-AutoGLM](https://github.com/zai-org/Open-AutoGLM), originally developed by [Zhipu AI](https://github.com/zai-org) and released under the Apache 2.0 license.

```
Copyright 2025 Zhipu AI (Original Python implementation)
Copyright 2025 ModerRAS (Rust implementation)

Licensed under the Apache License, Version 2.0
```

## Acknowledgments

- Original Python implementation: [zai-org/Open-AutoGLM](https://github.com/zai-org/Open-AutoGLM)
- AutoGLM model by [Zhipu AI](https://www.zhipuai.cn/)

## Citation

If you find this project helpful, please cite the original paper:

```bibtex
@article{liu2024autoglm,
  title={Autoglm: Autonomous foundation agents for guis},
  author={Liu, Xiao and Qin, Bo and Liang, Dongzhu and Dong, Guang and Lai, Hanyu and
Zhang, Hanchen and Zhao, Hanlin and Iong, Iat Long and Sun, Jiadai and Wang, Jiaqi
and others},
  journal={arXiv preprint arXiv:2411.00820},
  year={2024}
}
```
