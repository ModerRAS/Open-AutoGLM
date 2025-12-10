//! ADB connection management for local and remote devices.

use std::process::Command;
use thiserror::Error;

/// Type of ADB connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionType {
    Usb,
    Wifi,
    Remote,
}

/// Information about a connected device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub status: String,
    pub connection_type: ConnectionType,
    pub model: Option<String>,
    pub android_version: Option<String>,
}

/// ADB connection errors.
#[derive(Error, Debug)]
pub enum AdbError {
    #[error("Connection timeout after {0}s")]
    Timeout(u64),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Command execution failed: {0}")]
    CommandFailed(String),
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
}

/// Manages ADB connections to Android devices.
///
/// Supports USB, WiFi, and remote TCP/IP connections.
///
/// # Example
/// ```rust,no_run
/// use phone_agent::adb::ADBConnection;
///
/// let conn = ADBConnection::new();
/// // Connect to remote device
/// let result = conn.connect("192.168.1.100:5555");
/// // List devices
/// let devices = conn.list_devices();
/// ```
pub struct ADBConnection {
    adb_path: String,
}

impl Default for ADBConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl ADBConnection {
    /// Create a new ADB connection manager with default path.
    pub fn new() -> Self {
        Self {
            adb_path: "adb".to_string(),
        }
    }

    /// Create a new ADB connection manager with custom ADB path.
    pub fn with_path(adb_path: impl Into<String>) -> Self {
        Self {
            adb_path: adb_path.into(),
        }
    }

    /// Connect to a remote device via TCP/IP.
    ///
    /// # Arguments
    /// * `address` - Device address in format "host:port" (e.g., "192.168.1.100:5555").
    ///
    /// # Returns
    /// Tuple of (success, message).
    pub fn connect(&self, address: &str) -> Result<String, AdbError> {
        let address = if !address.contains(':') {
            format!("{}:5555", address)
        } else {
            address.to_string()
        };

        let output = Command::new(&self.adb_path)
            .args(["connect", &address])
            .output()
            .map_err(|e| AdbError::Connection(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        if combined.to_lowercase().contains("connected")
            || combined.to_lowercase().contains("already connected")
        {
            Ok(format!("Connected to {}", address))
        } else {
            Err(AdbError::Connection(combined.trim().to_string()))
        }
    }

    /// Disconnect from a remote device.
    ///
    /// # Arguments
    /// * `address` - Device address to disconnect. If None, disconnects all.
    pub fn disconnect(&self, address: Option<&str>) -> Result<String, AdbError> {
        let args: Vec<&str> = match address {
            Some(addr) => vec!["disconnect", addr],
            None => vec!["disconnect"],
        };

        let output = Command::new(&self.adb_path)
            .args(&args)
            .output()
            .map_err(|e| AdbError::Connection(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    /// List all connected devices.
    pub fn list_devices(&self) -> Result<Vec<DeviceInfo>, AdbError> {
        let output = Command::new(&self.adb_path)
            .args(["devices", "-l"])
            .output()
            .map_err(|e| AdbError::CommandFailed(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut devices = Vec::new();

        for line in stdout.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let device_id = parts[0].to_string();
                let status = parts[1].to_string();

                let connection_type = if device_id.contains(':') {
                    ConnectionType::Remote
                } else if device_id.contains("usb") {
                    ConnectionType::Usb
                } else {
                    ConnectionType::Wifi
                };

                // Extract model if available
                let model = parts.iter()
                    .find(|p| p.starts_with("model:"))
                    .map(|m| m.replace("model:", ""));

                devices.push(DeviceInfo {
                    device_id,
                    status,
                    connection_type,
                    model,
                    android_version: None,
                });
            }
        }

        Ok(devices)
    }

    /// Check if ADB server is running.
    pub fn is_running(&self) -> bool {
        Command::new(&self.adb_path)
            .args(["devices"])
            .output()
            .is_ok()
    }

    /// Start ADB server.
    pub fn start_server(&self) -> Result<(), AdbError> {
        Command::new(&self.adb_path)
            .args(["start-server"])
            .output()
            .map_err(|e| AdbError::CommandFailed(e.to_string()))?;
        Ok(())
    }

    /// Kill ADB server.
    pub fn kill_server(&self) -> Result<(), AdbError> {
        Command::new(&self.adb_path)
            .args(["kill-server"])
            .output()
            .map_err(|e| AdbError::CommandFailed(e.to_string()))?;
        Ok(())
    }
}

/// Get ADB command prefix with optional device specifier.
pub(crate) fn get_adb_prefix(device_id: Option<&str>) -> Vec<String> {
    match device_id {
        Some(id) => vec!["adb".to_string(), "-s".to_string(), id.to_string()],
        None => vec!["adb".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adb_connection_new() {
        let conn = ADBConnection::new();
        assert_eq!(conn.adb_path, "adb");
    }

    #[test]
    fn test_get_adb_prefix() {
        let prefix = get_adb_prefix(None);
        assert_eq!(prefix, vec!["adb"]);

        let prefix_with_device = get_adb_prefix(Some("device123"));
        assert_eq!(prefix_with_device, vec!["adb", "-s", "device123"]);
    }
}
