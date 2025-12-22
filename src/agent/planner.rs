//! Planner agent for the outer loop in dual-loop architecture.
//!
//! The Planner is responsible for:
//! - Task planning and todo list management
//! - Supervising the Executor
//! - Detecting stuck situations and intervening
//! - Managing prompt memory
//! - Handling user input

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::executor::{ExecutorCommand, ExecutorFeedback, ExecutorStatus, ExecutorWrapper};
use super::prompt_memory::PromptMemory;
use super::todo::{TodoList, TodoStatus};
use crate::agent::AgentConfig;
use crate::model::{MessageBuilder, ModelClient, ModelConfig};

/// Configuration for the Planner agent.
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// Model configuration for the Planner (e.g., DeepSeek).
    pub model_config: ModelConfig,
    /// Maximum number of Executor feedback entries to keep in history.
    pub max_executor_feedback_history: usize,
    /// Stuck detection threshold (consecutive unchanged screens).
    pub stuck_threshold: u32,
    /// Path to prompt memory JSON file.
    pub prompt_memory_path: Option<String>,
    /// Maximum retries for stuck situations before giving up.
    pub max_stuck_retries: u32,
    /// Whether to auto-optimize prompts after task completion.
    pub auto_optimize_prompts: bool,
    /// System prompt for the Planner model.
    pub system_prompt: Option<String>,
    /// Language for prompts ("cn" or "en").
    pub lang: String,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            model_config: ModelConfig::default(),
            max_executor_feedback_history: 2,
            stuck_threshold: 3,
            prompt_memory_path: Some("prompt_memory.json".to_string()),
            max_stuck_retries: 3,
            auto_optimize_prompts: true,
            system_prompt: None,
            lang: "cn".to_string(),
        }
    }
}

impl PlannerConfig {
    /// Set the model configuration.
    pub fn with_model_config(mut self, config: ModelConfig) -> Self {
        self.model_config = config;
        self
    }

    /// Set the max executor feedback history.
    pub fn with_max_feedback_history(mut self, max: usize) -> Self {
        self.max_executor_feedback_history = max;
        self
    }

    /// Set the stuck threshold.
    pub fn with_stuck_threshold(mut self, threshold: u32) -> Self {
        self.stuck_threshold = threshold;
        self
    }

    /// Set the prompt memory path.
    pub fn with_prompt_memory_path(mut self, path: impl Into<String>) -> Self {
        self.prompt_memory_path = Some(path.into());
        self
    }

    /// Set the system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set the language.
    pub fn with_lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = lang.into();
        self
    }

    /// Get the default system prompt for Planner.
    pub fn get_system_prompt(&self) -> String {
        self.system_prompt.clone().unwrap_or_else(|| {
            if self.lang == "cn" {
                DEFAULT_PLANNER_SYSTEM_PROMPT_CN.to_string()
            } else {
                DEFAULT_PLANNER_SYSTEM_PROMPT_EN.to_string()
            }
        })
    }
}

/// Default Planner system prompt (Chinese).
pub const DEFAULT_PLANNER_SYSTEM_PROMPT_CN: &str = r#"你是一个手机自动化任务规划和监督助手。你的职责是将用户需求拆分成子任务，监督执行，并在出问题时介入。

## 工具调用格式

你必须使用 JSON 格式调用工具。每次回复只能调用一个工具。格式如下：

### 添加任务
```json
{"action": "add_todo", "description": "任务描述", "task_type": "任务类型"}
```
task_type 可选值: "微信操作", "小红书操作", "抖音操作", "系统操作", "通用任务"

### 启动执行器
```json
{"action": "start_executor", "task_id": "任务ID"}
```
task_id 是 add_todo 后自动生成的，格式为 "task_1", "task_2" 等。

### 暂停/恢复执行器
```json
{"action": "pause_executor"}
{"action": "resume_executor"}
```

### 注入提示词（纠偏）
```json
{"action": "inject_prompt", "content": "提示内容"}
```

### 重置执行器
```json
{"action": "reset_executor"}
```

### 标记任务完成/失败
```json
{"action": "complete_todo", "task_id": "任务ID"}
{"action": "fail_todo", "task_id": "任务ID", "reason": "失败原因"}
```

### 汇报进度（不执行动作）
```json
{"action": "report", "message": "汇报内容"}
```

### 等待
```json
{"action": "wait"}
```

## 工作流程

1. 收到用户请求后，先用 add_todo 添加所有子任务
2. 然后用 start_executor 启动第一个任务
3. 监控执行反馈，必要时用 inject_prompt 纠偏
4. 任务完成后会自动执行下一个

## 重要规则

- 每次回复只输出一个 JSON 工具调用
- 先简要说明你的想法，然后输出 JSON
- 不要输出代码块，直接输出 JSON 对象
- 收到用户请求时，先添加第一个任务，等系统确认后再添加下一个或启动执行"#;

/// Default Planner system prompt (English).
pub const DEFAULT_PLANNER_SYSTEM_PROMPT_EN: &str = r#"You are a phone automation task planning and supervision assistant. Your job is to break down user requests into sub-tasks, supervise execution, and intervene when needed.

## Tool Call Format

You must use JSON format to call tools. Only call one tool per response. Format:

### Add Task
```json
{"action": "add_todo", "description": "Task description", "task_type": "task_type"}
```
task_type options: "wechat", "xiaohongshu", "douyin", "system", "general"

### Start Executor
```json
{"action": "start_executor", "task_id": "task_id"}
```
task_id is auto-generated after add_todo, format: "task_1", "task_2", etc.

### Pause/Resume Executor
```json
{"action": "pause_executor"}
{"action": "resume_executor"}
```

### Inject Prompt (correction)
```json
{"action": "inject_prompt", "content": "prompt content"}
```

### Reset Executor
```json
{"action": "reset_executor"}
```

### Mark Task Complete/Failed
```json
{"action": "complete_todo", "task_id": "task_id"}
{"action": "fail_todo", "task_id": "task_id", "reason": "failure reason"}
```

### Report Progress (no action)
```json
{"action": "report", "message": "report content"}
```

### Wait
```json
{"action": "wait"}
```

## Workflow

1. After receiving user request, use add_todo to add all sub-tasks
2. Then use start_executor to start the first task
3. Monitor execution feedback, use inject_prompt to correct if needed
4. Next task starts automatically after completion

## Important Rules

- Output only one JSON tool call per response
- Briefly explain your thinking, then output JSON
- Do not use code blocks, output JSON object directly
- When receiving user request, first add the first task, wait for confirmation, then add next or start execution"#;

/// Planner action types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PlannerAction {
    /// Add a new todo item.
    AddTodo {
        description: String,
        task_type: String,
    },
    /// Start the executor on a task.
    StartExecutor {
        task_id: String,
    },
    /// Pause the executor.
    PauseExecutor,
    /// Resume the executor.
    ResumeExecutor,
    /// Inject a prompt into executor.
    InjectPrompt {
        content: String,
    },
    /// Reset executor context.
    ResetExecutor,
    /// Mark a todo as complete.
    CompleteTodo {
        task_id: String,
    },
    /// Mark a todo as failed.
    FailTodo {
        task_id: String,
        reason: String,
    },
    /// Report to user (no action, just message).
    Report {
        message: String,
    },
    /// Wait for more information.
    Wait,
    /// Task planning complete.
    Done {
        message: String,
    },
}

/// Planner agent for the outer loop.
pub struct PlannerAgent {
    /// Model client for Planner.
    model_client: ModelClient,
    /// Planner configuration.
    config: PlannerConfig,
    /// Todo task list.
    todo_list: TodoList,
    /// Executor instance.
    executor: ExecutorWrapper,
    /// Executor model config (for recreating).
    executor_model_config: ModelConfig,
    /// Executor agent config (for recreating).
    executor_agent_config: AgentConfig,
    /// Limited history of Executor feedback.
    executor_feedback_history: VecDeque<ExecutorFeedback>,
    /// User input queue.
    user_input_queue: VecDeque<String>,
    /// Planner's own conversation context.
    context: Vec<Value>,
    /// Prompt memory for optimized prompts.
    prompt_memory: PromptMemory,
    /// Consecutive stuck count (for multi-stuck handling).
    consecutive_stuck_count: u32,
    /// Execution log for prompt optimization.
    execution_log: Vec<String>,
    /// Whether the planner is running.
    is_running: bool,
}

impl PlannerAgent {
    /// Create a new PlannerAgent.
    pub fn new(
        planner_config: PlannerConfig,
        executor_model_config: ModelConfig,
        executor_agent_config: AgentConfig,
    ) -> Self {
        let model_client = ModelClient::new(planner_config.model_config.clone());

        // Load prompt memory if path specified
        let prompt_memory = planner_config
            .prompt_memory_path
            .as_ref()
            .and_then(|path| PromptMemory::load(path).ok())
            .unwrap_or_default();

        let executor = ExecutorWrapper::new(
            executor_model_config.clone(),
            executor_agent_config.clone(),
        )
        .with_stuck_threshold(planner_config.stuck_threshold);

        Self {
            model_client,
            config: planner_config,
            todo_list: TodoList::new(),
            executor,
            executor_model_config,
            executor_agent_config,
            executor_feedback_history: VecDeque::new(),
            user_input_queue: VecDeque::new(),
            context: Vec::new(),
            prompt_memory,
            consecutive_stuck_count: 0,
            execution_log: Vec::new(),
            is_running: false,
        }
    }

    /// Queue user input for processing.
    pub fn queue_user_input(&mut self, input: String) {
        self.user_input_queue.push_back(input);
    }

    /// Check if there's pending user input.
    pub fn has_pending_input(&self) -> bool {
        !self.user_input_queue.is_empty()
    }

    /// Get the todo list.
    pub fn todo_list(&self) -> &TodoList {
        &self.todo_list
    }

    /// Get mutable todo list.
    pub fn todo_list_mut(&mut self) -> &mut TodoList {
        &mut self.todo_list
    }

    /// Get executor status.
    pub fn executor_status(&self) -> &ExecutorStatus {
        self.executor.status()
    }

    /// Check if planner is running.
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Start the planner.
    pub fn start(&mut self) {
        self.is_running = true;
        self.initialize_context();
    }

    /// Stop the planner.
    pub fn stop(&mut self) {
        self.is_running = false;
        self.executor.enqueue(ExecutorCommand::Stop);
    }

    /// Initialize planner context with system prompt.
    fn initialize_context(&mut self) {
        self.context.clear();
        self.context.push(MessageBuilder::create_system_message(
            &self.config.get_system_prompt(),
        ));
    }

    /// Execute one tick of the Executor loop.
    /// Call this in the Executor's dedicated loop.
    pub async fn tick_executor(&mut self) -> ExecutorFeedback {
        let feedback = self.executor.tick().await;
        self.collect_executor_feedback(feedback.clone());
        feedback
    }

    /// Execute one tick of the Planner loop.
    /// Returns true if the planner should continue running.
    pub async fn tick_planner(&mut self) -> bool {
        if !self.is_running {
            return false;
        }

        // 1. Process any pending user input
        self.process_user_input().await;

        // 2. Supervise executor and decide actions
        self.supervise_executor().await;

        // 3. Check if we should continue
        !self.todo_list.is_all_done() || self.has_pending_input()
    }

    /// Process pending user input.
    async fn process_user_input(&mut self) {
        while let Some(input) = self.user_input_queue.pop_front() {
            println!("\n🧠 [Planner] Processing user input: {}", input);
            tracing::info!("Processing user input: {}", input);

            // Add user message to context
            self.context.push(MessageBuilder::create_user_message(&input, None));

            // Continue conversation until planner stops adding tasks or starts executor
            self.continue_planner_conversation().await;
        }
    }

    /// Continue the planner conversation loop until a stopping condition.
    /// This allows the planner to add multiple tasks and then start execution.
    async fn continue_planner_conversation(&mut self) {
        let max_turns = 20; // Safety limit
        let mut turns = 0;

        loop {
            turns += 1;
            if turns > max_turns {
                println!("⚠️ [System] 达到最大对话轮数限制");
                break;
            }

            // Get planner's response
            match self.get_planner_response().await {
                Some((response_text, action)) => {
                    // Print DeepSeek's response to terminal
                    println!("\n💬 [DeepSeek Response]:");
                    println!("{}", response_text);
                    println!();

                    // Execute the parsed action
                    if let Some(action) = action {
                        println!("📋 [Action]: {:?}", action);
                        
                        // Check if this is a terminal action that should stop the loop
                        let should_continue = self.should_continue_after_action(&action);
                        
                        self.execute_planner_action(action).await;
                        
                        if !should_continue {
                            break;
                        }
                        
                        // After executing action, prompt the model to continue
                        // (The feedback was already added in execute_planner_action)
                    } else {
                        // No valid action parsed, stop
                        println!("⚠️ [System] 未能解析有效动作");
                        break;
                    }
                }
                None => {
                    println!("❌ [Planner] Failed to get response from model");
                    break;
                }
            }
        }
    }

    /// Determine if the conversation should continue after an action.
    fn should_continue_after_action(&self, action: &PlannerAction) -> bool {
        match action {
            // After adding a task, continue to add more or start executor
            PlannerAction::AddTodo { .. } => true,
            // After starting executor, stop the conversation loop
            PlannerAction::StartExecutor { .. } => false,
            // Report and wait can continue
            PlannerAction::Report { .. } => true,
            PlannerAction::Wait => false,
            // Done means planning is complete
            PlannerAction::Done { .. } => false,
            // Other actions stop the loop
            _ => false,
        }
    }

    /// Get planner's response and parse action.
    /// Returns (raw_response, parsed_action).
    async fn get_planner_response(&mut self) -> Option<(String, Option<PlannerAction>)> {
        // Call planner model
        match self.model_client.request(&self.context).await {
            Ok(response) => {
                let response_text = response.raw_content.clone();

                // Add assistant response to context
                self.context.push(MessageBuilder::create_assistant_message(&response.action));

                // Parse action from response
                let action = self.parse_planner_action(&response.action);
                Some((response_text, action))
            }
            Err(e) => {
                println!("❌ [Planner] Model error: {}", e);
                tracing::error!("Planner model error: {}", e);
                None
            }
        }
    }

    /// Supervise executor and handle stuck/completion.
    async fn supervise_executor(&mut self) {
        // Check latest executor feedback - clone to avoid borrow issues
        let feedback_info = self.executor_feedback_history.back().map(|f| {
            (f.status.clone(), f.clone())
        });

        if let Some((status, _feedback)) = feedback_info {
            match status {
                ExecutorStatus::Stuck => {
                    self.handle_stuck().await;
                }
                ExecutorStatus::Completed => {
                    self.handle_executor_completed().await;
                }
                ExecutorStatus::Failed(reason) => {
                    self.handle_executor_failed(reason).await;
                }
                _ => {
                    // Running, Paused, or Idle - nothing special to do
                    self.consecutive_stuck_count = 0;
                }
            }
        }
    }

    /// Handle stuck executor.
    async fn handle_stuck(&mut self) {
        self.consecutive_stuck_count += 1;
        tracing::warn!(
            "Executor stuck (consecutive count: {})",
            self.consecutive_stuck_count
        );

        // Log for prompt optimization
        self.execution_log.push(format!(
            "[STUCK] Consecutive stuck count: {}",
            self.consecutive_stuck_count
        ));

        if self.consecutive_stuck_count >= self.config.max_stuck_retries {
            // Too many stuck attempts, consider resetting
            tracing::warn!("Max stuck retries reached, resetting executor");
            self.executor.enqueue(ExecutorCommand::ResetContext);

            // Mark current task as needing retry
            if let Some(task) = self.todo_list.current_running() {
                let task_id = task.id.clone();
                if let Some(task) = self.todo_list.get_mut(&task_id) {
                    if !task.retry() {
                        tracing::error!("Task {} failed after max retries", task_id);
                    }
                }
            }

            self.consecutive_stuck_count = 0;
        } else {
            // Generate correction prompt
            let correction = self.generate_correction_prompt().await;
            self.executor.enqueue(ExecutorCommand::InjectPrompt {
                content: correction,
            });
        }
    }

    /// Handle executor completion.
    async fn handle_executor_completed(&mut self) {
        tracing::info!("Executor completed task");

        // Mark current todo as done
        if let Some(task) = self.todo_list.current_running() {
            let task_id = task.id.clone();
            let task_type = task.task_type.clone();

            if let Some(task) = self.todo_list.get_mut(&task_id) {
                task.complete();
            }

            // Record success in prompt memory
            self.prompt_memory.record_usage(&task_type, true);

            // Save prompt memory
            if let Some(path) = &self.config.prompt_memory_path {
                let _ = self.prompt_memory.save(path);
            }
        }

        // Check if there are more tasks
        if let Some(next_task) = self.todo_list.next_pending() {
            let task_id = next_task.id.clone();
            self.start_task(&task_id);
        }
    }

    /// Handle executor failure.
    async fn handle_executor_failed(&mut self, reason: String) {
        tracing::error!("Executor failed: {}", reason);

        // Log for prompt optimization
        self.execution_log.push(format!("[FAILED] {}", reason));

        // Mark current todo as failed or retry
        if let Some(task) = self.todo_list.current_running() {
            let task_id = task.id.clone();
            let task_type = task.task_type.clone();

            if let Some(task) = self.todo_list.get_mut(&task_id) {
                if task.can_retry() {
                    task.retry();
                    // Retry the task
                    self.start_task(&task_id);
                } else {
                    task.fail(&reason);

                    // Record failure in prompt memory
                    self.prompt_memory.record_usage(&task_type, false);

                    // Try to optimize prompt if enabled
                    if self.config.auto_optimize_prompts {
                        self.optimize_prompt(&task_type).await;
                    }

                    // Move to next task
                    if let Some(next_task) = self.todo_list.next_pending() {
                        let next_id = next_task.id.clone();
                        self.start_task(&next_id);
                    }
                }
            }
        }
    }

    /// Start executing a task.
    fn start_task(&mut self, task_id: &str) {
        if let Some(task) = self.todo_list.get_mut(task_id) {
            task.start();

            // Get system prompt from memory if available
            let system_prompt = self.prompt_memory.get_prompt(&task.task_type).map(|s| s.to_string());

            self.executor.enqueue(ExecutorCommand::StartTask {
                task_id: task.id.clone(),
                description: task.description.clone(),
                system_prompt,
            });

            tracing::info!("Started task: {} - {}", task.id, task.description);
        }
    }

    /// Generate a correction prompt for stuck situations.
    async fn generate_correction_prompt(&self) -> String {
        // Build context about what happened
        let recent_feedback: Vec<_> = self.executor_feedback_history.iter().collect();

        let feedback_summary = recent_feedback
            .iter()
            .map(|f| {
                format!(
                    "Step {}: status={:?}, screen_changed={}",
                    f.step_count, f.status, f.screen_changed
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Simple correction prompt (could be enhanced with model call)
        if self.config.lang == "cn" {
            format!(
                "执行似乎卡住了。最近状态：\n{}\n\n请尝试不同的方法：\n\
                1. 如果按钮无法点击，尝试滑动屏幕\n\
                2. 如果页面没有响应，尝试返回上一级\n\
                3. 如果操作位置不正确，仔细观察界面元素位置\n\
                请继续执行任务。",
                feedback_summary
            )
        } else {
            format!(
                "Execution seems stuck. Recent status:\n{}\n\n\
                Please try a different approach:\n\
                1. If button won't click, try scrolling\n\
                2. If page not responding, try going back\n\
                3. If position incorrect, carefully observe element positions\n\
                Please continue the task.",
                feedback_summary
            )
        }
    }

    /// Optimize prompt based on execution log.
    async fn optimize_prompt(&mut self, task_type: &str) {
        if self.execution_log.is_empty() {
            return;
        }

        let log_summary = self.execution_log.join("\n");

        // Build optimization request
        let request = if self.config.lang == "cn" {
            format!(
                "任务类型: {}\n\n执行日志:\n{}\n\n\
                请根据执行中遇到的问题，生成一个优化后的系统提示词，\
                帮助未来执行同类任务时避免这些问题。\
                只输出优化后的提示词内容，不要其他解释。",
                task_type, log_summary
            )
        } else {
            format!(
                "Task type: {}\n\nExecution log:\n{}\n\n\
                Based on the issues encountered, generate an optimized system prompt \
                to help avoid these problems in future executions of similar tasks.\
                Only output the optimized prompt content, no explanations.",
                task_type, log_summary
            )
        };

        // Request optimization from planner model
        let messages = vec![
            MessageBuilder::create_system_message("You are a prompt optimization assistant."),
            MessageBuilder::create_user_message(&request, None),
        ];

        if let Ok(response) = self.model_client.request(&messages).await {
            let optimized_prompt = response.action.trim().to_string();
            if !optimized_prompt.is_empty() {
                self.prompt_memory.update(task_type, &optimized_prompt);
                if let Some(path) = &self.config.prompt_memory_path {
                    let _ = self.prompt_memory.save(path);
                }
                tracing::info!("Optimized prompt for task type: {}", task_type);
            }
        }

        // Clear execution log
        self.execution_log.clear();
    }

    /// Get planner's decision based on current context.
    /// Used by supervise_executor (no need to print response again).
    async fn get_planner_decision(&mut self) -> Option<PlannerAction> {
        // Add executor status summary to context
        let status_summary = self.build_executor_status_summary();
        let todo_summary = self.build_todo_summary();

        let context_update = format!(
            "[当前状态]\n{}\n\n[任务列表]\n{}\n\n请决定下一步操作。",
            status_summary, todo_summary
        );

        // Add as a system message update
        self.context.push(MessageBuilder::create_user_message(&context_update, None));

        // Call planner model
        match self.model_client.request(&self.context).await {
            Ok(response) => {
                // Print response for debugging
                println!("\n🧠 [Planner Supervision Response]:");
                println!("{}", response.raw_content);
                println!();

                // Add assistant response to context
                self.context.push(MessageBuilder::create_assistant_message(&response.action));

                // Parse action from response
                self.parse_planner_action(&response.action)
            }
            Err(e) => {
                println!("❌ [Planner] Model error: {}", e);
                tracing::error!("Planner model error: {}", e);
                None
            }
        }
    }

    /// Build executor status summary.
    fn build_executor_status_summary(&self) -> String {
        let status = self.executor.status();
        let step_count = self.executor.step_count();

        let mut summary = format!(
            "Executor状态: {:?}\n步骤数: {}\n",
            status, step_count
        );

        // Add recent feedback
        if !self.executor_feedback_history.is_empty() {
            summary.push_str("\n最近反馈:\n");
            for (i, feedback) in self.executor_feedback_history.iter().enumerate() {
                summary.push_str(&format!(
                    "  {}. step={}, changed={}, status={:?}\n",
                    i + 1,
                    feedback.step_count,
                    feedback.screen_changed,
                    feedback.status
                ));
                if let Some(ref result) = feedback.last_result {
                    if let Some(ref msg) = result.message {
                        summary.push_str(&format!("     message: {}\n", msg));
                    }
                }
            }
        }

        summary
    }

    /// Build todo list summary.
    fn build_todo_summary(&self) -> String {
        let stats = self.todo_list.stats();
        let mut summary = format!(
            "总任务: {} | 待执行: {} | 执行中: {} | 完成: {} | 失败: {}\n\n",
            stats.total, stats.pending, stats.running, stats.done, stats.failed
        );

        for item in self.todo_list.items() {
            let status_icon = match item.status {
                TodoStatus::Pending => "⏳",
                TodoStatus::Running => "🔄",
                TodoStatus::Done => "✅",
                TodoStatus::Failed => "❌",
                TodoStatus::Skipped => "⏭️",
            };
            summary.push_str(&format!(
                "{} [{}] {} (类型: {})\n",
                status_icon, item.id, item.description, item.task_type
            ));
        }

        summary
    }

    /// Parse planner action from model response.
    fn parse_planner_action(&self, response: &str) -> Option<PlannerAction> {
        // Try to parse as JSON directly
        if let Ok(action) = serde_json::from_str::<PlannerAction>(response) {
            return Some(action);
        }

        // Try to extract JSON from code block (```json ... ``` or ``` ... ```)
        let json_str = self.extract_json_from_response(response);
        if let Some(json) = json_str {
            if let Ok(action) = serde_json::from_str::<PlannerAction>(&json) {
                return Some(action);
            }
        }

        // Try to find bare JSON object in the response
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                let json_str = &response[start..=end];
                if let Ok(action) = serde_json::from_str::<PlannerAction>(json_str) {
                    return Some(action);
                }
            }
        }

        // Simple keyword-based parsing as fallback
        let lower = response.to_lowercase();
        if lower.contains("wait") || lower.contains("等待") {
            Some(PlannerAction::Wait)
        } else if lower.contains("done") || lower.contains("完成") {
            Some(PlannerAction::Done {
                message: response.to_string(),
            })
        } else {
            Some(PlannerAction::Report {
                message: response.to_string(),
            })
        }
    }

    /// Extract JSON from markdown code blocks or bare JSON.
    fn extract_json_from_response(&self, response: &str) -> Option<String> {
        // Pattern 1: ```json\n{...}\n```
        if let Some(start) = response.find("```json") {
            let after_marker = &response[start + 7..];
            if let Some(end) = after_marker.find("```") {
                let json_part = after_marker[..end].trim();
                return Some(json_part.to_string());
            }
        }

        // Pattern 2: ```\n{...}\n```
        if let Some(start) = response.find("```\n{") {
            let after_marker = &response[start + 4..];
            if let Some(end) = after_marker.find("```") {
                let json_part = after_marker[..end].trim();
                return Some(json_part.to_string());
            }
        }

        // Pattern 3: ```{...}```
        if let Some(start) = response.find("```{") {
            let after_marker = &response[start + 3..];
            if let Some(end) = after_marker.find("```") {
                let json_part = after_marker[..end].trim();
                return Some(json_part.to_string());
            }
        }

        None
    }

    /// Execute a planner action.
    async fn execute_planner_action(&mut self, action: PlannerAction) {
        match action {
            PlannerAction::AddTodo { description, task_type } => {
                let task_id = self.todo_list.add(&description, &task_type);
                println!("✅ [System] 已添加任务: {} (ID: {}, 类型: {})", description, task_id, task_type);
                tracing::info!("Added todo: {} (id: {}, type: {})", description, task_id, task_type);
                
                // Add system feedback with clear next step instructions
                let feedback = format!(
                    "[系统反馈] 任务已添加成功。\n\
                    - ID: {}\n\
                    - 描述: {}\n\
                    - 类型: {}\n\n\
                    当前任务列表:\n{}\n\n\
                    请继续: 如果还有更多子任务，请继续使用 add_todo 添加。\
                    如果任务列表已完整，请使用 start_executor 启动执行第一个任务。",
                    task_id, description, task_type, self.build_todo_summary()
                );
                self.context.push(MessageBuilder::create_user_message(&feedback, None));
            }
            PlannerAction::StartExecutor { task_id } => {
                println!("🚀 [System] 启动执行器，任务ID: {}", task_id);
                self.start_task(&task_id);
                
                // Add system feedback
                let feedback = format!(
                    "[系统反馈] 执行器已启动，正在执行任务: {}\n\
                    执行器将自动运行，完成后会自动执行下一个任务。",
                    task_id
                );
                self.context.push(MessageBuilder::create_user_message(&feedback, None));
            }
            PlannerAction::PauseExecutor => {
                println!("⏸️ [System] 暂停执行器");
                self.executor.enqueue(ExecutorCommand::Pause);
            }
            PlannerAction::ResumeExecutor => {
                println!("▶️ [System] 恢复执行器");
                self.executor.enqueue(ExecutorCommand::Resume);
            }
            PlannerAction::InjectPrompt { content } => {
                println!("💉 [System] 注入提示词: {}", content);
                self.executor.enqueue(ExecutorCommand::InjectPrompt { content });
            }
            PlannerAction::ResetExecutor => {
                println!("🔄 [System] 重置执行器");
                self.executor.enqueue(ExecutorCommand::ResetContext);
            }
            PlannerAction::CompleteTodo { task_id } => {
                if let Some(task) = self.todo_list.get_mut(&task_id) {
                    task.complete();
                    println!("✅ [System] 任务完成: {}", task_id);
                }
            }
            PlannerAction::FailTodo { task_id, reason } => {
                if let Some(task) = self.todo_list.get_mut(&task_id) {
                    task.fail(&reason);
                    println!("❌ [System] 任务失败: {} - {}", task_id, reason);
                }
            }
            PlannerAction::Report { message } => {
                println!("📢 [Planner] {}", message);
                tracing::info!("Planner report: {}", message);
            }
            PlannerAction::Wait => {
                println!("⏳ [Planner] 等待中...");
            }
            PlannerAction::Done { message } => {
                println!("🎉 [Planner] 规划完成: {}", message);
                tracing::info!("Planner done: {}", message);
            }
        }
    }

    /// Collect executor feedback with history limit.
    fn collect_executor_feedback(&mut self, feedback: ExecutorFeedback) {
        self.executor_feedback_history.push_back(feedback);

        // Enforce history limit
        while self.executor_feedback_history.len() > self.config.max_executor_feedback_history {
            self.executor_feedback_history.pop_front();
        }
    }

    /// Get the prompt memory.
    pub fn prompt_memory(&self) -> &PromptMemory {
        &self.prompt_memory
    }

    /// Get mutable prompt memory.
    pub fn prompt_memory_mut(&mut self) -> &mut PromptMemory {
        &mut self.prompt_memory
    }

    /// Save prompt memory to disk.
    pub fn save_prompt_memory(&self) -> Result<(), crate::agent::prompt_memory::PromptMemoryError> {
        if let Some(path) = &self.config.prompt_memory_path {
            self.prompt_memory.save(path)
        } else {
            Ok(())
        }
    }

    /// Get executor feedback history.
    pub fn feedback_history(&self) -> &VecDeque<ExecutorFeedback> {
        &self.executor_feedback_history
    }

    /// Clear feedback history.
    pub fn clear_feedback_history(&mut self) {
        self.executor_feedback_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_config_default() {
        let config = PlannerConfig::default();
        assert_eq!(config.max_executor_feedback_history, 2);
        assert_eq!(config.stuck_threshold, 3);
        assert_eq!(config.lang, "cn");
    }

    #[test]
    fn test_planner_config_builder() {
        let config = PlannerConfig::default()
            .with_max_feedback_history(5)
            .with_stuck_threshold(4)
            .with_lang("en");

        assert_eq!(config.max_executor_feedback_history, 5);
        assert_eq!(config.stuck_threshold, 4);
        assert_eq!(config.lang, "en");
    }

    #[test]
    fn test_parse_planner_action_json() {
        let planner_config = PlannerConfig::default();
        let executor_model_config = ModelConfig::default();
        let executor_agent_config = AgentConfig::default();
        let planner = PlannerAgent::new(planner_config, executor_model_config, executor_agent_config);

        let json = r#"{"action": "add_todo", "description": "Test task", "task_type": "general"}"#;
        let action = planner.parse_planner_action(json);

        assert!(matches!(action, Some(PlannerAction::AddTodo { .. })));
    }
}
