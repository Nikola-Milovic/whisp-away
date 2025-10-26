use anyhow::Result;
use std::fs;
use std::process::Command;
use crate::recording;
use crate::socket;
use super::direct::transcribe_with_faster_whisper;

pub fn stop_and_transcribe_daemon(socket_path: &str, use_clipboard: bool) -> Result<()> {
    let audio_file = match recording::stop_recording(None)? {
        Some(path) => path,
        None => {
            Command::new("notify-send")
                .args(&[
                    "Voice Input (daemon)",
                    "❌ No recording found",
                    "-t", "2000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
            return Ok(());
        }
    };

    let audio_path = std::path::Path::new(&audio_file);
    if !audio_path.exists() {
            Command::new("notify-send")
                .args(&[
                    "Voice Input",
                    "❌ No audio recorded\nBackend: faster-whisper",
                    "-t", "2000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
        return Ok(());
    }
    
    if let Ok(metadata) = fs::metadata(&audio_file) {
        if metadata.len() <= 44 {
            Command::new("notify-send")
                .args(&[
                    "Voice Input",
                    "❌ Audio file is empty\nBackend: faster-whisper",
                    "-t", "2000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
            let _ = fs::remove_file(&audio_file);
            return Ok(());
        }
    }

    // Get model for notification
    let model = crate::helpers::resolve_model(None);
    let acceleration = crate::helpers::get_acceleration_type();
    let transcribe_msg = format!("⏳ Transcribing...\nBackend: faster-whisper ({}) | Model: {}", acceleration, model);
    
    Command::new("notify-send")
        .args(&[
            "Voice Input",
            &transcribe_msg,
            "-t", "2000",
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .spawn()?;

    match socket::send_transcription_request(socket_path, &audio_file, "faster-whisper", use_clipboard) {
        Ok(_) => {
            let _ = fs::remove_file(&audio_file);
        }
        Err(e) => {
            Command::new("notify-send")
                .args(&[
                    "Voice Input (daemon)",
                    "⚠️ Daemon not running, using direct mode",
                    "-t", "2000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
            
            let result = transcribe_with_faster_whisper(&audio_file, "base.en", use_clipboard);
            
            let _ = fs::remove_file(&audio_file);
            
            return result.map_err(|err| anyhow::anyhow!("Fallback transcription failed (daemon was: {}): {}", e, err));
        }
    }

    Ok(())
}