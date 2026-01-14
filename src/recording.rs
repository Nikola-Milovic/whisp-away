use anyhow::{Context, Result};
use std::fs::{self, File};
use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn, error};
use crate::helpers::is_process_running;

const LOCK_FILE: &str = "/tmp/whisp-away-recording.lock";
const PID_FILE: &str = "/tmp/whisp-away-recording.pid";
const MAX_RECORDING_AGE_SECS: u64 = 600; // 10 minutes

/// Check if a recording is currently in progress
pub fn is_recording() -> bool {
    // Check if pidfile exists and process is running
    let pid_exists = std::path::Path::new(PID_FILE).exists();
    let lock_exists = std::path::Path::new(LOCK_FILE).exists();
    
    debug!("Checking recording status - pid_file exists: {}, lock_file exists: {}", 
           pid_exists, lock_exists);
    
    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        debug!("PID file contents: '{}'", pid_str.trim());
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            let running = is_process_running(pid);
            debug!("PID {} running: {}", pid, running);
            if running {
                info!("Recording in progress (PID: {})", pid);
                return true;
            }
        } else {
            debug!("Failed to parse PID from: '{}'", pid_str.trim());
        }
    } else if pid_exists {
        debug!("PID file exists but couldn't read it");
    }
    
    // Also check if lock file exists and is locked
    if lock_exists {
        if let Ok(lock_file) = fs::OpenOptions::new().read(true).open(LOCK_FILE) {
            use std::os::unix::io::AsRawFd;
            let fd = lock_file.as_raw_fd();
            // Try to acquire lock non-blocking - if it fails, someone else has it
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            debug!("flock result: {} (0 = acquired, non-zero = held by other)", result);
            if result != 0 {
                info!("Recording lock is held by another process");
                return true;
            }
            // We got the lock, release it immediately
            unsafe { libc::flock(fd, libc::LOCK_UN) };
        } else {
            debug!("Couldn't open lock file");
        }
    }
    
    debug!("No recording in progress");
    false
}

/// Acquire an exclusive lock for recording
/// Returns the lock file handle that must be kept alive during recording
fn acquire_lock() -> Result<File> {
    debug!("Attempting to acquire recording lock at {}", LOCK_FILE);
    
    let lock_file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(LOCK_FILE)
        .context("Failed to create lock file")?;
    
    // Try to acquire exclusive lock (non-blocking)
    use std::os::unix::io::AsRawFd;
    let fd = lock_file.as_raw_fd();
    let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    
    if result != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::WouldBlock {
            warn!("Another recording is already in progress (lock held)");
            return Err(anyhow::anyhow!("Another recording is already in progress"));
        }
        return Err(anyhow::anyhow!("Failed to acquire lock: {}", err));
    }
    
    debug!("Successfully acquired recording lock");
    Ok(lock_file)
}

/// Release the lock (happens automatically when File is dropped, but this is explicit)
fn release_lock(lock_file: File) {
    use std::os::unix::io::AsRawFd;
    let fd = lock_file.as_raw_fd();
    unsafe { libc::flock(fd, libc::LOCK_UN) };
    drop(lock_file);
    let _ = fs::remove_file(LOCK_FILE);
    debug!("Released recording lock");
}

/// Kill any existing recording process forcefully
fn kill_existing_recording() -> Result<()> {
    debug!("Checking for existing recording process");
    
    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        let pid_str = pid_str.trim();
        if !pid_str.is_empty() {
            if let Ok(pid) = pid_str.parse::<u32>() {
                if is_process_running(pid) {
                    debug!("Killing existing recording process (PID: {})", pid);
                    
                    // Try SIGINT first for graceful shutdown
                    let _ = Command::new("kill")
                        .args(["-INT", &pid.to_string()])
                        .status();
                    
                    std::thread::sleep(Duration::from_millis(100));
                    
                    // If still running, use SIGTERM
                    if is_process_running(pid) {
                        debug!("Process still running after SIGINT, sending SIGTERM");
                        let _ = Command::new("kill")
                            .args(["-TERM", &pid.to_string()])
                            .status();
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    
                    // If STILL running, use SIGKILL
                    if is_process_running(pid) {
                        warn!("Process still running after SIGTERM, sending SIGKILL");
                        let _ = Command::new("kill")
                            .args(["-KILL", &pid.to_string()])
                            .status();
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    
                    if is_process_running(pid) {
                        error!("Failed to kill recording process (PID: {})", pid);
                        return Err(anyhow::anyhow!("Failed to kill existing recording process"));
                    }
                    
                    debug!("Successfully killed existing recording process");
                } else {
                    debug!("PID {} in pidfile is not running", pid);
                }
            }
        }
        let _ = fs::remove_file(PID_FILE);
    } else {
        debug!("No existing pidfile found");
    }
    
    Ok(())
}

/// Clean up old recording files (older than MAX_RECORDING_AGE_SECS)
fn cleanup_old_recordings(runtime_dir: &str, current_audio_file: Option<&str>) {
    debug!("Cleaning up old recording files in {}", runtime_dir);
    
    let now = SystemTime::now();
    let mut cleaned = 0;
    
    if let Ok(entries) = fs::read_dir(runtime_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                // Clean up voice recording files
                if name.starts_with("voice-recording-") && name.ends_with(".wav") {
                    let path = entry.path();
                    
                    // Don't delete the current recording file
                    if current_audio_file == path.to_str() {
                        continue;
                    }
                    
                    // Check file age
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age.as_secs() > MAX_RECORDING_AGE_SECS {
                                    debug!("Removing old recording: {} (age: {}s)", name, age.as_secs());
                                    if fs::remove_file(&path).is_ok() {
                                        cleaned += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    if cleaned > 0 {
        debug!("Cleaned up {} old recording file(s)", cleaned);
    }
}

/// Send a notification, handling errors gracefully
fn send_notification(title: &str, message: &str, timeout_ms: u32) {
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
                warn!("notify-send failed: {}", stderr);
                // Fallback: print to console
                eprintln!("[whisp-away] {}: {}", title, message);
            }
        }
        Err(e) => {
            warn!("Failed to run notify-send: {}", e);
            // Fallback: print to console
            eprintln!("[whisp-away] {}: {}", title, message);
        }
    }
}

/// Stop the recording process and return the audio file path
pub fn stop_recording(audio_file_override: Option<&str>) -> Result<Option<String>> {
    debug!("Stopping recording...");
    let uid = unsafe { libc::getuid() };
    
    // Wait a bit for the pidfile to appear if it doesn't exist yet
    let mut attempts = 0;
    while !std::path::Path::new(PID_FILE).exists() && attempts < 10 {
        debug!("Waiting for pidfile (attempt {})", attempts + 1);
        std::thread::sleep(Duration::from_millis(20));
        attempts += 1;
    }
    
    // Stop the recording process if it's running
    if let Ok(pid_str) = fs::read_to_string(PID_FILE) {
        let pid_str = pid_str.trim();
        if pid_str.is_empty() {
            debug!("Pidfile is empty");
            let _ = fs::remove_file(PID_FILE);
            return Ok(None);
        }
        
        if let Ok(pid) = pid_str.parse::<u32>() {
            debug!("Found recording PID: {}", pid);
            
            if !is_process_running(pid) {
                debug!("Recording process {} is not running", pid);
                let _ = fs::remove_file(PID_FILE);
                let _ = fs::remove_file(format!("/run/user/{}/voice-audio-file.tmp", uid));
                return Ok(None);
            }
            
            // Try graceful shutdown first
            debug!("Sending SIGINT to recording process (PID: {})", pid);
            std::thread::sleep(Duration::from_millis(100));
            
            let _ = Command::new("kill")
                .args(["-INT", &pid.to_string()])
                .status();
            
            std::thread::sleep(Duration::from_millis(50));
            
            // Force kill if still running
            if is_process_running(pid) {
                debug!("Process still running, sending SIGTERM");
                let _ = Command::new("kill")
                    .args(["-TERM", &pid.to_string()])
                    .status();
            }
            
            std::thread::sleep(Duration::from_millis(50));
            
            // Check one more time and use SIGKILL if needed
            if is_process_running(pid) {
                warn!("Process still running after SIGTERM, sending SIGKILL");
                let _ = Command::new("kill")
                    .args(["-KILL", &pid.to_string()])
                    .status();
                std::thread::sleep(Duration::from_millis(50));
            }
            
            debug!("Recording stopped");
        }
    } else {
        debug!("No pidfile found at {}", PID_FILE);
    }
    
    let _ = fs::remove_file(PID_FILE);
    
    // Release any lock that might be held
    if std::path::Path::new(LOCK_FILE).exists() {
        let _ = fs::remove_file(LOCK_FILE);
        debug!("Removed stale lock file");
    }

    // Get the audio file path
    let audio_file = if let Some(override_path) = audio_file_override {
        debug!("Using override audio file: {}", override_path);
        // Copy the override file to a temporary location so it can be cleaned up
        let runtime_dir = crate::helpers::get_runtime_dir();
        let temp_audio = format!("{}/voice-recording-override-{}.wav", runtime_dir, 
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis());
        fs::copy(override_path, &temp_audio)
            .context("Failed to copy audio file to temporary location")?;
        debug!("Copied override audio to: {}", temp_audio);
        temp_audio
    } else {
        let audio_path_file = format!("/run/user/{}/voice-audio-file.tmp", uid);
        match fs::read_to_string(&audio_path_file) {
            Ok(path) => {
                let path = path.trim().to_string();
                let _ = fs::remove_file(&audio_path_file);
                
                // Verify the audio file exists
                if std::path::Path::new(&path).exists() {
                if let Ok(metadata) = fs::metadata(&path) {
                    debug!("Audio file ready: {} ({} bytes)", path, metadata.len());
                }
                } else {
                    warn!("Audio file does not exist: {}", path);
                }
                
                path
            },
            Err(e) => {
                debug!("Could not read audio file path: {}", e);
                return Ok(None);
            }
        }
    };
    
    Ok(Some(audio_file))
}

/// Common function to start recording audio
pub fn start_recording() -> Result<()> {
    debug!("Starting recording...");
    
    let uid = unsafe { libc::getuid() };
    let runtime_dir = crate::helpers::get_runtime_dir();
    
    // Clean up old recordings first (older than 10 minutes)
    cleanup_old_recordings(&runtime_dir, None);
    
    // Kill any existing recording process FIRST
    kill_existing_recording()?;
    
    // Now try to acquire the lock
    let _lock = acquire_lock()?;
    
    // Generate unique audio file name
    let audio_file = format!("{}/voice-recording-{}.wav", runtime_dir, 
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis());
    
    debug!("Audio file will be: {}", audio_file);
    
    // Store the audio file path for later retrieval
    let audio_path_file = format!("/run/user/{}/voice-audio-file.tmp", uid);
    fs::write(&audio_path_file, &audio_file)
        .context("Failed to write audio file path")?;
    debug!("Wrote audio path to: {}", audio_path_file);

    // Start recording
    debug!("Starting pw-record...");
    let child = Command::new("pw-record")
        .args([
            "--channels", "1",
            "--rate", "16000",
            "--format", "s16",
            "--volume", "1.5",
            &audio_file,
        ])
        .spawn()
        .context("Failed to start pw-record")?;

    let pid = child.id();
    debug!("pw-record started with PID: {}", pid);
    
    fs::write(PID_FILE, pid.to_string())
        .context("Failed to write PID file")?;
    debug!("Wrote PID {} to {}", pid, PID_FILE);

    // Get config from environment for notification
    let model = crate::helpers::resolve_model();
    let backend = crate::helpers::resolve_backend();
    let acceleration = crate::helpers::get_acceleration_type();
    let recording_msg = format!("Recording... (release to stop)\nBackend: {} ({}) | Model: {}", backend, acceleration, model);
    
    send_notification("Voice Input", &recording_msg, 30000);

    // Note: We intentionally don't release the lock here - it will be released
    // when stop_recording is called or when the process exits
    // The lock file handle is dropped here, which releases the lock
    // This is actually desired behavior as we want the recording to be exclusive
    // only during the start phase, not during the actual recording
    
    debug!("Recording started successfully");
    Ok(())
}
