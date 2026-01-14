use anyhow::Result;
use ksni::{menu::StandardItem, Handle, MenuItem, Tray, TrayService};
use std::time::Duration;
use tracing::{debug, info};

/// Status information displayed by the tray
#[derive(Debug, Clone)]
struct TrayStatus {
    recording: bool,
    backend: String,
    model: String,
    acceleration: String,
}

impl Default for TrayStatus {
    fn default() -> Self {
        Self {
            recording: false,
            backend: crate::helpers::resolve_backend(),
            model: crate::helpers::resolve_model(),
            acceleration: crate::helpers::get_acceleration_type(),
        }
    }
}

#[derive(Debug)]
struct VoiceInputTray {
    status: TrayStatus,
}

impl VoiceInputTray {
    fn new() -> Self {
        Self {
            status: TrayStatus::default(),
        }
    }

    fn get_icon_name(&self) -> String {
        if self.status.recording {
            // Full/active microphone - recording in progress
            "microphone-sensitivity-high-symbolic"
        } else {
            // Empty/inactive microphone - not recording
            "microphone-sensitivity-muted-symbolic"
        }
        .to_string()
    }

    fn get_tooltip(&self) -> String {
        let backend_display = match self.status.backend.as_str() {
            "faster-whisper" => "Faster Whisper",
            "whisper-cpp" => "Whisper.cpp",
            other => other,
        };

        if self.status.recording {
            format!(
                "Voice Input - 🎙️ Recording...\n\nBackend: {}\nModel: {}\nAcceleration: {}",
                backend_display,
                self.status.model,
                self.status.acceleration.to_uppercase()
            )
        } else {
            format!(
                "Voice Input - Ready\n\nBackend: {}\nModel: {}\nAcceleration: {}",
                backend_display,
                self.status.model,
                self.status.acceleration.to_uppercase()
            )
        }
    }
    
    fn get_backend_display(&self) -> &str {
        match self.status.backend.as_str() {
            "faster-whisper" => "Faster Whisper",
            "whisper-cpp" => "Whisper.cpp",
            other => other,
        }
    }
}

impl Tray for VoiceInputTray {
    fn id(&self) -> String {
        "whisp-away-indicator".to_string()
    }

    fn title(&self) -> String {
        "Voice Input".to_string()
    }

    fn icon_name(&self) -> String {
        self.get_icon_name()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: self.get_tooltip(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            // Recording status indicator
            MenuItem::Standard(StandardItem {
                label: if self.status.recording {
                    "🎙️ Recording in progress".to_string()
                } else {
                    "⏸️ Not recording".to_string()
                },
                enabled: false,
                ..Default::default()
            }),
            MenuItem::Separator,
            // Backend info
            MenuItem::Standard(StandardItem {
                label: format!("Backend: {}", self.get_backend_display()),
                enabled: false,
                ..Default::default()
            }),
            // Model info
            MenuItem::Standard(StandardItem {
                label: format!("Model: {}", self.status.model),
                enabled: false,
                ..Default::default()
            }),
            // Acceleration info
            MenuItem::Standard(StandardItem {
                label: format!("Acceleration: {}", self.status.acceleration.to_uppercase()),
                enabled: false,
                ..Default::default()
            }),
            MenuItem::Separator,
            // Quit option
            MenuItem::Standard(StandardItem {
                label: "Quit Indicator".to_string(),
                activate: Box::new(|_tray: &mut Self| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }),
        ]
    }
}

/// Spawns a background thread that polls recording status and updates the tray
fn spawn_status_poller(handle: Handle<VoiceInputTray>) {
    std::thread::spawn(move || {
        let mut last_recording_state = false;
        info!("Polling thread started");
        
        loop {
            let is_recording = crate::recording::is_recording();
            
            // Only update when state changes to avoid unnecessary updates
            if is_recording != last_recording_state {
                info!("Recording state changed: {} -> {}", last_recording_state, is_recording);
                last_recording_state = is_recording;
                
                // Update the tray through the handle - this triggers a refresh
                handle.update(|tray| {
                    tray.status.recording = is_recording;
                    tray.status.backend = crate::helpers::resolve_backend();
                    tray.status.model = crate::helpers::resolve_model();
                    tray.status.acceleration = crate::helpers::get_acceleration_type();
                    debug!("Tray updated: recording={}", is_recording);
                });
            }

            // Poll every 200ms for responsive updates
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

pub async fn run_tray(_daemon_type: String) -> Result<()> {
    info!("Starting tray indicator...");
    
    let tray = VoiceInputTray::new();
    
    info!("Initial status - backend: {}, model: {}, acceleration: {}", 
          tray.status.backend, tray.status.model, tray.status.acceleration);

    // Create the tray service
    info!("Creating tray service...");
    let service = TrayService::new(tray);
    
    // Get a handle to update the tray from the polling thread
    let handle = service.handle();
    
    // Spawn the background polling thread
    info!("Spawning recording status polling thread...");
    spawn_status_poller(handle);

    // Run the tray service (this blocks)
    info!("Running tray service (this blocks)");
    service.run();

    Ok(())
}
