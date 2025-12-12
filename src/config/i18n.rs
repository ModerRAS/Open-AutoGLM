//! Internationalization (i18n) module for Phone Agent UI messages.

/// UI messages structure
#[derive(Debug, Clone)]
pub struct Messages {
    pub thinking: &'static str,
    pub action: &'static str,
    pub task_completed: &'static str,
    pub done: &'static str,
    pub starting_task: &'static str,
    pub final_result: &'static str,
    pub task_result: &'static str,
    pub confirmation_required: &'static str,
    pub continue_prompt: &'static str,
    pub manual_operation_required: &'static str,
    pub manual_operation_hint: &'static str,
    pub press_enter_when_done: &'static str,
    pub connection_failed: &'static str,
    pub connection_successful: &'static str,
    pub step: &'static str,
    pub task: &'static str,
    pub result: &'static str,
}

/// Chinese messages
pub static MESSAGES_ZH: Messages = Messages {
    thinking: "思考过程",
    action: "执行动作",
    task_completed: "任务完成",
    done: "完成",
    starting_task: "开始执行任务",
    final_result: "最终结果",
    task_result: "任务结果",
    confirmation_required: "需要确认",
    continue_prompt: "是否继续？(y/n)",
    manual_operation_required: "需要人工操作",
    manual_operation_hint: "请手动完成操作...",
    press_enter_when_done: "完成后按回车继续",
    connection_failed: "连接失败",
    connection_successful: "连接成功",
    step: "步骤",
    task: "任务",
    result: "结果",
};

/// English messages
pub static MESSAGES_EN: Messages = Messages {
    thinking: "Thinking",
    action: "Action",
    task_completed: "Task Completed",
    done: "Done",
    starting_task: "Starting task",
    final_result: "Final Result",
    task_result: "Task Result",
    confirmation_required: "Confirmation Required",
    continue_prompt: "Continue? (y/n)",
    manual_operation_required: "Manual Operation Required",
    manual_operation_hint: "Please complete the operation manually...",
    press_enter_when_done: "Press Enter when done",
    connection_failed: "Connection Failed",
    connection_successful: "Connection Successful",
    step: "Step",
    task: "Task",
    result: "Result",
};

/// Get UI messages by language.
///
/// # Arguments
/// * `lang` - Language code, "cn" for Chinese, "en" for English.
///
/// # Returns
/// Reference to Messages struct.
pub fn get_messages(lang: &str) -> &'static Messages {
    match lang {
        "en" => &MESSAGES_EN,
        _ => &MESSAGES_ZH,
    }
}

/// Get a single UI message by key and language.
///
/// # Arguments
/// * `key` - Message key.
/// * `lang` - Language code, "cn" for Chinese, "en" for English.
///
/// # Returns
/// Message string.
pub fn get_message(key: &str, lang: &str) -> &'static str {
    let messages = get_messages(lang);
    match key {
        "thinking" => messages.thinking,
        "action" => messages.action,
        "task_completed" => messages.task_completed,
        "done" => messages.done,
        "starting_task" => messages.starting_task,
        "final_result" => messages.final_result,
        "task_result" => messages.task_result,
        "confirmation_required" => messages.confirmation_required,
        "continue_prompt" => messages.continue_prompt,
        "manual_operation_required" => messages.manual_operation_required,
        "manual_operation_hint" => messages.manual_operation_hint,
        "press_enter_when_done" => messages.press_enter_when_done,
        "connection_failed" => messages.connection_failed,
        "connection_successful" => messages.connection_successful,
        "step" => messages.step,
        "task" => messages.task,
        "result" => messages.result,
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_messages() {
        let zh = get_messages("cn");
        assert_eq!(zh.thinking, "思考过程");

        let en = get_messages("en");
        assert_eq!(en.thinking, "Thinking");
    }

    #[test]
    fn test_get_message() {
        assert_eq!(get_message("thinking", "cn"), "思考过程");
        assert_eq!(get_message("thinking", "en"), "Thinking");
    }
}
