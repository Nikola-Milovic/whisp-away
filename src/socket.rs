use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use tracing::{debug, warn};
use crate::typing;
use crate::helpers;

/// Send a transcription request to the daemon via Unix socket
pub fn send_transcription_request(
    socket_path: &str,
    audio_file: &str,
    backend_name: &str,
    use_clipboard: bool,
) -> Result<()> {
    debug!("Connecting to daemon at {}", socket_path);
    
    match UnixStream::connect(socket_path) {
        Ok(mut stream) => {
            debug!("Connected to daemon, sending transcription request for: {}", audio_file);
            
            // Send request
            let request = format!(r#"{{"audio_path": "{}"}}"#, audio_file);
            debug!("Sending request: {}", request);
            stream.write_all(request.as_bytes())
                .context("Failed to send request to daemon")?;
            
            // Read response
            let mut response = String::new();
            stream.read_to_string(&mut response)
                .context("Failed to read response from daemon")?;
            
            debug!("Received response: {}", response);
            
            // Check if transcription was successful
            let success = response.contains(r#""success":true"#) || response.contains(r#""success": true"#);
            
            if success {
                // Parse the transcribed text from JSON response
                let text = extract_text_from_response(&response);
                
                if let Some(transcribed_text) = text {
                    debug!("Transcription result: '{}' ({} chars)", 
                          if transcribed_text.len() > 50 { &transcribed_text[..50] } else { &transcribed_text },
                          transcribed_text.len());
                    typing::output_text(transcribed_text.trim(), use_clipboard, &format!("{} daemon", backend_name))?;
                } else {
                    debug!("Could not parse text from response");
                    helpers::send_notification(
                        "Voice Input",
                        &format!("⚠️ Could not parse response\nBackend: {}", backend_name),
                        2000
                    );
                }
            } else {
                warn!("Transcription failed, response: {}", response);
                helpers::send_notification(
                    "Voice Input",
                    &format!("❌ Transcription failed\nBackend: {}", backend_name),
                    2000
                );
            }
            
            Ok(())
        }
        Err(e) => {
            debug!("Failed to connect to daemon: {}", e);
            // Return the error so the caller can handle fallback logic
            Err(anyhow::anyhow!("Failed to connect to daemon: {}", e))
        }
    }
}

/// Extract the "text" field value from a JSON response string
fn extract_text_from_response(response: &str) -> Option<String> {
    if let Some(text_start_idx) = response.find(r#""text":"#) {
        let after_text = &response[text_start_idx + 7..];
        let content_start = after_text.trim_start();
        
        if content_start.starts_with('"') {
            let text_content = &content_start[1..];
            if let Some(end_quote) = text_content.find('"') {
                Some(text_content[..end_quote].to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}