use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::io::Write;
use tracing::debug;
use crate::helpers;

/// Output transcribed text to clipboard or type at cursor
pub fn output_text(text: &str, use_clipboard: bool, backend_name: &str) -> Result<()> {
    debug!("output_text called: text='{}', use_clipboard={}, backend={}", 
           if text.len() > 50 { &text[..50] } else { text },
           use_clipboard, backend_name);
    
    if text.trim().is_empty() {
        debug!("No speech detected (empty text received)");
        helpers::send_notification(
            "Voice Input",
            &format!("⚠️ No speech detected\nBackend: {}", backend_name),
            2000
        );
        return Ok(());
    }

    if use_clipboard {
        debug!("Copying to clipboard ({} chars)", text.trim().len());
        copy_to_clipboard(text.trim())?;
        
        helpers::send_notification(
            "Voice Input",
            &format!("✅ Copied to clipboard\nBackend: {}", backend_name),
            1000
        );
    } else {
        debug!("Typing at cursor ({} chars)", text.trim().len());
        // Small delay before typing
        std::thread::sleep(std::time::Duration::from_millis(30));
        
        type_at_cursor(text.trim(), backend_name)?;
    }

    Ok(())
}

/// Type text at cursor using wtype (Wayland) or xdotool (X11)
fn type_at_cursor(text: &str, backend_name: &str) -> Result<()> {
    debug!("Attempting to type at cursor using wtype (Wayland)");
    
    // Try wtype first (Wayland)
    let wtype_result = Command::new("wtype")
        .arg(text)
        .spawn()
        .and_then(|mut child| child.wait());
    
    if let Ok(status) = wtype_result {
        if status.success() {
            debug!("Successfully typed using wtype");
            helpers::send_notification(
                "Voice Input",
                &format!("✅ Transcribed\nBackend: {}", backend_name),
                1000
            );
            return Ok(());
        }
        debug!("wtype failed with status: {}", status);
    }
    
    debug!("Falling back to xdotool (X11)");
    
    // Fallback to xdotool (X11)
    Command::new("xdotool")
        .args(["type", "--clearmodifiers", "--", text])
        .spawn()
        .context("Failed to run typing command (tried wtype and xdotool)")?
        .wait()?;
    
    debug!("Successfully typed using xdotool");
    helpers::send_notification(
        "Voice Input",
        &format!("✅ Transcribed\nBackend: {}", backend_name),
        1000
    );
    
    Ok(())
}

/// Copy text to clipboard using wl-copy (Wayland) or xclip (X11)
fn copy_to_clipboard(text: &str) -> Result<()> {
    // Try wl-copy first (Wayland)
    let wl_copy_result = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
                drop(stdin);
            }
            child.wait()
        });
    
    if let Ok(status) = wl_copy_result {
        if status.success() {
            return Ok(());
        }
    }
    
    // Fallback to xclip (X11)
    let mut child = Command::new("xclip")
        .args(&["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to run clipboard command (tried wl-copy and xclip)")?;
    
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
        drop(stdin);
    }
    
    child.wait()
        .context("Clipboard command failed")?;
    
    Ok(())
}

/// Legacy function for backwards compatibility - uses typing mode
pub fn type_text(text: &str, backend_name: &str) -> Result<()> {
    output_text(text, false, backend_name)
}