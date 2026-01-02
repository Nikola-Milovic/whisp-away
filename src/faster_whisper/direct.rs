use anyhow::{Context, Result};
use std::process::Command;
use tracing::{debug, warn};
use crate::typing;
use crate::helpers;

/// Transcribe audio with faster-whisper and type the result
pub fn transcribe_with_faster_whisper(audio_file: &str, model: &str, use_clipboard: bool) -> Result<()> {
    debug!("Direct transcription with faster-whisper, model: {}, audio: {}", model, audio_file);
    
    let acceleration = helpers::get_acceleration_type();
    let transcribe_msg = format!("⏳ Transcribing... ({})", acceleration);
    
    helpers::send_notification("Voice Input (faster-whisper)", &transcribe_msg, 2000);

    let python_path = std::env::var("FASTER_WHISPER_PYTHON")
        .unwrap_or_else(|_| "python3".to_string());
    let pythonpath = std::env::var("FASTER_WHISPER_PYTHONPATH")
        .unwrap_or_else(|_| "".to_string());
    let script_path = std::env::var("FASTER_WHISPER_SCRIPT")
        .unwrap_or_else(|_| "/run/current-system/sw/bin/transcribe_faster.py".to_string());
    
    debug!("Python path: {}", python_path);
    debug!("Script path: {}", script_path);
    debug!("PYTHONPATH: {}", pythonpath);
    
    let output = Command::new(&python_path)
        .arg(&script_path)
        .args([audio_file, model])
        .env("PYTHONPATH", &pythonpath)
        .env("CUDA_VISIBLE_DEVICES", std::env::var("CUDA_VISIBLE_DEVICES").unwrap_or_default())
        .env("LD_LIBRARY_PATH", std::env::var("LD_LIBRARY_PATH").unwrap_or_default())
        .output()
        .context("Failed to run faster-whisper transcription")?;
    
    let transcribed_text = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    debug!("Exit status: {}", output.status);
    debug!("Stdout: '{}'", transcribed_text);
    if !stderr.is_empty() {
        debug!("Stderr: '{}'", stderr);
    }

    if output.status.success() {
        let clean_text = transcribed_text.trim();
        debug!("Transcription result: '{}' ({} chars)", 
              if clean_text.len() > 50 { &clean_text[..50] } else { clean_text },
              clean_text.len());
        
        typing::output_text(clean_text, use_clipboard, "faster-whisper")?;
    } else {
        warn!("Transcription failed. Exit code: {:?}, stderr: {}", output.status.code(), stderr);
        helpers::send_notification(
            "Voice Input (faster-whisper)",
            &format!("❌ Transcription failed\n{}", 
                     if stderr.len() > 100 { &stderr[..100] } else { &stderr }),
            3000
        );
        return Err(anyhow::anyhow!("Transcription failed: {}", stderr));
    }

    Ok(())
}