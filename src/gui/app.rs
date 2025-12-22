//! Main Iced application for Phone Agent GUI.

use iced::widget::{
    button, column, container, horizontal_rule, horizontal_space, pick_list, row, scrollable,
    text, text_input, toggler, vertical_space,
};
use iced::{Element, Length, Task, Theme};

use crate::calibration::{CalibrationConfig, CalibrationMode, CoordinateCalibrator};
use crate::model::ModelClient;
use crate::{AgentConfig, CoordinateSystem, ModelConfig, PhoneAgent, StepResult};

use super::logger::Logger;
use super::settings::AppSettings;

/// Current view/tab of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Main,
    Settings,
    Logs,
}

/// Language options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Chinese,
    English,
}

impl Language {
    fn as_code(&self) -> &'static str {
        match self {
            Language::Chinese => "cn",
            Language::English => "en",
        }
    }

    fn from_code(code: &str) -> Self {
        match code {
            "en" => Language::English,
            _ => Language::Chinese,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Chinese => write!(f, "中文"),
            Language::English => write!(f, "English"),
        }
    }
}

/// Coordinate system options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordSystemOption {
    Relative,
    Absolute,
}

impl CoordSystemOption {
    fn as_str(&self) -> &'static str {
        match self {
            CoordSystemOption::Relative => "relative",
            CoordSystemOption::Absolute => "absolute",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "absolute" | "abs" => CoordSystemOption::Absolute,
            _ => CoordSystemOption::Relative,
        }
    }

    fn as_coordinate_system(self) -> CoordinateSystem {
        match self {
            CoordSystemOption::Relative => CoordinateSystem::Relative,
            CoordSystemOption::Absolute => CoordinateSystem::Absolute,
        }
    }
}

impl std::fmt::Display for CoordSystemOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordSystemOption::Relative => write!(f, "相对坐标 (0-999)"),
            CoordSystemOption::Absolute => write!(f, "绝对坐标 (像素)"),
        }
    }
}

/// Calibration mode options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibModeOption {
    Simple,
    Complex,
}

impl CalibModeOption {
    fn as_str(&self) -> &'static str {
        match self {
            CalibModeOption::Simple => "simple",
            CalibModeOption::Complex => "complex",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "complex" => CalibModeOption::Complex,
            _ => CalibModeOption::Simple,
        }
    }

    fn as_calibration_mode(self) -> CalibrationMode {
        match self {
            CalibModeOption::Simple => CalibrationMode::Simple,
            CalibModeOption::Complex => CalibrationMode::Complex,
        }
    }
}

impl std::fmt::Display for CalibModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalibModeOption::Simple => write!(f, "简单模式"),
            CalibModeOption::Complex => write!(f, "复杂模式"),
        }
    }
}

/// Application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppState {
    #[default]
    Idle,
    Running,
    Calibrating,
}

/// Messages for the Iced application.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    SwitchView(View),

    // Settings - Model
    BaseUrlChanged(String),
    ApiKeyChanged(String),
    ModelNameChanged(String),

    // Settings - Device
    DeviceIdChanged(String),
    LanguageSelected(Language),

    // Settings - Coordinates
    CoordSystemSelected(CoordSystemOption),
    ScaleXChanged(String),
    ScaleYChanged(String),

    // Settings - Retry
    MaxRetriesChanged(String),
    RetryDelayChanged(String),

    // Settings - Agent
    MaxStepsChanged(String),
    EnableCalibrationToggled(bool),
    CalibModeSelected(CalibModeOption),
    CalibRoundsChanged(String),

    // Settings actions
    SaveSettings,
    ResetSettings,
    SettingsSaved(Result<(), String>),

    // Task execution
    TaskInputChanged(String),
    RunTask,
    StopTask,
    TaskStep(StepResult),
    TaskCompleted(Result<String, String>),

    // Calibration
    RunCalibration,
    CalibrationCompleted(Result<(f64, f64), String>),

    // Logs
    ClearLogs,
}

/// Main application struct.
pub struct PhoneAgentApp {
    // Current view
    view: View,

    // Settings
    settings: AppSettings,

    // Parsed settings for pick_list
    language: Language,
    coord_system: CoordSystemOption,
    calib_mode: CalibModeOption,

    // Input fields as strings
    scale_x_input: String,
    scale_y_input: String,
    max_retries_input: String,
    retry_delay_input: String,
    max_steps_input: String,
    calib_rounds_input: String,

    // Task input
    task_input: String,

    // Application state
    state: AppState,

    // Logger
    logger: Logger,

    // Status message
    status: String,
}

impl Default for PhoneAgentApp {
    fn default() -> Self {
        Self::new()
    }
}

impl PhoneAgentApp {
    /// Create a new application instance.
    pub fn new() -> Self {
        let settings = AppSettings::load();
        let mut logger = Logger::new();
        logger.info("Phone Agent GUI 启动");

        Self {
            view: View::Main,
            language: Language::from_code(&settings.lang),
            coord_system: CoordSystemOption::from_str(&settings.coordinate_system),
            calib_mode: CalibModeOption::from_str(&settings.calibration_mode),
            scale_x_input: settings.scale_x.to_string(),
            scale_y_input: settings.scale_y.to_string(),
            max_retries_input: settings.max_retries.to_string(),
            retry_delay_input: settings.retry_delay.to_string(),
            max_steps_input: settings.max_steps.to_string(),
            calib_rounds_input: settings.calibration_rounds.to_string(),
            settings,
            task_input: String::new(),
            state: AppState::Idle,
            logger,
            status: "就绪".to_string(),
        }
    }

    /// Get the window title.
    pub fn title(&self) -> String {
        "Phone Agent - AI 手机自动化".to_string()
    }

    /// Get the theme.
    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    /// Update the application state based on messages.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Navigation
            Message::SwitchView(view) => {
                self.view = view;
                Task::none()
            }

            // Settings - Model
            Message::BaseUrlChanged(value) => {
                self.settings.base_url = value;
                Task::none()
            }
            Message::ApiKeyChanged(value) => {
                self.settings.api_key = value;
                Task::none()
            }
            Message::ModelNameChanged(value) => {
                self.settings.model_name = value;
                Task::none()
            }

            // Settings - Device
            Message::DeviceIdChanged(value) => {
                self.settings.device_id = value;
                Task::none()
            }
            Message::LanguageSelected(lang) => {
                self.language = lang;
                self.settings.lang = lang.as_code().to_string();
                Task::none()
            }

            // Settings - Coordinates
            Message::CoordSystemSelected(coord) => {
                self.coord_system = coord;
                self.settings.coordinate_system = coord.as_str().to_string();
                // Update default scale values
                if coord == CoordSystemOption::Relative {
                    self.settings.scale_x = 1.0;
                    self.settings.scale_y = 1.0;
                    self.scale_x_input = "1.0".to_string();
                    self.scale_y_input = "1.0".to_string();
                } else {
                    self.settings.scale_x = 1.61;
                    self.settings.scale_y = 1.61;
                    self.scale_x_input = "1.61".to_string();
                    self.scale_y_input = "1.61".to_string();
                }
                Task::none()
            }
            Message::ScaleXChanged(value) => {
                self.scale_x_input = value.clone();
                if let Ok(v) = value.parse() {
                    self.settings.scale_x = v;
                }
                Task::none()
            }
            Message::ScaleYChanged(value) => {
                self.scale_y_input = value.clone();
                if let Ok(v) = value.parse() {
                    self.settings.scale_y = v;
                }
                Task::none()
            }

            // Settings - Retry
            Message::MaxRetriesChanged(value) => {
                self.max_retries_input = value.clone();
                if let Ok(v) = value.parse() {
                    self.settings.max_retries = v;
                }
                Task::none()
            }
            Message::RetryDelayChanged(value) => {
                self.retry_delay_input = value.clone();
                if let Ok(v) = value.parse() {
                    self.settings.retry_delay = v;
                }
                Task::none()
            }

            // Settings - Agent
            Message::MaxStepsChanged(value) => {
                self.max_steps_input = value.clone();
                if let Ok(v) = value.parse() {
                    self.settings.max_steps = v;
                }
                Task::none()
            }
            Message::EnableCalibrationToggled(enabled) => {
                self.settings.enable_calibration = enabled;
                Task::none()
            }
            Message::CalibModeSelected(mode) => {
                self.calib_mode = mode;
                self.settings.calibration_mode = mode.as_str().to_string();
                Task::none()
            }
            Message::CalibRoundsChanged(value) => {
                self.calib_rounds_input = value.clone();
                if let Ok(v) = value.parse() {
                    self.settings.calibration_rounds = v;
                }
                Task::none()
            }

            // Settings actions
            Message::SaveSettings => {
                let settings = self.settings.clone();
                Task::perform(
                    async move { settings.save() },
                    Message::SettingsSaved,
                )
            }
            Message::ResetSettings => {
                self.settings = AppSettings::default();
                self.language = Language::Chinese;
                self.coord_system = CoordSystemOption::Relative;
                self.calib_mode = CalibModeOption::Simple;
                self.scale_x_input = self.settings.scale_x.to_string();
                self.scale_y_input = self.settings.scale_y.to_string();
                self.max_retries_input = self.settings.max_retries.to_string();
                self.retry_delay_input = self.settings.retry_delay.to_string();
                self.max_steps_input = self.settings.max_steps.to_string();
                self.calib_rounds_input = self.settings.calibration_rounds.to_string();
                self.logger.info("设置已重置为默认值");
                Task::none()
            }
            Message::SettingsSaved(result) => {
                match result {
                    Ok(()) => {
                        self.logger.success("设置已保存");
                        self.status = "设置已保存".to_string();
                    }
                    Err(e) => {
                        self.logger.error(format!("保存设置失败: {}", e));
                        self.status = format!("保存失败: {}", e);
                    }
                }
                Task::none()
            }

            // Task execution
            Message::TaskInputChanged(value) => {
                self.task_input = value;
                Task::none()
            }
            Message::RunTask => {
                if self.task_input.trim().is_empty() {
                    self.logger.warning("请输入任务");
                    return Task::none();
                }

                self.state = AppState::Running;
                self.logger.info(format!("开始执行任务: {}", self.task_input));
                self.status = "正在执行...".to_string();

                let settings = self.settings.clone();
                let task = self.task_input.clone();

                Task::perform(
                    async move { run_agent_task(settings, task).await },
                    Message::TaskCompleted,
                )
            }
            Message::StopTask => {
                self.state = AppState::Idle;
                self.logger.warning("任务已停止");
                self.status = "已停止".to_string();
                Task::none()
            }
            Message::TaskStep(step) => {
                let action_str = step.action
                    .as_ref()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "无".to_string());
                self.logger.action(format!("动作: {}", action_str));
                if !step.thinking.is_empty() {
                    self.logger.thinking(step.thinking);
                }
                Task::none()
            }
            Message::TaskCompleted(result) => {
                self.state = AppState::Idle;
                match result {
                    Ok(result) => {
                        self.logger.success(format!("任务完成: {}", result));
                        self.status = "任务完成".to_string();
                    }
                    Err(e) => {
                        self.logger.error(format!("任务失败: {}", e));
                        self.status = format!("失败: {}", e);
                    }
                }
                Task::none()
            }

            // Calibration
            Message::RunCalibration => {
                self.state = AppState::Calibrating;
                self.logger.info("开始坐标校准...");
                self.status = "正在校准...".to_string();

                let settings = self.settings.clone();

                Task::perform(
                    async move { run_calibration(settings).await },
                    Message::CalibrationCompleted,
                )
            }
            Message::CalibrationCompleted(result) => {
                self.state = AppState::Idle;
                match result {
                    Ok((scale_x, scale_y)) => {
                        self.settings.scale_x = scale_x;
                        self.settings.scale_y = scale_y;
                        self.scale_x_input = format!("{:.4}", scale_x);
                        self.scale_y_input = format!("{:.4}", scale_y);
                        self.logger.success(format!(
                            "校准完成: X={:.4}, Y={:.4}",
                            scale_x, scale_y
                        ));
                        self.status = "校准完成".to_string();
                    }
                    Err(e) => {
                        self.logger.error(format!("校准失败: {}", e));
                        self.status = format!("校准失败: {}", e);
                    }
                }
                Task::none()
            }

            // Logs
            Message::ClearLogs => {
                self.logger.clear();
                self.logger.info("日志已清空");
                Task::none()
            }
        }
    }

    /// Build the view.
    pub fn view(&self) -> Element<'_, Message> {
        let content = match self.view {
            View::Main => self.view_main(),
            View::Settings => self.view_settings(),
            View::Logs => self.view_logs(),
        };

        let nav_bar = self.view_nav_bar();
        let status_bar = self.view_status_bar();

        column![nav_bar, content, status_bar]
            .spacing(10)
            .padding(20)
            .into()
    }

    /// Navigation bar.
    fn view_nav_bar(&self) -> Element<'_, Message> {
        let main_btn = button(text("🏠 主页"))
            .on_press(Message::SwitchView(View::Main))
            .style(if self.view == View::Main {
                button::primary
            } else {
                button::secondary
            });

        let settings_btn = button(text("⚙️ 设置"))
            .on_press(Message::SwitchView(View::Settings))
            .style(if self.view == View::Settings {
                button::primary
            } else {
                button::secondary
            });

        let logs_btn = button(text("📋 日志"))
            .on_press(Message::SwitchView(View::Logs))
            .style(if self.view == View::Logs {
                button::primary
            } else {
                button::secondary
            });

        row![main_btn, settings_btn, logs_btn]
            .spacing(10)
            .into()
    }

    /// Status bar.
    fn view_status_bar(&self) -> Element<'_, Message> {
        let state_text = match self.state {
            AppState::Idle => "🟢 就绪",
            AppState::Running => "🔵 运行中",
            AppState::Calibrating => "🟡 校准中",
        };

        row![
            text(state_text).size(14),
            horizontal_space(),
            text(&self.status).size(14),
        ]
        .padding(10)
        .into()
    }

    /// Main view with task input and execution.
    fn view_main(&self) -> Element<'_, Message> {
        let title = text("📱 Phone Agent")
            .size(28);

        let task_input = text_input("输入任务，例如: 打开微信", &self.task_input)
            .on_input(Message::TaskInputChanged)
            .padding(10)
            .size(16);

        let run_btn = if self.state == AppState::Idle {
            button(text("▶️ 运行").size(16))
                .on_press(Message::RunTask)
                .style(button::success)
                .padding([10, 20])
        } else {
            button(text("⏹️ 停止").size(16))
                .on_press(Message::StopTask)
                .style(button::danger)
                .padding([10, 20])
        };

        let calibrate_btn = button(text("🎯 校准").size(16))
            .on_press(Message::RunCalibration)
            .padding([10, 20]);

        let task_row = row![task_input, run_btn, calibrate_btn]
            .spacing(10);

        // Log display
        let log_content = self.logger.format_all();
        let log_view = scrollable(
            text(log_content)
                .size(13)
        )
        .height(Length::Fill);

        let log_container = container(log_view)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .style(container::bordered_box);

        column![
            title,
            vertical_space().height(10),
            task_row,
            vertical_space().height(10),
            text("📜 执行日志").size(16),
            log_container,
        ]
        .spacing(5)
        .height(Length::Fill)
        .into()
    }

    /// Settings view.
    fn view_settings(&self) -> Element<'_, Message> {
        let title = text("⚙️ 设置").size(28);

        // Model settings section
        let model_section = self.view_model_settings();
        
        // Device settings section
        let device_section = self.view_device_settings();
        
        // Coordinate settings section
        let coord_section = self.view_coord_settings();
        
        // Retry settings section
        let retry_section = self.view_retry_settings();
        
        // Calibration settings section
        let calib_section = self.view_calib_settings();

        // Action buttons
        let save_btn = button(text("💾 保存设置"))
            .on_press(Message::SaveSettings)
            .style(button::success)
            .padding([10, 20]);

        let reset_btn = button(text("🔄 重置默认"))
            .on_press(Message::ResetSettings)
            .style(button::secondary)
            .padding([10, 20]);

        let actions = row![save_btn, reset_btn].spacing(10);

        let content = column![
            title,
            vertical_space().height(10),
            model_section,
            horizontal_rule(1),
            device_section,
            horizontal_rule(1),
            coord_section,
            horizontal_rule(1),
            retry_section,
            horizontal_rule(1),
            calib_section,
            vertical_space().height(20),
            actions,
        ]
        .spacing(15)
        .padding(10);

        scrollable(content).height(Length::Fill).into()
    }

    fn view_model_settings(&self) -> Element<'_, Message> {
        let section_title = text("🤖 模型配置").size(18);

        let base_url = labeled_input(
            "API 地址",
            &self.settings.base_url,
            "http://localhost:8000/v1",
            Message::BaseUrlChanged,
        );

        let api_key = labeled_input(
            "API 密钥",
            &self.settings.api_key,
            "EMPTY",
            Message::ApiKeyChanged,
        );

        let model_name = labeled_input(
            "模型名称",
            &self.settings.model_name,
            "autoglm-phone-9b",
            Message::ModelNameChanged,
        );

        column![section_title, base_url, api_key, model_name]
            .spacing(10)
            .into()
    }

    fn view_device_settings(&self) -> Element<'_, Message> {
        let section_title = text("📱 设备配置").size(18);

        let device_id = labeled_input(
            "设备 ID (可选)",
            &self.settings.device_id,
            "留空自动检测",
            Message::DeviceIdChanged,
        );

        let lang_picker = row![
            text("语言").width(120),
            pick_list(
                vec![Language::Chinese, Language::English],
                Some(self.language),
                Message::LanguageSelected,
            )
            .width(200),
        ]
        .spacing(10);

        column![section_title, device_id, lang_picker]
            .spacing(10)
            .into()
    }

    fn view_coord_settings(&self) -> Element<'_, Message> {
        let section_title = text("📐 坐标系统").size(18);

        let coord_picker = row![
            text("坐标系统").width(120),
            pick_list(
                vec![CoordSystemOption::Absolute, CoordSystemOption::Relative],
                Some(self.coord_system),
                Message::CoordSystemSelected,
            )
            .width(200),
        ]
        .spacing(10);

        let scale_inputs = if self.coord_system == CoordSystemOption::Absolute {
            let scale_x = labeled_input(
                "缩放比例 X",
                &self.scale_x_input,
                "1.61",
                Message::ScaleXChanged,
            );

            let scale_y = labeled_input(
                "缩放比例 Y",
                &self.scale_y_input,
                "1.61",
                Message::ScaleYChanged,
            );

            column![scale_x, scale_y].spacing(10)
        } else {
            column![text("相对坐标模式下不需要缩放设置").size(14)]
        };

        column![section_title, coord_picker, scale_inputs]
            .spacing(10)
            .into()
    }

    fn view_retry_settings(&self) -> Element<'_, Message> {
        let section_title = text("🔄 重试配置").size(18);

        let max_retries = labeled_input(
            "最大重试次数",
            &self.max_retries_input,
            "3",
            Message::MaxRetriesChanged,
        );

        let retry_delay = labeled_input(
            "重试延迟(秒)",
            &self.retry_delay_input,
            "2",
            Message::RetryDelayChanged,
        );

        let max_steps = labeled_input(
            "最大步骤数",
            &self.max_steps_input,
            "100",
            Message::MaxStepsChanged,
        );

        column![section_title, max_retries, retry_delay, max_steps]
            .spacing(10)
            .into()
    }

    fn view_calib_settings(&self) -> Element<'_, Message> {
        let section_title = text("🎯 校准配置").size(18);

        let enable_toggle = row![
            text("启用自动校准").width(120),
            toggler(self.settings.enable_calibration)
                .on_toggle(Message::EnableCalibrationToggled),
        ]
        .spacing(10);

        let mode_picker = row![
            text("校准模式").width(120),
            pick_list(
                vec![CalibModeOption::Simple, CalibModeOption::Complex],
                Some(self.calib_mode),
                Message::CalibModeSelected,
            )
            .width(200),
        ]
        .spacing(10);

        let rounds = if self.calib_mode == CalibModeOption::Complex {
            labeled_input(
                "复杂模式轮数",
                &self.calib_rounds_input,
                "5",
                Message::CalibRoundsChanged,
            )
        } else {
            row![].into()
        };

        column![section_title, enable_toggle, mode_picker, rounds]
            .spacing(10)
            .into()
    }

    /// Logs view.
    fn view_logs(&self) -> Element<'_, Message> {
        let title = text("📋 日志").size(28);

        let clear_btn = button(text("🗑️ 清空日志"))
            .on_press(Message::ClearLogs)
            .style(button::secondary);

        let header = row![title, horizontal_space(), clear_btn];

        let log_content = self.logger.format_all();
        let log_view = scrollable(
            text(log_content)
                .size(13)
        )
        .height(Length::Fill);

        let log_container = container(log_view)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .style(container::bordered_box);

        // Show log file path
        let log_path = self
            .logger
            .log_file_path()
            .map(|p| format!("日志文件: {}", p.display()))
            .unwrap_or_else(|| "日志文件: 未创建".to_string());

        column![
            header,
            vertical_space().height(10),
            log_container,
            text(log_path).size(12),
        ]
        .spacing(10)
        .height(Length::Fill)
        .into()
    }
}

/// Helper function to create a labeled input row.
fn labeled_input<'a>(
    label: &'a str,
    value: &'a str,
    placeholder: &'a str,
    on_change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).width(120),
        text_input(placeholder, value)
            .on_input(on_change)
            .width(300),
    ]
    .spacing(10)
    .into()
}

/// Run the agent task asynchronously.
async fn run_agent_task(settings: AppSettings, task: String) -> Result<String, String> {
    // Build model config
    let model_config = ModelConfig::default()
        .with_base_url(&settings.base_url)
        .with_api_key(&settings.api_key)
        .with_model_name(&settings.model_name)
        .with_max_retries(settings.max_retries)
        .with_retry_delay(settings.retry_delay);

    // Build agent config
    let coord_system = CoordSystemOption::from_str(&settings.coordinate_system).as_coordinate_system();
    let mut agent_config = AgentConfig::default()
        .with_lang(&settings.lang)
        .with_coordinate_system(coord_system)
        .with_scale(settings.scale_x, settings.scale_y)
        .with_max_steps(settings.max_steps);

    if !settings.device_id.is_empty() {
        agent_config = agent_config.with_device_id(&settings.device_id);
    }

    // Run calibration if enabled
    let (scale_x, scale_y) = if settings.enable_calibration {
        match run_calibration(settings.clone()).await {
            Ok((x, y)) => (x, y),
            Err(_) => (settings.scale_x, settings.scale_y),
        }
    } else {
        (settings.scale_x, settings.scale_y)
    };

    agent_config = agent_config.with_scale(scale_x, scale_y);

    // Create and run agent
    let mut agent = PhoneAgent::new(model_config, agent_config, None, None);

    agent
        .run(&task)
        .await
        .map_err(|e| e.to_string())
}

/// Run coordinate calibration.
async fn run_calibration(settings: AppSettings) -> Result<(f64, f64), String> {
    let model_config = ModelConfig::default()
        .with_base_url(&settings.base_url)
        .with_api_key(&settings.api_key)
        .with_model_name(&settings.model_name)
        .with_max_retries(settings.max_retries)
        .with_retry_delay(settings.retry_delay);

    let calib_mode = CalibModeOption::from_str(&settings.calibration_mode).as_calibration_mode();
    
    let mut calibration_config = CalibrationConfig::default()
        .with_mode(calib_mode)
        .with_lang(&settings.lang)
        .with_complex_rounds(settings.calibration_rounds);

    if !settings.device_id.is_empty() {
        calibration_config = calibration_config.with_device_id(&settings.device_id);
    }

    let calibrator = CoordinateCalibrator::new(calibration_config);
    let model_client = ModelClient::new(model_config);

    let result = calibrator.calibrate(&model_client).await;

    if result.success {
        Ok((result.scale_x, result.scale_y))
    } else {
        Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}
