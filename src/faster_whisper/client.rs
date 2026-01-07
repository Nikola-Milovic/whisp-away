use anyhow::Result;
use std::fs;
use tracing::{debug, warn};
use crate::recording;
use crate::socket;
use crate::helpers;
use super::direct::transcribe_with_faster_whisper;

pub fn stop_and_transcribe_daemon(socket_path: &str, use_clipboard: bool) -> Result<()> {
    debug!("stop_and_transcribe_daemon called, socket_path: {}", socket_path);
    
    let audio_file = match recording::stop_recording(None)? {
        Some(path) => {
            debug!("Got audio file: {}", path);
            path
        }
        None => {
            warn!("No recording found");
            helpers::send_notification(
                "Voice Input (daemon)",
                "❌ No recording found",
                2000
            );
            return Ok(());
        }
    };

    let audio_path = std::path::Path::new(&audio_file);
    if !audio_path.exists() {
        warn!("Audio file does not exist: {}", audio_file);
        helpers::send_notification(
            "Voice Input",
            "❌ No audio recorded\nBackend: faster-whisper",
            2000
        );
        return Ok(());
    }
    
    if let Ok(metadata) = fs::metadata(&audio_file) {
        let file_size = metadata.len();
        debug!("Audio file size: {} bytes", file_size);
        
        if file_size <= 44 {
            warn!("Audio file is empty (only WAV header): {} bytes", file_size);
            helpers::send_notification(
                "Voice Input",
                "❌ Audio file is empty\nBackend: faster-whisper",
                2000
            );
            let _ = fs::remove_file(&audio_file);
            return Ok(());
        }
    }

    // Get model for notification
    let model = helpers::resolve_model();
    let acceleration = helpers::get_acceleration_type();
    let transcribe_msg = format!("⏳ Transcribing...\nBackend: faster-whisper ({}) | Model: {}", acceleration, model);
    
    debug!("Sending transcription request, model: {}, acceleration: {}", model, acceleration);
    helpers::send_notification("Voice Input", &transcribe_msg, 2000);

    match socket::send_transcription_request(socket_path, &audio_file, "faster-whisper", use_clipboard) {
        Ok(_) => {
            debug!("Daemon transcription completed successfully");
            let _ = fs::remove_file(&audio_file);
        }
        Err(e) => {
            warn!("Daemon not available ({}), falling back to direct mode", e);
            helpers::send_notification(
                "Voice Input (daemon)",
                "⚠️ Daemon not running, using direct mode",
                2000
            );
            
            // Use the resolved model, not hardcoded base.en
            let result = transcribe_with_faster_whisper(&audio_file, &model, use_clipboard);
            
            let _ = fs::remove_file(&audio_file);
            
            return result.map_err(|err| anyhow::anyhow!("Fallback transcription failed (daemon was: {}): {}", e, err));
        }
    }

    Ok(())
}