use anyhow::{anyhow, Result};
use std::process::Command;
use serde::{Deserialize, Serialize};
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

/// Tray state stored in runtime dir
#[derive(Serialize, Deserialize, Clone)]
pub struct TrayState {
    pub model: String,
    pub backend: String,
    #[serde(default)]
    pub use_clipboard: bool,
}

/// Get the runtime directory (XDG_RUNTIME_DIR or /tmp fallback)
pub fn get_runtime_dir() -> String {
    std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        let uid = unsafe { libc::getuid() };
        format!("/tmp/whisp-away-{}", uid)
    })
}

/// Get the tray state file path
fn get_state_file() -> String {
    format!("{}/whisp-away-state.json", get_runtime_dir())
}

/// Read current tray state if available
pub fn read_tray_state() -> Option<TrayState> {
    let state_file = get_state_file();
    debug!("Reading tray state from: {}", state_file);
    if let Ok(content) = std::fs::read_to_string(&state_file) {
        match serde_json::from_str::<TrayState>(&content) {
            Ok(state) => {
                debug!("Tray state loaded: backend={}, model={}, clipboard={}", 
                       state.backend, state.model, state.use_clipboard);
                Some(state)
            }
            Err(e) => {
                debug!("Failed to parse tray state: {}", e);
                None
            }
        }
    } else {
        debug!("No tray state file found");
        None
    }
}

/// Write tray state
pub fn write_tray_state(state: &TrayState) -> Result<()> {
    let state_file = get_state_file();
    let runtime_dir = get_runtime_dir();
    
    // Ensure runtime dir exists
    std::fs::create_dir_all(&runtime_dir).ok();
    
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(state_file, json)?;
    Ok(())
}

/// Resolves the model to use with priority:
/// 1. Command-line argument
/// 2. Tray state file
/// 3. WA_WHISPER_MODEL env var
/// 4. Default to "medium.en"
pub fn resolve_model(arg: Option<String>) -> String {
    // Priority 1: Command-line argument
    if let Some(model) = arg {
        debug!("Using model from command-line: {}", model);
        return model;
    }
    
    // Priority 2: Tray state
    if let Some(state) = read_tray_state() {
        debug!("Using model from tray state: {}", state.model);
        return state.model;
    }
    
    // Priority 3: Environment variable
    // Priority 4: Default
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
/// 1. Command-line argument
/// 2. Tray state file
/// 3. WA_USE_CLIPBOARD env var
/// 4. Default to false
pub fn resolve_use_clipboard(arg: Option<bool>) -> bool {
    // Priority 1: Command-line argument
    if let Some(use_clipboard) = arg {
        return use_clipboard;
    }
    
    // Priority 2: Tray state
    if let Some(state) = read_tray_state() {
        return state.use_clipboard;
    }
    
    // Priority 3: Environment variable
    // Priority 4: Default
    std::env::var("WA_USE_CLIPBOARD")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true"
}

