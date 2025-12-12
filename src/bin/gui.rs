//! GUI entry point for Phone Agent.
//!
//! Run with: cargo run --bin phone-agent-gui

use iced::Size;

use phone_agent::gui::PhoneAgentApp;

fn main() -> iced::Result {
    iced::application(PhoneAgentApp::title, PhoneAgentApp::update, PhoneAgentApp::view)
        .theme(PhoneAgentApp::theme)
        .window_size(Size::new(900.0, 700.0))
        .run_with(|| (PhoneAgentApp::new(), iced::Task::none()))
}
