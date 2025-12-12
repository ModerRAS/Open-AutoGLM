# Open-AutoGLM (Rust)

[![CI](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/ci.yml/badge.svg)](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/ci.yml)
[![Release](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/release.yml/badge.svg)](https://github.com/ModerRAS/Open-AutoGLM/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

AI驱动的Android手机自动化代理 - Rust实现

[English](README.md)

## 概述

这是 [Open-AutoGLM](https://github.com/THUDM/Open-AutoGLM) phone agent 的 Rust 重写版本。它使用视觉语言模型来理解屏幕内容，并通过 ADB 自动化 Android 设备交互。

## 特性

- 🤖 使用视觉语言模型的AI驱动手机自动化
- 📱 通过ADB控制Android设备
- 🔧 支持各种操作：点击、滑动、输入、启动应用等
- 🌐 多语言支持（中文和英文）
- ⚡ 基于async/await的架构
- 🛡️ 类型安全的Rust实现

## 前置要求

- Rust 1.70 或更高版本
- 已安装ADB（Android Debug Bridge）并添加到PATH
- 已启用USB调试的Android设备
- 设备上安装了 [ADB Keyboard](https://github.com/nicnocquee/AdbKeyboard)（用于文本输入）
- 运行中的OpenAI兼容API服务器和AutoGLM模型

## 安装

### 从源码编译

```bash
git clone https://github.com/ModerRAS/Open-AutoGLM.git
cd Open-AutoGLM
cargo build --release
```

### 作为库使用

在 `Cargo.toml` 中添加：

```toml
[dependencies]
phone-agent = { git = "https://github.com/ModerRAS/Open-AutoGLM.git" }
```

## 使用方法

### 命令行

```bash
# 方式1：使用 .env 文件（推荐）
# 在项目根目录创建 .env 文件
cat > .env << EOF
MODEL_BASE_URL=http://localhost:8000/v1
MODEL_API_KEY=EMPTY
MODEL_NAME=autoglm-phone-9b
AGENT_LANG=cn
ADB_DEVICE_ID=your-device-id

# 坐标系统："relative"（0-999相对坐标）或 "absolute"（像素坐标）
# autoglm-phone 模型使用 "relative"，其他模型使用 "absolute"
COORDINATE_SYSTEM=relative

# 坐标缩放因子（仅 absolute 模式使用）
# COORDINATE_SCALE=1.61
EOF

# 方式2：设置环境变量
# Linux/macOS:
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="EMPTY"
export MODEL_NAME="autoglm-phone-9b"
export AGENT_LANG="cn"  # 或 "en"
export ADB_DEVICE_ID="your-device-id"  # 单设备时可选
export COORDINATE_SYSTEM="relative"  # 或 "absolute"

# Windows PowerShell:
$env:MODEL_BASE_URL="http://localhost:8000/v1"
$env:MODEL_API_KEY="EMPTY"
$env:MODEL_NAME="autoglm-phone-9b"
$env:AGENT_LANG="cn"
$env:ADB_DEVICE_ID="your-device-id"
$env:COORDINATE_SYSTEM="relative"

# 运行任务
cargo run --release -- "打开微信发送消息给张三"

# 或进入交互模式
cargo run --release
```

### 作为库

```rust
use phone_agent::{AgentConfig, CoordinateSystem, ModelConfig, PhoneAgent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model_config = ModelConfig::default()
        .with_base_url("http://localhost:8000/v1");
    
    // 使用相对坐标（0-999）- 适用于 autoglm-phone 模型
    let agent_config = AgentConfig::default()
        .with_lang("cn")
        .with_coordinate_system(CoordinateSystem::Relative)
        .with_max_steps(50);
    
    // 或使用简写：
    // let agent_config = AgentConfig::relative().with_lang("cn");
    
    let mut agent = PhoneAgent::new(model_config, agent_config, None, None);
    
    let result = agent.run("打开微信").await?;
    println!("结果: {}", result);
    
    Ok(())
}
```

## 配置

### 模型配置

| 字段 | 默认值 | 描述 |
|------|--------|------|
| `base_url` | `http://localhost:8000/v1` | API端点 |
| `api_key` | `EMPTY` | API认证密钥 |
| `model_name` | `autoglm-phone-9b` | 模型名称 |
| `max_tokens` | `3000` | 响应最大token数 |
| `temperature` | `0.0` | 采样温度 |
| `max_retries` | `3` | 请求失败时的最大重试次数 |
| `retry_delay_secs` | `2` | 重试间隔时间（秒） |

### 代理配置

| 字段 | 默认值 | 描述 |
|------|--------|------|
| `max_steps` | `100` | 停止前最大步数 |
| `device_id` | `None` | ADB设备ID（可选） |
| `lang` | `cn` | 提示和消息的语言 |
| `verbose` | `true` | 打印详细输出 |
| `coordinate_system` | `Absolute` | 坐标系统模式 |
| `scale_x` | `1.61` | X坐标缩放因子（仅 absolute 模式） |
| `scale_y` | `1.61` | Y坐标缩放因子（仅 absolute 模式） |

### 坐标系统配置

代理支持两种坐标系统：

| 模式 | 范围 | 描述 | 适用场景 |
|------|------|------|----------|
| **Relative** | 0-999 | 坐标被归一化到 0-999 范围，自动映射到实际屏幕尺寸 | `autoglm-phone` 模型 |
| **Absolute** | 像素 | 坐标为实际屏幕像素，可选缩放 | 其他视觉模型 |

**环境变量**：
- `COORDINATE_SYSTEM` - 设置为 `relative`（或 `rel`）或 `absolute`（或 `abs`，默认）

**示例**（在 `.env` 文件中）：
```bash
# autoglm-phone 模型（使用 0-999 相对坐标）
COORDINATE_SYSTEM=relative

# 其他模型（使用像素坐标）
COORDINATE_SYSTEM=absolute
COORDINATE_SCALE=1.61
```

**作为库使用**：
```rust
use phone_agent::{AgentConfig, CoordinateSystem};

// 相对坐标（0-999）- 适用于 autoglm-phone 模型
let config = AgentConfig::default()
    .with_coordinate_system(CoordinateSystem::Relative);
// 或使用简写：
let config = AgentConfig::relative();

// 绝对坐标（像素）- 适用于其他模型
let config = AgentConfig::default()
    .with_coordinate_system(CoordinateSystem::Absolute)
    .with_scale(1.61, 1.61);
```

### 请求重试配置

模型客户端会自动重试失败的请求，包括网络错误、超时和服务器错误（5xx、429）。

**环境变量**：
- `MODEL_MAX_RETRIES` - 最大重试次数（默认：3）
- `MODEL_RETRY_DELAY` - 重试间隔秒数（默认：2）

**示例**（在 `.env` 文件中）：
```bash
MODEL_MAX_RETRIES=5
MODEL_RETRY_DELAY=3
```

### 坐标缩放配置（仅 Absolute 模式）

坐标缩放因子用于将LLM输出的坐标调整为实际屏幕坐标。仅当 `COORDINATE_SYSTEM=absolute` 时使用。

**计算公式**：`实际坐标 = LLM输出 × 缩放因子`

**环境变量**：
- `COORDINATE_SCALE` - 同时设置X和Y缩放因子（优先级最高）
- `COORDINATE_SCALE_X` - 仅设置X缩放因子
- `COORDINATE_SCALE_Y` - 仅设置Y缩放因子

**示例**（在 `.env` 文件中）：
```bash
COORDINATE_SYSTEM=absolute

# 设置统一缩放因子
COORDINATE_SCALE=1.61

# 或者分别设置X和Y
COORDINATE_SCALE_X=1.61
COORDINATE_SCALE_Y=1.61
```

**作为库使用**：
```rust
let agent_config = AgentConfig::default()
    .with_coordinate_system(CoordinateSystem::Absolute)
    .with_uniform_scale(1.61)  // X和Y使用相同值
    // 或者
    .with_scale(1.61, 1.61);   // 分别设置X和Y
```

### 自动坐标校准

Phone Agent 内置坐标校准功能，可以通过生成带有标记的测试图片，让 LLM 自动识别坐标位置，从而计算出最佳的坐标缩放因子。

**校准模式**：
- **简单模式** (默认)：使用彩色标记在特定位置 - 快速直接
- **复杂模式**：模拟真实手机 UI 布局（评论区，含用户名、时间、内容、按钮）- 测试 LLM 在真实场景中定位元素的能力

**工作原理**：
1. 从连接的设备截图以检测实际屏幕尺寸
2. 生成带有可视标记的测试图片，标记位于已知像素坐标（匹配屏幕尺寸）
3. 将图片发送给 LLM，询问其标记位置
4. 比较 LLM 报告的坐标与实际坐标
5. 根据期望值/报告值的比率计算缩放因子

**命令行用法**：
```bash
# 仅运行简单校准（输出推荐的缩放因子）
cargo run --release -- --calibrate

# 运行复杂校准（模拟真实UI布局）
cargo run --release -- --calibrate-complex

# 每次启动时自动校准
ENABLE_CALIBRATION=true cargo run --release

# 通过环境变量使用复杂模式
CALIBRATION_MODE=complex ENABLE_CALIBRATION=true cargo run --release

# 调整复杂校准轮数（默认：5）
CALIBRATION_COMPLEX_ROUNDS=10 cargo run --release -- --calibrate-complex
```

**环境变量**：
- `ENABLE_CALIBRATION` - 设置为 `true` 或 `1` 启用启动时校准
- `CALIBRATION_MODE` - 设置为 `simple`（默认）或 `complex`
- `CALIBRATION_COMPLEX_ROUNDS` - 复杂模式的测试轮数（默认：5）

**作为库使用**：
```rust
use phone_agent::calibration::{CalibrationConfig, CalibrationMode, CoordinateCalibrator};
use phone_agent::model::ModelClient;

async fn calibrate(model_client: &ModelClient) -> (f64, f64) {
    // 屏幕尺寸会自动从设备截图中检测
    let config = CalibrationConfig::default()
        .with_mode(CalibrationMode::Complex)  // 使用复杂UI模拟
        .with_complex_rounds(10)               // 10轮校准
        .with_lang("cn")
        .with_device_id("your-device-id");    // 可选
    
    let calibrator = CoordinateCalibrator::new(config);
    let result = calibrator.calibrate(model_client).await;
    
    if result.success {
        println!("模式: {:?}", result.mode);
        println!("屏幕: {}x{}", result.screen_width, result.screen_height);
        (result.scale_x, result.scale_y)
    } else {
        (1.61, 1.61)  // 回退到默认值
    }
}
```

## 项目结构

```
src/
├── lib.rs              # 库入口
├── main.rs             # CLI入口
├── agent/              # 核心代理逻辑
│   └── phone_agent.rs  # PhoneAgent实现
├── actions/            # 动作处理
│   └── handler.rs      # 动作解析和执行器
├── adb/                # ADB工具
│   ├── connection.rs   # ADB连接管理
│   ├── device.rs       # 设备控制（点击、滑动等）
│   ├── input.rs        # 文本输入工具
│   └── screenshot.rs   # 截图捕获
├── calibration/        # 坐标校准
│   └── calibrator.rs   # 自动缩放因子检测
├── config/             # 配置
│   ├── apps.rs         # 应用包名映射
│   ├── i18n.rs         # 国际化
│   └── prompts.rs      # 系统提示词
└── model/              # 模型客户端
    └── client.rs       # OpenAI兼容API客户端
```

## 支持的操作

| 操作 | 描述 |
|------|------|
| `Launch` | 按名称启动应用 |
| `Tap` | 点击坐标 |
| `Type` | 输入文本 |
| `Swipe` | 滑动手势 |
| `Back` | 按返回键 |
| `Home` | 按主页键 |
| `Long Press` | 长按坐标 |
| `Double Tap` | 双击坐标 |
| `Wait` | 等待指定时长 |
| `Take_over` | 请求用户介入 |

## 示例

查看 `examples/` 目录获取更多使用示例：

```bash
# 基本使用
cargo run --example basic_usage

# 演示思考过程
cargo run --example demo_thinking
```

## 许可证

本项目基于 [Apache License 2.0](LICENSE) 许可证发布。

本项目是 [Open-AutoGLM](https://github.com/zai-org/Open-AutoGLM) 的 Rust 重写版本。原始项目由 [Zhipu AI](https://github.com/zai-org) 开发并以 Apache 2.0 许可证开源。

```
Copyright 2025 Zhipu AI (原始 Python 实现)
Copyright 2025 ModerRAS (Rust 实现)

Licensed under the Apache License, Version 2.0
```

## 致谢

- 原始 Python 实现：[zai-org/Open-AutoGLM](https://github.com/zai-org/Open-AutoGLM)
- AutoGLM 模型由 [Zhipu AI](https://www.zhipuai.cn/) 提供

## 引用

如果你觉得这个项目有帮助，请引用原始论文：

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
