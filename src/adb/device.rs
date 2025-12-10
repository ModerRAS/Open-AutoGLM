//! Device control utilities for Android automation.

use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::config::APP_PACKAGES;

use super::connection::get_adb_prefix;

/// Get the currently focused app name.
///
/// # Arguments
/// * `device_id` - Optional ADB device ID for multi-device setups.
///
/// # Returns
/// The app name if recognized, otherwise "System Home".
pub fn get_current_app(device_id: Option<&str>) -> String {
    let prefix = get_adb_prefix(device_id);
    
    let output = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "dumpsys", "window"])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return "System Home".to_string(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if line.contains("mCurrentFocus") || line.contains("mFocusedApp") {
            for (app_name, package) in APP_PACKAGES.iter() {
                if line.contains(*package) {
                    return app_name.to_string();
                }
            }
        }
    }

    "System Home".to_string()
}

/// Tap at the specified coordinates.
///
/// # Arguments
/// * `x` - X coordinate.
/// * `y` - Y coordinate.
/// * `device_id` - Optional ADB device ID.
/// * `delay_ms` - Delay in milliseconds after tap (default 1000).
pub fn tap(x: i32, y: i32, device_id: Option<&str>, delay_ms: Option<u64>) {
    let prefix = get_adb_prefix(device_id);
    let delay = delay_ms.unwrap_or(1000);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "input", "tap", &x.to_string(), &y.to_string()])
        .output();

    thread::sleep(Duration::from_millis(delay));
}

/// Double tap at the specified coordinates.
///
/// # Arguments
/// * `x` - X coordinate.
/// * `y` - Y coordinate.
/// * `device_id` - Optional ADB device ID.
/// * `delay_ms` - Delay in milliseconds after double tap (default 1000).
pub fn double_tap(x: i32, y: i32, device_id: Option<&str>, delay_ms: Option<u64>) {
    let prefix = get_adb_prefix(device_id);
    let delay = delay_ms.unwrap_or(1000);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "input", "tap", &x.to_string(), &y.to_string()])
        .output();

    thread::sleep(Duration::from_millis(100));

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "input", "tap", &x.to_string(), &y.to_string()])
        .output();

    thread::sleep(Duration::from_millis(delay));
}

/// Long press at the specified coordinates.
///
/// # Arguments
/// * `x` - X coordinate.
/// * `y` - Y coordinate.
/// * `duration_ms` - Duration of press in milliseconds (default 3000).
/// * `device_id` - Optional ADB device ID.
/// * `delay_ms` - Delay in milliseconds after long press (default 1000).
pub fn long_press(
    x: i32,
    y: i32,
    duration_ms: Option<u64>,
    device_id: Option<&str>,
    delay_ms: Option<u64>,
) {
    let prefix = get_adb_prefix(device_id);
    let duration = duration_ms.unwrap_or(3000);
    let delay = delay_ms.unwrap_or(1000);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args([
            "shell",
            "input",
            "swipe",
            &x.to_string(),
            &y.to_string(),
            &x.to_string(),
            &y.to_string(),
            &duration.to_string(),
        ])
        .output();

    thread::sleep(Duration::from_millis(delay));
}

/// Swipe from start to end coordinates.
///
/// # Arguments
/// * `start_x` - Starting X coordinate.
/// * `start_y` - Starting Y coordinate.
/// * `end_x` - Ending X coordinate.
/// * `end_y` - Ending Y coordinate.
/// * `duration_ms` - Duration of swipe in milliseconds (auto-calculated if None).
/// * `device_id` - Optional ADB device ID.
/// * `delay_ms` - Delay in milliseconds after swipe (default 1000).
pub fn swipe(
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    duration_ms: Option<u64>,
    device_id: Option<&str>,
    delay_ms: Option<u64>,
) {
    let prefix = get_adb_prefix(device_id);
    let delay = delay_ms.unwrap_or(1000);

    // Calculate duration based on distance if not provided
    let duration = duration_ms.unwrap_or_else(|| {
        let dist_sq = ((start_x - end_x).pow(2) + (start_y - end_y).pow(2)) as u64;
        let calc_duration = dist_sq / 1000;
        calc_duration.clamp(1000, 2000)
    });

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args([
            "shell",
            "input",
            "swipe",
            &start_x.to_string(),
            &start_y.to_string(),
            &end_x.to_string(),
            &end_y.to_string(),
            &duration.to_string(),
        ])
        .output();

    thread::sleep(Duration::from_millis(delay));
}

/// Press the back button.
///
/// # Arguments
/// * `device_id` - Optional ADB device ID.
/// * `delay_ms` - Delay in milliseconds after pressing back (default 1000).
pub fn back(device_id: Option<&str>, delay_ms: Option<u64>) {
    let prefix = get_adb_prefix(device_id);
    let delay = delay_ms.unwrap_or(1000);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "input", "keyevent", "4"])
        .output();

    thread::sleep(Duration::from_millis(delay));
}

/// Press the home button.
///
/// # Arguments
/// * `device_id` - Optional ADB device ID.
/// * `delay_ms` - Delay in milliseconds after pressing home (default 1000).
pub fn home(device_id: Option<&str>, delay_ms: Option<u64>) {
    let prefix = get_adb_prefix(device_id);
    let delay = delay_ms.unwrap_or(1000);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args(["shell", "input", "keyevent", "KEYCODE_HOME"])
        .output();

    thread::sleep(Duration::from_millis(delay));
}

/// Launch an app by name.
///
/// # Arguments
/// * `app_name` - The app name (must be in APP_PACKAGES).
/// * `device_id` - Optional ADB device ID.
/// * `delay_ms` - Delay in milliseconds after launching (default 1000).
///
/// # Returns
/// True if app was launched, False if app not found.
pub fn launch_app(app_name: &str, device_id: Option<&str>, delay_ms: Option<u64>) -> bool {
    let package = match APP_PACKAGES.get(app_name) {
        Some(p) => *p,
        None => return false,
    };

    let prefix = get_adb_prefix(device_id);
    let delay = delay_ms.unwrap_or(1000);

    let _ = Command::new(&prefix[0])
        .args(&prefix[1..])
        .args([
            "shell",
            "monkey",
            "-p",
            package,
            "-c",
            "android.intent.category.LAUNCHER",
            "1",
        ])
        .output();

    thread::sleep(Duration::from_millis(delay));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_app_unknown() {
        // Should return false for unknown apps
        assert!(!launch_app("UnknownApp123", None, None));
    }
}
