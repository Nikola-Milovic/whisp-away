use anyhow::Result;
use std::process::Command;
use tracing::{debug, trace};

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
/// 3. Default to "/tmp/whisp-away-daemon.sock"
pub fn resolve_socket_path(arg: Option<String>) -> String {
    if let Some(path) = arg {
        debug!("Using socket path from command-line: {}", path);
        return path;
    }
    
    let path = std::env::var("WA_WHISPER_SOCKET")
        .unwrap_or_else(|_| "/tmp/whisp-away-daemon.sock".to_string());
    debug!("Using socket path from env/default: {}", path);
    path
}

/// Resolves the backend with priority:
/// 1. Command-line argument (explicit override)
/// 2. WA_WHISPER_BACKEND env var (set via NixOS config)
/// 3. Default to "faster-whisper"
pub fn resolve_backend(arg: Option<String>) -> String {
    if let Some(backend) = arg {
        debug!("Using backend from command-line: {}", backend);
        return backend;
    }
    
    let backend = std::env::var("WA_WHISPER_BACKEND")
        .unwrap_or_else(|_| "faster-whisper".to_string());
    debug!("Using backend from env/default: {}", backend);
    backend
}

/// Resolves the model to use with priority:
/// 1. Command-line argument (explicit override)
/// 2. WA_WHISPER_MODEL env var (set via NixOS config)
/// 3. Default to "base.en"
pub fn resolve_model(arg: Option<String>) -> String {
    if let Some(model) = arg {
        debug!("Using model from command-line: {}", model);
        return model;
    }
    
    let model = std::env::var("WA_WHISPER_MODEL").unwrap_or_else(|_| "base.en".to_string());
    debug!("Using model from env/default: {}", model);
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
/// 3. Default to false
pub fn resolve_use_clipboard(arg: Option<bool>) -> bool {
    if let Some(use_clipboard) = arg {
        debug!("Using clipboard setting from command-line: {}", use_clipboard);
        return use_clipboard;
    }
    
    let use_clipboard = std::env::var("WA_USE_CLIPBOARD")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true";
    debug!("Using clipboard setting from env/default: {}", use_clipboard);
    use_clipboard
}

