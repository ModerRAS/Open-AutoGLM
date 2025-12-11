# Open-AutoGLM (Rust)

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
# 坐标缩放因子（LLM输出 × 缩放 = 实际坐标）
COORDINATE_SCALE=1.61
# 或者分别设置X和Y：
# COORDINATE_SCALE_X=1.61
# COORDINATE_SCALE_Y=1.61
EOF

# 方式2：设置环境变量
# Linux/macOS:
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="EMPTY"
export MODEL_NAME="autoglm-phone-9b"
export AGENT_LANG="cn"  # 或 "en"
export ADB_DEVICE_ID="your-device-id"  # 单设备时可选
export COORDINATE_SCALE="1.61"  # 坐标缩放因子

# Windows PowerShell:
$env:MODEL_BASE_URL="http://localhost:8000/v1"
$env:MODEL_API_KEY="EMPTY"
$env:MODEL_NAME="autoglm-phone-9b"
$env:AGENT_LANG="cn"
$env:ADB_DEVICE_ID="your-device-id"
$env:COORDINATE_SCALE="1.61"

# 运行任务
cargo run --release -- "打开微信发送消息给张三"

# 或进入交互模式
cargo run --release
```

### 作为库

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

### 代理配置

| 字段 | 默认值 | 描述 |
|------|--------|------|
| `max_steps` | `100` | 停止前最大步数 |
| `device_id` | `None` | ADB设备ID（可选） |
| `lang` | `cn` | 提示和消息的语言 |
| `verbose` | `true` | 打印详细输出 |
| `scale_x` | `1.61` | X坐标缩放因子 |
| `scale_y` | `1.61` | Y坐标缩放因子 |

### 坐标缩放配置

坐标缩放因子用于将LLM输出的坐标调整为实际屏幕坐标。当模型输出的坐标与实际屏幕像素不一致时，可以使用此功能进行校正。

**计算公式**：`实际坐标 = LLM输出 × 缩放因子`

**环境变量**：
- `COORDINATE_SCALE` - 同时设置X和Y缩放因子（优先级最高）
- `COORDINATE_SCALE_X` - 仅设置X缩放因子
- `COORDINATE_SCALE_Y` - 仅设置Y缩放因子

**示例**（在 `.env` 文件中）：
```bash
# 设置统一缩放因子
COORDINATE_SCALE=1.61

# 或者分别设置X和Y
COORDINATE_SCALE_X=1.61
COORDINATE_SCALE_Y=1.61
```

**作为库使用**：
```rust
let agent_config = AgentConfig::default()
    .with_uniform_scale(1.61)  // X和Y使用相同值
    // 或者
    .with_scale(1.61, 1.61);   // 分别设置X和Y
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
