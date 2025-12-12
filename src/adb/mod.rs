//! ADB (Android Debug Bridge) module for device interaction.

mod connection;
mod device;
pub mod input;
mod screenshot;

pub use connection::{ADBConnection, ConnectionType, DeviceInfo};
pub use device::{back, double_tap, get_current_app, home, launch_app, long_press, swipe, tap};
pub use input::{clear_text, detect_and_set_adb_keyboard, restore_keyboard, type_text};
pub use screenshot::{get_screenshot, Screenshot};
