use anyhow::{Context, Result};
use std::process::Command;
use crate::typing;

/// Transcribe audio with faster-whisper and type the result
pub fn transcribe_with_faster_whisper(audio_file: &str, model: &str, use_clipboard: bool) -> Result<()> {
    let acceleration = crate::helpers::get_acceleration_type();
    let transcribe_msg = format!("⏳ Transcribing... ({})", acceleration);
    
    Command::new("notify-send")
        .args(&[
            "Voice Input (faster-whisper)",
            &transcribe_msg,
            "-t", "2000",
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .spawn()?;

    let python_path = std::env::var("FASTER_WHISPER_PYTHON")
        .unwrap_or_else(|_| "python3".to_string());
    let pythonpath = std::env::var("FASTER_WHISPER_PYTHONPATH")
        .unwrap_or_else(|_| "".to_string());
    let script_path = std::env::var("FASTER_WHISPER_SCRIPT")
        .unwrap_or_else(|_| "/run/current-system/sw/bin/transcribe_faster.py".to_string());
    
    let output = Command::new(&python_path)
        .arg(&script_path)
        .args(&[audio_file, model])
        .env("PYTHONPATH", &pythonpath)
        .env("CUDA_VISIBLE_DEVICES", std::env::var("CUDA_VISIBLE_DEVICES").unwrap_or_default())
        .env("LD_LIBRARY_PATH", std::env::var("LD_LIBRARY_PATH").unwrap_or_default())
        .output()
        .context("Failed to run faster-whisper transcription")?;
    
    let transcribed_text = String::from_utf8_lossy(&output.stdout);

    if output.status.success() {
        let clean_text = transcribed_text.trim();
        
        typing::output_text(&clean_text, use_clipboard, "faster-whisper")?;
    } else {
        Command::new("notify-send")
            .args(&[
                "Voice Input (faster-whisper)",
                "❌ Transcription failed",
                "-t", "2000",
                "-h", "string:x-canonical-private-synchronous:voice"
            ])
            .spawn()?;
        return Err(anyhow::anyhow!("Transcription failed"));
    }

    Ok(())
}