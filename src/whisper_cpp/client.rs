use anyhow::Result;
use std::fs;
use std::process::Command;
use crate::recording;
use crate::socket;
use super::direct::{transcribe_with_whisper_rs, transcribe_with_cli};

pub fn stop_and_transcribe_daemon(socket_path: &str, audio_file_override: Option<&str>, model: Option<String>, bindings: bool, whisper_path: Option<String>, use_clipboard: bool) -> Result<()> {
    let audio_file = match recording::stop_recording(audio_file_override)? {
        Some(path) => path,
        None => {
            Command::new("notify-send")
                .args(&[
                    "Voice Input (whisper.cpp daemon)",
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
                "Voice Input (whisper.cpp daemon)",
                "❌ No audio recorded",
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
                    "❌ Audio file is empty\nBackend: whisper-cpp",
                    "-t", "2000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
            let _ = fs::remove_file(&audio_file);
            return Ok(());
        }
    }

    let start_time = std::time::Instant::now();
    eprintln!("DEBUG: Starting transcription at {:?}", start_time);
    
    // Get model for notification
    let resolved_model = crate::helpers::resolve_model(model.clone());
    let acceleration = crate::helpers::get_acceleration_type();
    let transcribe_msg = format!("⏳ Transcribing...\nBackend: whisper-cpp ({}) | Model: {}", acceleration, resolved_model);
    
    Command::new("notify-send")
        .args(&[
            "Voice Input",
            &transcribe_msg,
            "-t", "2000",
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .spawn()?;

    eprintln!("DEBUG: Connecting to daemon socket at: {}", socket_path);
    
    match socket::send_transcription_request(socket_path, &audio_file, "whisper-cpp", use_clipboard) {
        Ok(_) => {
            eprintln!("DEBUG: Total time: {:?}", start_time.elapsed());
            let _ = fs::remove_file(&audio_file);
        }
        Err(e) => {
            // Use the model parameter if provided, otherwise resolve from env
            let model = crate::helpers::resolve_model(model);
            
            let fallback_msg = if bindings {
                format!("⚠️ Daemon not running, using fallback\nBackend: whisper-cpp (bindings) | Model: {}", model)
            } else {
                format!("⚠️ Daemon not running, using fallback\nBackend: whisper-cpp (CLI) | Model: {}", model)
            };
            
            Command::new("notify-send")
                .args(&[
                    "Voice Input",
                    &fallback_msg,
                    "-t", "2000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
            
            // By default, fallback uses whisper-rs bindings (same as daemon)
            // With --no-bindings flag, it uses the CLI binary instead
            let result = if !bindings {
                // Use whisper-cpp CLI binary for fallback
                let whisper_path = whisper_path.unwrap_or_else(|| 
                    std::env::var("WHISPER_CPP_PATH").unwrap_or_else(|_| "whisper-cpp".to_string())
                );
                transcribe_with_cli(&audio_file, &model, &whisper_path, use_clipboard)
            } else {
                // Use whisper-rs bindings for fallback (default, same as daemon)
                transcribe_with_whisper_rs(&audio_file, &model, "", use_clipboard)
            };
            
            let _ = fs::remove_file(&audio_file);
            
            return result.map_err(|err| anyhow::anyhow!("Fallback transcription failed (daemon was: {}): {}", e, err));
        }
    }

    Ok(())
}