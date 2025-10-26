use anyhow::{anyhow, Context, Result};
use std::fs;
use std::process::Command;
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use crate::helpers::wav_to_samples;
use crate::typing;

/// Core transcription function using whisper-rs library
pub fn transcribe_audio(audio_file: &str, model: &str) -> Result<String> {
    let total_start = std::time::Instant::now();
    
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/martin".to_string());
    let model_extension = if model.ends_with(".bin") { "" } else { ".bin" };
    let model_path = format!("{}/.cache/whisper-cpp/models/ggml-{}{}", home, model, model_extension);
    
    if !std::path::Path::new(&model_path).exists() {
        return Err(anyhow::anyhow!("Model file not found: {}", model_path));
    }
    
    let t1 = std::time::Instant::now();
    let audio_data = fs::read(audio_file)
        .context("Failed to read audio file")?;
    eprintln!("DEBUG FALLBACK: Audio file read took {:?}", t1.elapsed());
    
    let t2 = std::time::Instant::now();
    let samples = wav_to_samples(&audio_data)?;
    eprintln!("DEBUG FALLBACK: WAV conversion took {:?}", t2.elapsed());
    
    eprintln!("DEBUG FALLBACK: Starting whisper-rs transcription for file: {}", audio_file);
    eprintln!("DEBUG FALLBACK: Model path: {}", model_path);
    eprintln!("DEBUG FALLBACK: Audio samples: {} samples", samples.len());
    
    let mut ctx_params = WhisperContextParameters::default();
    ctx_params.use_gpu(true);
    ctx_params.gpu_device(0);
    
    eprintln!("DEBUG FALLBACK: Creating WhisperContext with GPU enabled...");
    let t3 = std::time::Instant::now();
    let ctx = WhisperContext::new_with_params(&model_path, ctx_params)
        .context("Failed to create WhisperContext")?;
    eprintln!("DEBUG FALLBACK: WhisperContext creation took {:?}", t3.elapsed());
    
    eprintln!("DEBUG FALLBACK: Creating whisper state...");
    let t4 = std::time::Instant::now();
    let mut state = ctx.create_state()
        .context("Failed to create whisper state")?;
    eprintln!("DEBUG FALLBACK: State creation took {:?}", t4.elapsed());
    
    // Initialize OpenVINO at STATE level (fallback pattern that was working at 1.6s)
    #[cfg(feature = "openvino")]
    {
        eprintln!("DEBUG FALLBACK: Initializing OpenVINO encoder at STATE level...");
        let t5 = std::time::Instant::now();
        // Check if OpenVINO model files exist
        let model_base = model_path.trim_end_matches(".bin");
        let openvino_model = format!("{}-encoder-openvino.xml", model_base);
        
        if std::path::Path::new(&openvino_model).exists() {
            eprintln!("DEBUG FALLBACK: Found OpenVINO model: {}", openvino_model);
            // Set cache directory as subdirectory next to the model files
            let cache_dir = format!("{}-encoder-openvino-cache", model_base);
            // Ensure cache directory exists
            if let Err(e) = std::fs::create_dir_all(&cache_dir) {
                eprintln!("DEBUG FALLBACK: Warning: Could not create cache dir: {:?}", e);
            }
            eprintln!("DEBUG FALLBACK: Using cache dir: {}", cache_dir);
            // Use AUTO to let OpenVINO choose the best available device
            if let Err(e) = state.init_openvino_encoder_state_level(None, "AUTO", Some(&cache_dir)) {
                eprintln!("DEBUG FALLBACK: AUTO device selection failed: {:?}, trying CPU...", e);
                if let Err(e) = state.init_openvino_encoder_state_level(None, "CPU", Some(&cache_dir)) {
                    eprintln!("DEBUG FALLBACK: CPU initialization also failed: {:?}", e);
                    eprintln!("DEBUG FALLBACK: Will use regular CPU inference without OpenVINO");
                } else {
                    eprintln!("DEBUG FALLBACK: OpenVINO initialized with CPU");
                }
            } else {
                eprintln!("DEBUG FALLBACK: OpenVINO initialized with AUTO device selection");
            }
            eprintln!("DEBUG FALLBACK: OpenVINO initialization took {:?}", t5.elapsed());
        } else {
            eprintln!("DEBUG FALLBACK: OpenVINO model not found at {}, using regular CPU", openvino_model);
        }
    }
    
    let t6 = std::time::Instant::now();
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    
    // Match the native CLI's thread count more closely
    let num_threads = 4;  // Try with 4 threads like CLI default
    params.set_n_threads(num_threads);
    eprintln!("DEBUG FALLBACK: Using {} threads (forced to 4 to match CLI)", num_threads);
    
    params.set_translate(false);
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_temperature(0.0);
    eprintln!("DEBUG FALLBACK: Param setup took {:?}", t6.elapsed());
    
    eprintln!("DEBUG FALLBACK: Starting transcription...");
    let t7 = std::time::Instant::now();
    state.full(params, &samples)
        .context("Failed to transcribe audio")?;
    eprintln!("DEBUG FALLBACK: Whisper transcription (state.full) took {:?}", t7.elapsed());
    
    let t8 = std::time::Instant::now();
    let mut transcribed_text = String::new();
    let num_segments = state.full_n_segments();
    for i in 0..num_segments {
        let segment = state.get_segment(i)
            .ok_or_else(|| anyhow!("Failed to get segment {}", i))?;
        let segment_text = segment.to_str()?;
        eprintln!("DEBUG FALLBACK: Segment: {:?}", segment_text);
        transcribed_text.push_str(segment_text);
        transcribed_text.push(' ');
    }
    eprintln!("DEBUG FALLBACK: Segment extraction took {:?}", t8.elapsed());
    
    let clean_text = transcribed_text.trim().to_string();
    eprintln!("DEBUG FALLBACK: Final transcription: {:?}", clean_text);
    eprintln!("DEBUG FALLBACK: TOTAL TIME: {:?}", total_start.elapsed());
    
    Ok(clean_text)
}


/// Transcribe audio using whisper-cpp CLI binary
pub fn transcribe_with_cli(audio_file: &str, model: &str, whisper_path: &str, use_clipboard: bool) -> Result<()> {
    let acceleration = crate::helpers::get_acceleration_type();
    let transcribe_msg = format!("⏳ Transcribing with CLI... ({})", acceleration);
    
    Command::new("notify-send")
        .args(&[
            "Voice Input (whisper.cpp)",
            &transcribe_msg,
            "-t", "2000",
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .spawn()?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/martin".to_string());
    let model_extension = if model.ends_with(".bin") { "" } else { ".bin" };
    let model_path = format!("{}/.cache/whisper-cpp/models/ggml-{}{}", home, model, model_extension);
    
    let output = Command::new(whisper_path)
        .args(&[
            "-m", &model_path,
            "-f", audio_file,
            "-t", "8",
            "-np",
            "-nt"
        ])
        .output()
        .context("Failed to run whisper-cpp")?;

    if !output.status.success() {
        Command::new("notify-send")
            .args(&[
                "Voice Input (whisper.cpp)",
                "❌ Transcription failed",
                "-t", "2000",
                "-h", "string:x-canonical-private-synchronous:voice"
            ])
            .spawn()?;
        return Err(anyhow!("whisper-cpp failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let stdout_text = String::from_utf8_lossy(&output.stdout);
    let mut result = String::new();
    
    for line in stdout_text.lines() {
        if line.contains(" --> ") && line.contains("]") {
            if let Some(end_bracket) = line.rfind(']') {
                let text = &line[end_bracket + 1..].trim();
                if !text.is_empty() && !text.starts_with("(") && !text.ends_with(")") {
                    if !result.is_empty() {
                        result.push(' ');
                    }
                    result.push_str(text);
                }
            }
        }
    }

    typing::output_text(result.trim(), use_clipboard, "whisper-cpp CLI")?;
    Ok(())
}

/// Transcribe audio from file and type the result using wtype
pub fn transcribe_with_whisper_rs(audio_file: &str, model: &str, _whisper_path: &str, use_clipboard: bool) -> Result<()> {
    let acceleration = crate::helpers::get_acceleration_type();
    let transcribe_msg = format!("⏳ Transcribing with GPU... ({})", acceleration);
    
    Command::new("notify-send")
        .args(&[
            "Voice Input (whisper.cpp)",
            &transcribe_msg,
            "-t", "2000",
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .spawn()?;

    match transcribe_audio(audio_file, model) {
        Ok(clean_text) => {
        typing::output_text(&clean_text, use_clipboard, "whisper-cpp")?;
            Ok(())
        }
        Err(e) => {
            Command::new("notify-send")
                .args(&[
                    "Voice Input (whisper.cpp)",
                    "❌ Model file not found",
                    "-t", "2000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
            Err(e)
        }
    }
}