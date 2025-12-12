//! GUI entry point for Phone Agent.
//!
//! Run with: cargo run --bin phone-agent-gui

use iced::font::Font;
use iced::Size;

use phone_agent::gui::PhoneAgentApp;

/// Embedded Noto Sans SC font for Chinese character support.
const NOTO_SANS_SC: &[u8] = include_bytes!("../../resources/NotoSansSC-Regular.ttf");

fn main() -> iced::Result {
    iced::application(PhoneAgentApp::title, PhoneAgentApp::update, PhoneAgentApp::view)
        .theme(PhoneAgentApp::theme)
        .window_size(Size::new(900.0, 700.0))
        .default_font(Font::with_name("Noto Sans SC"))
        .font(NOTO_SANS_SC)
        .run_with(|| (PhoneAgentApp::new(), iced::Task::none()))
}
