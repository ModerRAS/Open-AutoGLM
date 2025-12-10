//! Input utilities for Android device text input.

use base64::{engine::general_purpose::STANDARD, Engine};
use std::process::Command;
use std::thread;
use std::time::Duration;

use super::connection::get_adb_prefix;

/// Type text into the currently focused input field using ADB Keyboard.
///
/// # Arguments
/// * `text` - The text to type.
/// * `device_id` - Optional ADB device ID for multi-device setups.
///
/// # Note
/// Requires ADB Keyboard to be installed on the device.
/// See: https://github.com/nicnocquee/AdbKeyboard
pub fn type_text(text: &str, device_id: Option<&str>) {
    let prefix = get_adb_prefix(device_id);
    let encoded_text = STANDARD.encode(text.as_bytes());

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args([
            "shell",
            "am",
            "broadcast",
            "-a",
            "ADB_INPUT_B64",
            "--es",
            "msg",
            &encoded_text,
        ])
        .output();
}

/// Clear text in the currently focused input field.
///
/// # Arguments
/// * `device_id` - Optional ADB device ID for multi-device setups.
pub fn clear_text(device_id: Option<&str>) {
    let prefix = get_adb_prefix(device_id);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "am", "broadcast", "-a", "ADB_CLEAR_TEXT"])
        .output();
}

/// Detect current keyboard and switch to ADB Keyboard if needed.
///
/// # Arguments
/// * `device_id` - Optional ADB device ID for multi-device setups.
///
/// # Returns
/// The original keyboard IME identifier for later restoration.
pub fn detect_and_set_adb_keyboard(device_id: Option<&str>) -> String {
    let prefix = get_adb_prefix(device_id);

    // Get current IME
    let output = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "settings", "get", "secure", "default_input_method"])
        .output();

    let current_ime = match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            format!("{}{}", stdout, stderr).trim().to_string()
        }
        Err(_) => String::new(),
    };

    // Switch to ADB Keyboard if not already set
    if !current_ime.contains("com.android.adbkeyboard/.AdbIME") {
        let _ = Command::new(&prefix[0])
            .args(&prefix[1..])
            .args(["shell", "ime", "set", "com.android.adbkeyboard/.AdbIME"])
            .output();
    }

    // Warm up the keyboard
    type_text("", device_id);

    current_ime
}

/// Restore the original keyboard IME.
///
/// # Arguments
/// * `ime` - The IME identifier to restore.
/// * `device_id` - Optional ADB device ID for multi-device setups.
pub fn restore_keyboard(ime: &str, device_id: Option<&str>) {
    if ime.is_empty() || ime.contains("com.android.adbkeyboard/.AdbIME") {
        return;
    }

    let prefix = get_adb_prefix(device_id);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "ime", "set", ime])
        .output();
}

/// Type text with full keyboard handling (switch, type, restore).
///
/// # Arguments
/// * `text` - The text to type.
/// * `device_id` - Optional ADB device ID for multi-device setups.
pub fn type_text_with_keyboard_handling(text: &str, device_id: Option<&str>) {
    // Switch to ADB keyboard
    let original_ime = detect_and_set_adb_keyboard(device_id);
    thread::sleep(Duration::from_secs(1));

    // Clear existing text and type new text
    clear_text(device_id);
    thread::sleep(Duration::from_secs(1));

    type_text(text, device_id);
    thread::sleep(Duration::from_secs(1));

    // Restore original keyboard
    restore_keyboard(&original_ime, device_id);
    thread::sleep(Duration::from_secs(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encoding() {
        let text = "Hello, 世界!";
        let encoded = STANDARD.encode(text.as_bytes());
        assert!(!encoded.is_empty());
    }
}
