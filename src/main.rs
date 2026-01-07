use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{debug, Level};
use tracing_subscriber::FmtSubscriber;

mod tray;
mod helpers;
mod recording;
mod typing;
mod socket;
mod whisper_cpp;
mod faster_whisper;

#[derive(Parser)]
#[command(name = "whisp-away")]
#[command(about = "Simple dictation tool using whisper.cpp or faster-whisper", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start recording audio
    Start,
    
    /// Toggle recording: start if not recording, stop and transcribe if recording
    /// Configuration comes from WA_* environment variables or daemon config
    Toggle,
    
    /// Stop recording and transcribe
    /// Configuration comes from WA_* environment variables or daemon config
    Stop,
    
    /// Run as a daemon server with model preloaded
    /// Uses WA_WHISPER_BACKEND, WA_WHISPER_MODEL, WA_WHISPER_SOCKET, WA_USE_CLIPBOARD env vars
    Daemon,
    
    /// Run system tray icon for daemon control
    Tray,
}

fn main() -> Result<()> {
    // Initialize logging - quiet by default, use RUST_LOG=debug for verbose output
    let log_level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Level::WARN);
    
    // Use try_init to avoid panic if subscriber already set
    let _ = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .try_init();
    
    debug!("whisp-away starting");
    
    let cli = Cli::parse();

    match cli.command {
        Commands::Start => {
            debug!("Start command");
            recording::start_recording()
        }
        
        Commands::Toggle => {
            let backend = helpers::resolve_backend();
            debug!("Toggle command - backend: {}", backend);
            
            // Check if recording is in progress
            if recording::is_recording() {
                // Stop and transcribe
                debug!("Recording in progress, stopping and transcribing");
                let socket_path = helpers::resolve_socket_path();
                let use_clipboard = helpers::resolve_use_clipboard();
                
                match backend.as_str() {
                    "whisper-cpp" => {
                        whisper_cpp::stop_and_transcribe_daemon(&socket_path, None, None, true, None, use_clipboard)
                    }
                    "faster-whisper" => {
                        faster_whisper::stop_and_transcribe_daemon(&socket_path, use_clipboard)
                    }
                    _ => Err(anyhow::anyhow!("Unknown backend: {}", backend))
                }
            } else {
                // Start recording
                debug!("No recording in progress, starting");
                recording::start_recording()
            }
        }
        
        Commands::Stop => {
            let backend = helpers::resolve_backend();
            let socket_path = helpers::resolve_socket_path();
            let use_clipboard = helpers::resolve_use_clipboard();
            debug!("Stop command - backend: {}, socket: {}, clipboard: {}", 
                   backend, socket_path, use_clipboard);
            
            match backend.as_str() {
                "whisper-cpp" => {
                    whisper_cpp::stop_and_transcribe_daemon(&socket_path, None, None, true, None, use_clipboard)
                }
                "faster-whisper" => {
                    faster_whisper::stop_and_transcribe_daemon(&socket_path, use_clipboard)
                }
                _ => Err(anyhow::anyhow!("Unknown backend: {}", backend))
            }
        }
        
        Commands::Daemon => {
            let backend = helpers::resolve_backend();
            let model = helpers::resolve_model();
            let socket_path = helpers::resolve_socket_path();
            debug!("Daemon command - backend: {}, model: {}, socket: {}", 
                   backend, model, socket_path);
            
            match backend.as_str() {
                "whisper-cpp" => whisper_cpp::run_daemon(&model),
                "faster-whisper" => faster_whisper::run_daemon(&model, &socket_path),
                unknown => Err(anyhow::anyhow!("Unknown backend: {}", unknown)),
            }
        }
        
        Commands::Tray => {
            let backend = helpers::resolve_backend();
            debug!("Tray command - backend: {}", backend);
            tokio::runtime::Runtime::new()?.block_on(tray::run_tray(backend))
        }
    }
}