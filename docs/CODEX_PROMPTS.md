# Codex 魔改提示词 - 双层 Agent Loop 架构

> 直接复制以下内容给 gpt-5.1-codex-max 使用

---

## 【系统约束提示词 - 必须最先给】

```
你正在修改一个【已经存在】的 Rust 单层 AutoGLM 循环程序。
目标不是重写，而是【在保留现有 Executor loop 的前提下】，
对其进行结构性魔改，升级为"双层 Agent Loop 架构"。

请严格遵守以下约束：

1. 现有 PhoneAgent 循环 = 内层 Executor
   - 不允许删除
   - 不允许整体推倒重写
   - 只能做：抽象、拆分、包裹、加控制接口

2. 新增的外层 Agent（Planner）必须：
   - 拥有独立事件循环
   - 不直接嵌入 Executor loop
   - 通过明确接口与 Executor 通信

3. 请以【最小侵入改造】为目标：
   - 明确指出哪些代码保持不动
   - 明确指出哪些地方需要改造或抽象
   - 不要假设可以随意重构所有模块

4. 输出必须是【可实施的工程改造方案】：
   - 明确模块边界
   - 明确函数/接口责任
   - 避免抽象描述或架构空谈
```

---

## 【Executor 封装提示词】

```
请将当前的单层 PhoneAgent loop 视为：

- 一个「ExecutorLoop」
- 负责连续执行、快速反馈
- 不负责任务规划、纠错决策或用户交互

你的任务是：

1. 从现有代码中识别并标注：
   - Executor 的主循环（PhoneAgent::run 和 execute_step）
   - Executor 的输入入口（task 参数）
   - Executor 的状态输出点（StepResult）

2. 在不破坏其运行逻辑的前提下：
   - 将其封装为一个可控的 ExecutorWrapper 实例
   - 暴露以下控制接口：
     - start_task(task_id, description, system_prompt)
     - pause()
     - resume()
     - inject_prompt(text) 
     - reset_context()
     - get_status() -> ExecutorStatus
     - tick() -> ExecutorFeedback（执行单步并返回反馈）

3. 所有"是否执行/如何执行/是否中断"的决策
   一律上移到新的 Planner Loop 中，
   Executor 内部不得自行做高层决策。

4. 新增卡住检测功能：
   - 计算截图的 hash 值
   - 连续 N 次（默认 3 次）截图 hash 相同则判定为 stuck
   - 上报 stuck 状态给外层
```

---

## 【Planner Loop 提示词】

```
新增的 PlannerAgent Loop 必须满足以下行为特征：

1. Planner 与 Executor 不共享循环
   - 不允许 Planner 阻塞等待 Executor
   - 不允许在 Planner 中直接调用 Executor 的 execute_step

2. Planner 与 Executor 的交互方式只能是：
   - 消息队列（VecDeque<ExecutorCommand>）
   - 明确的控制接口调用
   - 状态回流（ExecutorFeedback）

3. Planner Loop 必须能够：
   - 在 Executor 运行期间接收新的用户输入
   - 将用户输入排队（VecDeque<String>），而不是立即打断 Executor
   - 在合适时机决定是否介入 Executor

4. 请明确指出：
   - Planner Loop 的 tick 触发点
   - Executor Loop 的 tick 触发点
   - 两者之间的数据流方向

5. Planner 需要维护：
   - Todo 任务列表（Vec<TodoItem>）
   - 提示词记忆（HashMap<String, String>，按任务类型存储）
   - 有限的 Executor 反馈历史（VecDeque<ExecutorFeedback>，最多保留 2 条）
```

---

## 【上下文与卡住检测提示词】

```
关于上下文与状态监控，请遵守以下规则：

1. Executor 的执行日志不得无限注入 Planner 上下文
   - 明确限制为最近 N 次（默认 N = 2）
   - 超出部分必须丢弃，而不是总结
   - 使用 VecDeque 并在 push 后检查长度

2. Planner 必须实现「卡住检测」逻辑，至少包含：
   - 连续两次 Executor 状态上报界面无变化（screen_changed == false）
   - Executor 主动上报 ExecutorStatus::Stuck

3. 当判定卡住时，Planner 的可选行为包括：
   - inject_prompt（注入纠偏提示词）
   - pause + inject_prompt
   - reset_context + restart task（重置并重新开始）

请明确写出这些判断发生在 PlannerAgent::supervise_executor() 方法中。
```

---

## 【提示词记忆系统提示词】

```
本项目【明确不使用】向量数据库或语义检索系统。

提示词记忆系统要求：
- 使用 HashMap<String, PromptEntry> 结构
- 按 task_type（任务类型字符串）分类
- 每类任务维护一份 system prompt
- 允许覆盖更新，不需要历史版本管理
- 使用 JSON 文件持久化（serde_json）

PromptEntry 结构：
{
    "system_prompt": "优化后的提示词...",
    "last_updated": "2024-12-23T10:00:00Z"
}

请不要引入 embedding、相似度搜索或外部记忆框架。

任务启动时：
1. 从 PlannerAgent 获取任务的 task_type
2. 查询 PromptMemory.get(task_type)
3. 如果存在，将其作为 system_prompt 传给 ExecutorCommand::StartTask

任务完成后（如果执行曲折）：
1. 收集执行日志
2. 请求 Planner 模型优化提示词
3. 调用 PromptMemory.update(task_type, new_prompt)
4. 保存到 JSON 文件
```

---

## 【数据结构定义提示词】

```
请按以下结构实现核心数据类型：

// === executor.rs ===

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorStatus {
    Idle,
    Running,
    Paused,
    Stuck,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum ExecutorCommand {
    StartTask { task_id: String, description: String, system_prompt: Option<String> },
    Pause,
    Resume,
    InjectPrompt { content: String },
    ResetContext,
    Stop,
}

#[derive(Debug, Clone)]
pub struct ExecutorFeedback {
    pub step_count: u32,
    pub status: ExecutorStatus,
    pub last_result: Option<StepResult>,
    pub screen_changed: bool,
    pub timestamp: u64,
}

// === planner.rs ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TodoStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub description: String,
    pub task_type: String,
    pub status: TodoStatus,
    pub retry_count: u32,
}

// === prompt_memory.rs ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEntry {
    pub system_prompt: String,
    pub last_updated: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptMemory {
    pub prompts: HashMap<String, PromptEntry>,
}
```

---

## 【实现顺序提示词】

```
请按以下顺序实现，每一步完成后可独立测试：

Phase 1: 实现 ExecutorWrapper（src/agent/executor.rs）
- 包装现有 PhoneAgent
- 实现 ExecutorCommand 处理
- 实现 tick() 返回 ExecutorFeedback
- 实现截图 hash 计算和卡住检测

Phase 2: 实现 TodoList 和 PromptMemory（src/agent/todo.rs, src/agent/prompt_memory.rs）
- 独立数据结构
- JSON 序列化/反序列化

Phase 3: 实现 PlannerAgent（src/agent/planner.rs）
- PlannerConfig 配置
- tick() 方法
- supervise_executor() 卡住处理
- build_planner_context() 上下文构建
- 用户输入队列处理

Phase 4: 实现 DualLoopRunner（src/agent/dual_loop.rs）
- 两个异步循环协调
- tokio::select! 或独立 spawn

Phase 5: 更新入口
- 修改 src/agent/mod.rs 导出新模块
- 修改 src/bin/cli.rs 支持双循环模式
```

---

## 【禁止事项提示词】

```
以下是明确禁止的实现方式：

❌ 不要使用向量数据库或 embedding
❌ 不要让 Planner 直接调用 PhoneAgent::execute_step()
❌ 不要让 Planner 阻塞等待 Executor 完成
❌ 不要把所有 Executor 历史塞进 Planner 上下文
❌ 不要删除现有的单循环模式（保留 PhoneAgent::run 兼容）
❌ 不要在 ExecutorWrapper 内部做任务规划决策
❌ 不要使用外部数据库（SQLite、Redis 等）
❌ 不要修改 PhoneAgent::execute_step 的核心逻辑
```

---

## 【一句话总结提示词（可选）】

如果上面太长，可以用这个精简版：

```
我有一个 Rust 写的单层 AutoGLM 执行 loop（PhoneAgent），位于 src/agent/phone_agent.rs。

请帮我做最小侵入改造：
1. 用 ExecutorWrapper 包装 PhoneAgent，暴露 start/pause/inject_prompt/reset 接口
2. 新增独立的 PlannerAgent 作为外层循环，用 DeepSeek 模型
3. 两个循环通过消息队列通信，不共享循环
4. Planner 只保留最近 2 条 Executor 反馈
5. 实现截图 hash 对比检测卡住
6. 提示词按任务类型存 JSON 文件，不用向量数据库

请给我具体的代码实现，明确指出改哪个文件、加什么代码。
```

---

## 【使用方法】

1. **完整版**: 按顺序把上面的提示词分段发给 codex
2. **精简版**: 直接发最后的"一句话总结提示词"
3. **配合架构文档**: 同时附上 `DUAL_LOOP_ARCHITECTURE_SPEC.md` 文件内容

推荐做法：先发【系统约束提示词】，然后把架构文档发过去，最后逐个 Phase 请求实现。
