//! GUI module for Phone Agent.
//!
//! Provides a graphical user interface using Iced.

pub mod app;
pub mod logger;
pub mod settings;

pub use app::PhoneAgentApp;
pub use logger::{LogEntry, LogLevel, Logger};
pub use settings::AppSettings;
