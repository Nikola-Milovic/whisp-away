use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
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

#[derive(Debug, Clone, ValueEnum)]
enum Backend {
    /// Use whisper.cpp backend
    #[value(name = "whisper-cpp", alias = "cpp")]
    WhisperCpp,
    /// Use faster-whisper backend
    #[value(name = "faster-whisper", alias = "faster")]
    FasterWhisper,
    /// Use the backend from WA_WHISPER_BACKEND env var (default: faster-whisper)
    #[value(name = "auto", alias = "default")]
    Auto,
}

#[derive(Subcommand)]
enum Commands {
    /// Start recording audio
    Start,
    
    /// Toggle recording: start if not recording, stop and transcribe if recording
    Toggle {
        /// Backend to use for transcription
        #[arg(short, long, default_value = "auto")]
        backend: Backend,
        
        /// Output to clipboard instead of typing at cursor
        #[arg(long)]
        clipboard: Option<bool>,
    },
    
    /// Stop recording and transcribe
    Stop {
        /// Backend to use for transcription
        #[arg(short, long, default_value = "auto")]
        backend: Backend,
        
        /// Use whisper-rs bindings for fallback (default: true, whisper-cpp only)
        #[arg(long, default_value_t = true)]
        bindings: bool,
        
        /// Model to use for transcription (overrides WA_WHISPER_MODEL env var)
        #[arg(short, long)]
        model: Option<String>,
        
        /// Optional audio file to transcribe (instead of recorded audio)
        #[arg(short, long)]
        audio_file: Option<String>,
        
        /// Unix socket path for daemon communication
        #[arg(long)]
        socket_path: Option<String>,
        
        /// Path to whisper.cpp binary (for whisper-cpp backend)
        #[arg(long)]
        whisper_path: Option<String>,
        
        /// Output to clipboard instead of typing at cursor (overrides tray/env setting)
        #[arg(long)]
        clipboard: Option<bool>,
    },
    
    /// Run as a daemon server with model preloaded
    Daemon {
        /// Backend to use
        #[arg(short, long, default_value = "auto")]
        backend: Backend,
        
        /// Model to use (overrides WA_WHISPER_MODEL env var)
        #[arg(short, long)]
        model: Option<String>,
        
        /// Unix socket path for daemon communication
        #[arg(long)]
        socket_path: Option<String>,
    },
    
    /// Run system tray icon for daemon control
    Tray {
        /// Backend to monitor
        #[arg(short, long, default_value = "auto")]
        backend: Backend,
    },
}

/// Resolves the backend to use
fn resolve_backend(backend: &Backend) -> String {
    match backend {
        Backend::WhisperCpp => "whisper-cpp".to_string(),
        Backend::FasterWhisper => "faster-whisper".to_string(),
        Backend::Auto => {
            // Use env var or default to faster-whisper
            std::env::var("WA_WHISPER_BACKEND").unwrap_or_else(|_| "faster-whisper".to_string())
        }
    }
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
        // New unified commands
        Commands::Start => {
            debug!("Start command");
            recording::start_recording()
        }
        
        Commands::Toggle { backend, clipboard } => {
            let resolved_backend = resolve_backend(&backend);
            debug!("Toggle command - resolved backend: {}", resolved_backend);
            
            // Check if recording is in progress
            if recording::is_recording() {
                // Stop and transcribe
                debug!("Recording in progress, stopping and transcribing");
                let socket_path = helpers::resolve_socket_path(None);
                let use_clipboard = helpers::resolve_use_clipboard(clipboard);
                
                match resolved_backend.as_str() {
                    "whisper-cpp" => {
                        whisper_cpp::stop_and_transcribe_daemon(&socket_path, None, None, true, None, use_clipboard)
                    }
                    "faster-whisper" => {
                        faster_whisper::stop_and_transcribe_daemon(&socket_path, use_clipboard)
                    }
                    _ => Err(anyhow::anyhow!("Unknown backend: {}", resolved_backend))
                }
            } else {
                // Start recording
                debug!("No recording in progress, starting");
                recording::start_recording()
            }
        }
        
        Commands::Stop { backend, bindings, model, audio_file, socket_path, whisper_path, clipboard } => {
            let resolved_backend = resolve_backend(&backend);
            debug!("Stop command - resolved backend: {}, bindings: {}, model: {:?}", 
                   resolved_backend, bindings, model);
            
            let socket_path = helpers::resolve_socket_path(socket_path);
            let use_clipboard = helpers::resolve_use_clipboard(clipboard);
            debug!("Socket path: {}, use_clipboard: {}", socket_path, use_clipboard);
            
            match resolved_backend.as_str() {
                "whisper-cpp" => {
                    whisper_cpp::stop_and_transcribe_daemon(&socket_path, audio_file.as_deref(), model, bindings, whisper_path, use_clipboard)
                }
                "faster-whisper" => {
                    faster_whisper::stop_and_transcribe_daemon(&socket_path, use_clipboard)
                }
                _ => Err(anyhow::anyhow!("Unknown backend: {}", resolved_backend))
            }
        }
        
        Commands::Daemon { backend, model, socket_path } => {
            let resolved_backend = resolve_backend(&backend);
            let model = helpers::resolve_model(model);
            let socket_path = helpers::resolve_socket_path(socket_path);
            debug!("Daemon command - backend: {}, model: {}, socket: {}", 
                   resolved_backend, model, socket_path);
            
            match resolved_backend.as_str() {
                "whisper-cpp" => whisper_cpp::run_daemon(&model),
                "faster-whisper" => faster_whisper::run_daemon(&model, &socket_path),
                unknown => Err(anyhow::anyhow!("Unknown backend: {}", unknown)),
            }
        }
        
        Commands::Tray { backend } => {
            let daemon_type = resolve_backend(&backend);
            debug!("Tray command - daemon type: {}", daemon_type);
            tokio::runtime::Runtime::new()?.block_on(tray::run_tray(daemon_type))
        }
    }
}