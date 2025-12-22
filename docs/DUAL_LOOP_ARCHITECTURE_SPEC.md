# 双层 Agent Loop 架构魔改规范

> **目标读者**: AI 代码生成模型（如 gpt-5.1-codex-max）  
> **项目语言**: Rust  
> **现有代码基础**: Open-AutoGLM（单层 PhoneAgent 执行循环）

---

## 一、项目总览

### 1.1 当前状态
项目已有一个可运行的单层 Agent 循环（`PhoneAgent`），位于 `src/agent/phone_agent.rs`。

**现有核心组件**：
- `PhoneAgent`: 主 Agent 结构体
- `PhoneAgent::run()`: 任务主循环
- `PhoneAgent::step()`: 单步执行
- `PhoneAgent::execute_step()`: 内部执行逻辑
- `PhoneAgent::reset()`: 上下文重置
- `StepResult`: 步骤执行结果
- `AgentConfig`: Agent 配置

### 1.2 目标状态
将现有单层循环**魔改**为双层 Agent Loop 架构：

```
┌─────────────────────────────────────────────────────────┐
│              外层 Agent（Planner / Supervisor）          │
│                    DeepSeek Loop                        │
│  职责：                                                 │
│  - 任务规划（拆分为 Todo）                              │
│  - 监督内层执行                                         │
│  - 判断是否介入/纠错                                    │
│  - 用户输入入口                                         │
│  - 提示词优化与记忆管理                                 │
└────────────────────┬────────────────────────────────────┘
                     │ 工具调用 / 消息队列
                     ▼
┌─────────────────────────────────────────────────────────┐
│              内层 Agent（Executor）                      │
│                   AutoGLM Loop                          │
│  职责：                                                 │
│  - 高频连续执行                                         │
│  - 快速 UI/环境操作                                     │
│  - 状态上报                                             │
└─────────────────────────────────────────────────────────┘
```

---

## 二、硬性约束（MUST）

### 2.1 不允许做的事

1. **不允许删除现有 `PhoneAgent` 实现**
2. **不允许推倒重写整个系统**
3. **不允许把 Planner 和 Executor 混成一坨**
4. **不允许使用向量数据库或语义检索系统**
5. **不允许让 Planner 同步阻塞等待 Executor**
6. **不允许在 Planner 中直接调用 Executor 的内部逻辑**

### 2.2 必须做的事

1. **保留现有 `PhoneAgent` 作为内层 Executor**
2. **新增独立的外层 `PlannerAgent`，拥有自己的事件循环**
3. **两个 Loop 通过明确接口通信，不共享循环**
4. **最小侵入改造**：明确哪些代码保持不动，哪些需要抽象

---

## 三、现有代码分析与改造点

### 3.1 现有 `PhoneAgent` 结构（保持不动的部分）

```rust
// 文件: src/agent/phone_agent.rs

pub struct PhoneAgent {
    model_client: ModelClient,      // 保持
    agent_config: AgentConfig,      // 保持
    action_handler: ActionHandler,  // 保持
    context: Vec<Value>,            // 保持
    step_count: u32,                // 保持
}

// 以下方法保持内部逻辑不变：
impl PhoneAgent {
    pub fn new(...) -> Self { ... }           // 保持
    pub async fn step(...) -> Result<...>     // 保持
    pub fn reset(&mut self) { ... }           // 保持
    async fn execute_step(...) -> Result<...> // 保持（核心执行逻辑）
    pub fn context(&self) -> &[Value] { ... } // 保持
    pub fn step_count(&self) -> u32 { ... }   // 保持
}
```

### 3.2 需要改造/新增的部分

#### 3.2.1 将 `PhoneAgent` 封装为可控 Executor

**新增文件**: `src/agent/executor.rs`

需要实现的控制接口：

```rust
/// Executor 状态枚举
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorStatus {
    Idle,           // 空闲
    Running,        // 执行中
    Paused,         // 已暂停
    Stuck,          // 卡住
    Completed,      // 完成
    Failed(String), // 失败
}

/// Executor 包装器
pub struct ExecutorWrapper {
    inner: PhoneAgent,
    status: ExecutorStatus,
    message_queue: VecDeque<ExecutorCommand>,
    last_screen_hash: Option<u64>,  // 用于检测界面无变化
    stuck_count: u32,               // 连续无变化计数
}

/// 外层向内层发送的命令
#[derive(Debug, Clone)]
pub enum ExecutorCommand {
    /// 启动任务
    StartTask {
        task_id: String,
        description: String,
        system_prompt: Option<String>,
    },
    /// 暂停执行
    Pause,
    /// 恢复执行
    Resume,
    /// 注入补充提示词
    InjectPrompt { content: String },
    /// 重置上下文
    ResetContext,
    /// 停止当前任务
    Stop,
}

/// Executor 执行结果回流
#[derive(Debug, Clone)]
pub struct ExecutorFeedback {
    pub step_count: u32,
    pub status: ExecutorStatus,
    pub last_result: Option<StepResult>,
    pub screen_changed: bool,
    pub timestamp: u64,
}

impl ExecutorWrapper {
    /// 创建新的 Executor 包装器
    pub fn new(model_config: ModelConfig, agent_config: AgentConfig) -> Self;
    
    /// 处理消息队列中的下一条命令
    pub async fn process_next_command(&mut self) -> Option<ExecutorFeedback>;
    
    /// 执行单步（内部调用 PhoneAgent::step）
    pub async fn tick(&mut self) -> ExecutorFeedback;
    
    /// 获取当前状态
    pub fn status(&self) -> ExecutorStatus;
    
    /// 入队命令
    pub fn enqueue(&mut self, cmd: ExecutorCommand);
    
    /// 重置
    pub fn reset(&mut self);
    
    /// 判断是否卡住（连续 N 次界面无变化）
    fn detect_stuck(&mut self, current_screen_hash: u64) -> bool;
}
```

#### 3.2.2 新增外层 PlannerAgent

**新增文件**: `src/agent/planner.rs`

```rust
/// Todo 任务状态
#[derive(Debug, Clone, PartialEq)]
pub enum TodoStatus {
    Pending,   // 待执行
    Running,   // 执行中
    Done,      // 已完成
    Failed,    // 失败
}

/// Todo 任务项
#[derive(Debug, Clone)]
pub struct TodoItem {
    pub id: String,
    pub description: String,
    pub task_type: String,       // 用于提示词记忆匹配
    pub status: TodoStatus,
    pub retry_count: u32,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Planner 配置
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// 外层模型配置
    pub model_config: ModelConfig,
    /// 最大保留的 Executor 反馈条数（默认 2）
    pub max_executor_feedback_history: usize,
    /// 卡住检测阈值（连续无变化次数）
    pub stuck_threshold: u32,
    /// 提示词记忆存储路径
    pub prompt_memory_path: Option<String>,
}

/// 提示词记忆（按任务类型存储）
#[derive(Debug, Clone, Default)]
pub struct PromptMemory {
    /// task_type -> optimized_system_prompt
    prompts: HashMap<String, String>,
}

impl PromptMemory {
    pub fn load(path: &str) -> Self;
    pub fn save(&self, path: &str);
    pub fn get(&self, task_type: &str) -> Option<&str>;
    pub fn update(&mut self, task_type: &str, prompt: String);
}

/// 外层 Planner Agent
pub struct PlannerAgent {
    model_client: ModelClient,
    config: PlannerConfig,
    
    /// Todo 列表
    todo_list: Vec<TodoItem>,
    
    /// Executor 实例
    executor: ExecutorWrapper,
    
    /// Executor 反馈历史（限制长度）
    executor_feedback_history: VecDeque<ExecutorFeedback>,
    
    /// 用户输入队列
    user_input_queue: VecDeque<String>,
    
    /// Planner 自身上下文
    context: Vec<Value>,
    
    /// 提示词记忆
    prompt_memory: PromptMemory,
}

impl PlannerAgent {
    pub fn new(
        planner_config: PlannerConfig,
        executor_model_config: ModelConfig,
        executor_agent_config: AgentConfig,
    ) -> Self;
    
    /// 添加用户输入到队列
    pub fn queue_user_input(&mut self, input: String);
    
    /// Planner 主循环单次 tick
    /// 返回是否需要继续
    pub async fn tick(&mut self) -> bool;
    
    /// 处理用户输入，更新 Todo 列表
    async fn process_user_input(&mut self);
    
    /// 监督 Executor，决定是否介入
    async fn supervise_executor(&mut self);
    
    /// 检测卡住并尝试纠偏
    async fn handle_stuck(&mut self);
    
    /// 优化提示词并保存
    async fn optimize_prompt(&mut self, task_type: &str, execution_log: &str);
    
    /// 获取当前任务的系统提示词（从记忆中选择）
    fn get_system_prompt_for_task(&self, task_type: &str) -> Option<String>;
    
    /// 向 Executor 发送命令
    fn send_to_executor(&mut self, cmd: ExecutorCommand);
    
    /// 收集 Executor 反馈（自动限制历史长度）
    fn collect_executor_feedback(&mut self, feedback: ExecutorFeedback);
    
    /// 清理过期的 Executor 反馈
    fn prune_executor_feedback(&mut self);
    
    /// 构建给 Planner 模型的上下文（包含有限的 Executor 信息）
    fn build_planner_context(&self) -> Vec<Value>;
}
```

#### 3.2.3 新增双循环运行器

**新增文件**: `src/agent/dual_loop.rs`

```rust
/// 双循环运行器
pub struct DualLoopRunner {
    planner: PlannerAgent,
    
    /// Planner tick 间隔（毫秒）
    planner_interval_ms: u64,
    
    /// Executor tick 间隔（毫秒）
    executor_interval_ms: u64,
    
    /// 是否正在运行
    running: Arc<AtomicBool>,
}

impl DualLoopRunner {
    pub fn new(
        planner: PlannerAgent,
        planner_interval_ms: u64,
        executor_interval_ms: u64,
    ) -> Self;
    
    /// 启动双循环（两个循环独立运行）
    pub async fn run(&mut self);
    
    /// 停止运行
    pub fn stop(&self);
    
    /// 添加用户输入
    pub fn add_user_input(&mut self, input: String);
}
```

---

## 四、模块文件结构

```
src/agent/
├── mod.rs              # 更新：导出新模块
├── phone_agent.rs      # 保持：现有 PhoneAgent（内层 Executor 核心）
├── executor.rs         # 新增：Executor 包装器
├── planner.rs          # 新增：外层 Planner Agent
├── dual_loop.rs        # 新增：双循环运行器
├── todo.rs             # 新增：Todo 任务管理
└── prompt_memory.rs    # 新增：提示词记忆系统
```

**更新 `src/agent/mod.rs`**:

```rust
//! Agent module for orchestrating phone automation.

mod phone_agent;
mod executor;
mod planner;
mod dual_loop;
mod todo;
mod prompt_memory;

pub use phone_agent::{AgentConfig, PhoneAgent, StepResult};
pub use executor::{ExecutorWrapper, ExecutorCommand, ExecutorFeedback, ExecutorStatus};
pub use planner::{PlannerAgent, PlannerConfig, TodoItem, TodoStatus};
pub use dual_loop::DualLoopRunner;
pub use todo::TodoList;
pub use prompt_memory::PromptMemory;
```

---

## 五、通信协议详细定义

### 5.1 Planner → Executor 命令格式

```rust
/// 启动任务
ExecutorCommand::StartTask {
    task_id: "task_001".to_string(),
    description: "打开微信并发送消息给张三".to_string(),
    system_prompt: Some("优化后的系统提示词...".to_string()),
}

/// 暂停
ExecutorCommand::Pause

/// 注入提示词（用于纠偏）
ExecutorCommand::InjectPrompt {
    content: "当前操作似乎卡住了，请尝试其他方法。如果按钮无法点击，请尝试滑动屏幕。".to_string(),
}

/// 重置上下文
ExecutorCommand::ResetContext
```

### 5.2 Executor → Planner 反馈格式

```rust
ExecutorFeedback {
    step_count: 5,
    status: ExecutorStatus::Running,
    last_result: Some(StepResult {
        success: true,
        finished: false,
        action: Some(json!({"action": "tap", "x": 500, "y": 300})),
        thinking: "检测到微信图标，准备点击...".to_string(),
        message: None,
    }),
    screen_changed: true,
    timestamp: 1703260800,
}
```

---

## 六、上下文管理策略

### 6.1 Executor 反馈历史限制

```rust
impl PlannerAgent {
    fn collect_executor_feedback(&mut self, feedback: ExecutorFeedback) {
        self.executor_feedback_history.push_back(feedback);
        
        // 硬性限制：只保留最近 N 条
        while self.executor_feedback_history.len() > self.config.max_executor_feedback_history {
            self.executor_feedback_history.pop_front();
        }
    }
}
```

### 6.2 Planner 上下文构建

```rust
impl PlannerAgent {
    fn build_planner_context(&self) -> Vec<Value> {
        let mut context = self.context.clone();
        
        // 只附加最近 N 条 Executor 反馈
        if !self.executor_feedback_history.is_empty() {
            let summary = self.summarize_executor_feedback();
            context.push(MessageBuilder::create_system_message(&format!(
                "[Executor 近期状态]\n{}",
                summary
            )));
        }
        
        context
    }
}
```

---

## 七、卡住检测与纠偏机制

### 7.1 检测逻辑

```rust
impl ExecutorWrapper {
    fn detect_stuck(&mut self, current_screen_hash: u64) -> bool {
        if let Some(last_hash) = self.last_screen_hash {
            if last_hash == current_screen_hash {
                self.stuck_count += 1;
            } else {
                self.stuck_count = 0;
            }
        }
        self.last_screen_hash = Some(current_screen_hash);
        
        // 阈值判定
        self.stuck_count >= STUCK_THRESHOLD
    }
}
```

### 7.2 Planner 纠偏流程

```rust
impl PlannerAgent {
    async fn handle_stuck(&mut self) {
        // 1. 生成纠偏提示词
        let correction_prompt = self.generate_correction_prompt().await;
        
        // 2. 注入提示词
        self.send_to_executor(ExecutorCommand::InjectPrompt {
            content: correction_prompt,
        });
        
        // 3. 如果多次纠偏失败，考虑重置
        if self.consecutive_stuck_count > MAX_STUCK_RETRIES {
            self.send_to_executor(ExecutorCommand::ResetContext);
            self.send_to_executor(ExecutorCommand::StartTask {
                task_id: self.current_task_id(),
                description: self.current_task_description(),
                system_prompt: self.get_enhanced_system_prompt(),
            });
        }
    }
}
```

---

## 八、提示词记忆系统

### 8.1 存储结构（JSON 文件，不用数据库）

```json
{
  "task_types": {
    "微信操作": {
      "system_prompt": "你是一个手机操作助手，专门处理微信相关任务...",
      "last_updated": "2024-12-23T10:00:00Z",
      "success_rate": 0.85
    },
    "设置调整": {
      "system_prompt": "你是一个手机操作助手，专门处理系统设置...",
      "last_updated": "2024-12-22T15:30:00Z",
      "success_rate": 0.92
    }
  }
}
```

### 8.2 自动优化流程

```rust
impl PlannerAgent {
    async fn optimize_prompt(&mut self, task_type: &str, execution_log: &str) {
        // 1. 构建优化请求
        let optimization_context = format!(
            "任务类型: {}\n执行日志:\n{}\n\n请根据执行过程中的问题，优化系统提示词。",
            task_type, execution_log
        );
        
        // 2. 请求 Planner 模型生成优化后的提示词
        let optimized_prompt = self.model_client
            .request_prompt_optimization(&optimization_context)
            .await;
        
        // 3. 保存到记忆
        if let Ok(prompt) = optimized_prompt {
            self.prompt_memory.update(task_type, prompt);
            self.prompt_memory.save(&self.config.prompt_memory_path);
        }
    }
}
```

---

## 九、CLI 入口修改

**修改文件**: `src/bin/cli.rs`

新增双循环模式启动选项：

```rust
// 检测是否启用双循环模式
let dual_loop_mode = env::var("DUAL_LOOP_MODE")
    .map(|v| v == "1" || v.to_lowercase() == "true")
    .unwrap_or(false);

if dual_loop_mode {
    // 双循环模式
    let planner_config = PlannerConfig {
        model_config: ModelConfig::default()
            .with_base_url(&planner_base_url)
            .with_model_name(&planner_model_name),
        max_executor_feedback_history: 2,
        stuck_threshold: 3,
        prompt_memory_path: Some("prompt_memory.json".to_string()),
    };
    
    let planner = PlannerAgent::new(
        planner_config,
        model_config.clone(),  // Executor 用的模型配置
        agent_config.clone(),
    );
    
    let mut runner = DualLoopRunner::new(planner, 2000, 500);
    
    // 异步添加用户输入
    let runner_handle = runner.clone_handle();
    tokio::spawn(async move {
        // 用户输入循环
        loop {
            let input = read_user_input().await;
            runner_handle.add_user_input(input);
        }
    });
    
    runner.run().await;
} else {
    // 原有单循环模式（保持不变）
    let mut agent = PhoneAgent::new(model_config, agent_config, None, None);
    // ...
}
```

---

## 十、环境变量配置

```bash
# 双循环模式开关
DUAL_LOOP_MODE=true

# 外层 Planner 模型配置
PLANNER_MODEL_BASE_URL=https://api.deepseek.com/v1
PLANNER_MODEL_API_KEY=your-deepseek-key
PLANNER_MODEL_NAME=deepseek-chat

# 内层 Executor 模型配置（现有配置）
MODEL_BASE_URL=http://localhost:8000/v1
MODEL_API_KEY=EMPTY
MODEL_NAME=autoglm-phone-9b

# 上下文管理
MAX_EXECUTOR_FEEDBACK_HISTORY=2
STUCK_THRESHOLD=3

# 提示词记忆
PROMPT_MEMORY_PATH=./prompt_memory.json

# 循环间隔（毫秒）
PLANNER_INTERVAL_MS=2000
EXECUTOR_INTERVAL_MS=500
```

---

## 十一、实现顺序建议

按以下顺序实现，确保每一步可独立测试：

1. **Phase 1**: 实现 `ExecutorWrapper`
   - 封装现有 `PhoneAgent`
   - 实现控制接口（pause/resume/inject/reset）
   - 实现状态上报
   - 实现卡住检测

2. **Phase 2**: 实现 `TodoList` 和 `PromptMemory`
   - 独立的数据结构
   - JSON 文件读写

3. **Phase 3**: 实现 `PlannerAgent`
   - 独立事件循环
   - 与 Executor 通信
   - Todo 管理
   - 上下文限制

4. **Phase 4**: 实现 `DualLoopRunner`
   - 两个循环的协调
   - 用户输入队列

5. **Phase 5**: 更新 CLI
   - 新增双循环模式
   - 环境变量支持

---

## 十二、测试要点

每个 Phase 完成后需要验证：

1. **Phase 1 测试**:
   - `ExecutorWrapper` 可以正常包装 `PhoneAgent` 执行任务
   - `pause()`/`resume()` 正常工作
   - `inject_prompt()` 能正确修改上下文
   - 卡住检测在连续相同截图时触发

2. **Phase 2 测试**:
   - Todo 列表的 CRUD 操作
   - PromptMemory 的 JSON 持久化

3. **Phase 3 测试**:
   - Planner 能独立运行
   - 能正确收集和裁剪 Executor 反馈
   - 能在卡住时自动注入提示词

4. **Phase 4 测试**:
   - 两个循环独立运行，互不阻塞
   - 用户输入能异步进入 Planner 队列

---

## 十三、不要做的事（显式声明）

- ❌ 不要引入向量数据库
- ❌ 不要做复杂语义检索
- ❌ 不要让 Planner 直接调用 `execute_step()`
- ❌ 不要把所有 Executor 历史塞进 Planner 上下文
- ❌ 不要删除现有的单循环模式（保留兼容）
- ❌ 不要在 Executor 内部做高层决策

---

## 十四、总结

这是一个**最小侵入式改造**方案：

- 现有 `PhoneAgent` **完全保留**，只是被 `ExecutorWrapper` 包了一层
- 新增 `PlannerAgent` 作为独立的外层循环
- 两者通过**消息队列**和**状态回流**通信
- 上下文严格限制，不追求全量命中
- 提示词记忆按任务类型存储，不用向量数据库

改造完成后，系统支持：
1. 原有单循环模式（完全兼容）
2. 新的双循环模式（通过环境变量切换）
