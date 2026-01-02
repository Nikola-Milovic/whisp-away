use anyhow::Result;
use std::process::Command;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

/// Daemon configuration - written by daemon, read by CLI commands
/// This ensures CLI commands use the same settings as the running daemon
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonConfig {
    pub backend: Option<String>,
    pub model: Option<String>,
    pub socket_path: Option<String>,
    pub use_clipboard: Option<bool>,
}

/// Get the path to the daemon config file
fn get_daemon_config_path() -> String {
    format!("{}/whisp-away-daemon.json", get_runtime_dir())
}

/// Write daemon configuration (called when daemon starts)
pub fn write_daemon_config(config: &DaemonConfig) -> Result<()> {
    let config_path = get_daemon_config_path();
    let runtime_dir = get_runtime_dir();
    
    // Ensure runtime dir exists
    std::fs::create_dir_all(&runtime_dir).ok();
    
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&config_path, json)?;
    debug!("Wrote daemon config to: {}", config_path);
    Ok(())
}

/// Read daemon configuration (called by CLI commands)
pub fn read_daemon_config() -> Option<DaemonConfig> {
    let config_path = get_daemon_config_path();
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        match serde_json::from_str::<DaemonConfig>(&content) {
            Ok(config) => {
                trace!("Read daemon config from: {}", config_path);
                Some(config)
            }
            Err(e) => {
                debug!("Failed to parse daemon config: {}", e);
                None
            }
        }
    } else {
        trace!("No daemon config file found at: {}", config_path);
        None
    }
}

pub fn is_process_running(pid: u32) -> bool {
    let running = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    trace!("Process {} running: {}", pid, running);
    running
}



pub fn wav_to_samples(wav_data: &[u8]) -> Result<Vec<f32>> {
    // Skip WAV header (44 bytes) and convert to f32 samples
    // This assumes 16-bit PCM mono audio at 16kHz
    
    if wav_data.len() < 44 {
        return Err(anyhow::anyhow!("Invalid WAV file: too short"));
    }
    
    let raw_samples = &wav_data[44..];
    let mut samples = Vec::with_capacity(raw_samples.len() / 2);
    
    for chunk in raw_samples.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(sample as f32 / i16::MAX as f32);
    }
    
    Ok(samples)
}

/// Get the runtime directory (XDG_RUNTIME_DIR or /tmp fallback)
pub fn get_runtime_dir() -> String {
    std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        let uid = unsafe { libc::getuid() };
        format!("/tmp/whisp-away-{}", uid)
    })
}

/// Resolves the socket path with priority:
/// 1. Command-line argument (explicit override)
/// 2. WA_WHISPER_SOCKET env var (set via NixOS config)
/// 3. Daemon config file (written by running daemon)
/// 4. Default to "/tmp/whisp-away-daemon.sock"
pub fn resolve_socket_path(arg: Option<String>) -> String {
    if let Some(path) = arg {
        debug!("Using socket path from command-line: {}", path);
        return path;
    }
    
    if let Ok(path) = std::env::var("WA_WHISPER_SOCKET") {
        debug!("Using socket path from env: {}", path);
        return path;
    }
    
    if let Some(config) = read_daemon_config() {
        if let Some(path) = config.socket_path {
            debug!("Using socket path from daemon config: {}", path);
            return path;
        }
    }
    
    let path = "/tmp/whisp-away-daemon.sock".to_string();
    debug!("Using default socket path: {}", path);
    path
}

/// Resolves the backend with priority:
/// 1. Command-line argument (explicit override)
/// 2. WA_WHISPER_BACKEND env var (set via NixOS config)
/// 3. Daemon config file (written by running daemon)
/// 4. Default to "faster-whisper"
pub fn resolve_backend(arg: Option<String>) -> String {
    if let Some(backend) = arg {
        debug!("Using backend from command-line: {}", backend);
        return backend;
    }
    
    if let Ok(backend) = std::env::var("WA_WHISPER_BACKEND") {
        debug!("Using backend from env: {}", backend);
        return backend;
    }
    
    if let Some(config) = read_daemon_config() {
        if let Some(backend) = config.backend {
            debug!("Using backend from daemon config: {}", backend);
            return backend;
        }
    }
    
    let backend = "faster-whisper".to_string();
    debug!("Using default backend: {}", backend);
    backend
}

/// Resolves the model to use with priority:
/// 1. Command-line argument (explicit override)
/// 2. WA_WHISPER_MODEL env var (set via NixOS config)
/// 3. Daemon config file (written by running daemon)
/// 4. Default to "base.en"
pub fn resolve_model(arg: Option<String>) -> String {
    if let Some(model) = arg {
        debug!("Using model from command-line: {}", model);
        return model;
    }
    
    if let Ok(model) = std::env::var("WA_WHISPER_MODEL") {
        debug!("Using model from env: {}", model);
        return model;
    }
    
    if let Some(config) = read_daemon_config() {
        if let Some(model) = config.model {
            debug!("Using model from daemon config: {}", model);
            return model;
        }
    }
    
    let model = "base.en".to_string();
    debug!("Using default model: {}", model);
    model
}

/// Get the acceleration type from environment variable
pub fn get_acceleration_type() -> String {
    std::env::var("WA_ACCELERATION_TYPE").unwrap_or_else(|_| "unknown".to_string())
}

/// Send a notification, handling errors gracefully
pub fn send_notification(title: &str, message: &str, timeout_ms: u32) {
    use std::process::Command;
    debug!("Sending notification: {} - {}", title, message);
    
    match Command::new("notify-send")
        .args([
            title,
            message,
            "-t", &timeout_ms.to_string(),
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                debug!("notify-send failed: {}", stderr);
                // Fallback: print to console
                eprintln!("[whisp-away] {}: {}", title, message);
            }
        }
        Err(e) => {
            debug!("Failed to run notify-send: {}", e);
            // Fallback: print to console
            eprintln!("[whisp-away] {}: {}", title, message);
        }
    }
}

/// Resolves whether to use clipboard with priority:
/// 1. Command-line argument (explicit override)
/// 2. WA_USE_CLIPBOARD env var (set via NixOS config)
/// 3. Daemon config file (written by running daemon)
/// 4. Default to false
pub fn resolve_use_clipboard(arg: Option<bool>) -> bool {
    if let Some(use_clipboard) = arg {
        debug!("Using clipboard setting from command-line: {}", use_clipboard);
        return use_clipboard;
    }
    
    if let Ok(val) = std::env::var("WA_USE_CLIPBOARD") {
        let use_clipboard = val.to_lowercase() == "true";
        debug!("Using clipboard setting from env: {}", use_clipboard);
        return use_clipboard;
    }
    
    if let Some(config) = read_daemon_config() {
        if let Some(use_clipboard) = config.use_clipboard {
            debug!("Using clipboard setting from daemon config: {}", use_clipboard);
            return use_clipboard;
        }
    }
    
    debug!("Using default clipboard setting: false");
    false
}

