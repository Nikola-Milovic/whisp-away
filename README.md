# WhispAway

Voice dictation for Linux using OpenAI's Whisper models. Type with your voice using local speech recognition - no cloud services required.

## Features

- **Flexible Output**: Instant typing at cursor or copy to clipboard
- **Dual Backends**: Choose between `whisper.cpp` or `faster-whisper`
- **Hardware Acceleration**: CUDA, Vulkan, OpenVINO, and CPU support
- **Model Preloading**: Daemon mode keeps models in memory for instant transcription
- **System Tray Indicator**: Shows recording status, backend info on hover
- **NixOS Integration**: First-class NixOS and Home Manager support
- **Single Instance**: Only one recording at a time, automatic cleanup of old files

## Installation

### NixOS / Home Manager

```nix
{
  # With NixOS
  imports = [ whisp-away.nixosModules.nixos ];
  # With home-manager
  imports = [ whisp-away.nixosModules.home-manager ];
  
  services.whisp-away = {
    enable = true;
    accelerationType = "vulkan";  # or "cuda", "openvino", "cpu" - requires rebuild
    defaultBackend = "faster-whisper";  # or "whisper-cpp"
    defaultModel = "base.en";     # Model to use
    useClipboard = false;         # true = copy to clipboard, false = type at cursor
    
    # Optional: Auto-start services on login
    autoStartDaemon = true;       # Keep model preloaded for fast transcription
    autoStartTray = true;         # System tray for control
  };
}
```

## Usage

### Keybinds (Recommended)

Configure in your window manager. Two options:

**Option 1: Single key toggle** (simplest)

```conf
# Hyprland - press once to start, press again to stop and transcribe
bind = ,section,exec, whisp-away toggle

# With clipboard output:
bind = ,section,exec, whisp-away toggle --clipboard true
```

**Option 2: Push-to-talk** (hold to record)

```conf
# Hyprland - hold key to record, release to transcribe
bind = ,section,exec, whisp-away start
bindr = ,section,exec, whisp-away stop

# Sway equivalent:
bindsym section exec whisp-away start
bindsym --release section exec whisp-away stop
```

Note: `section` is the § key on Swedish keyboards (top-left, below Esc). Replace with your preferred key.

### System Tray

The tray icon shows recording status at a glance:

- **Icon**: Changes to indicate recording state (active mic = recording, muted mic = idle)
- **Hover**: Shows backend, model, and acceleration info
- **Right-click menu**: Displays current status (informational only)

Start manually if not using `autoStartTray`:

```bash
whisp-away tray
```

The tray is purely informational - use keybinds to control recording.

### Daemon Mode

Keep the model preloaded for instant transcription:

```bash
# Start daemon (or use autoStartDaemon = true in NixOS config)
whisp-away daemon

# In another terminal, use start/stop as normal
whisp-away start   # Begin recording
whisp-away stop    # Stop and transcribe (instant with daemon)
```

### Command Line

```bash
# Toggle mode - single command for start/stop
whisp-away toggle             # Start if not recording, stop if recording
whisp-away toggle --clipboard true  # Same but output to clipboard

# Separate start/stop commands
whisp-away start              # Start recording
whisp-away stop               # Stop and transcribe

# Specify model, backend, or output mode
whisp-away stop --model medium.en
whisp-away stop --backend faster-whisper
whisp-away stop --clipboard true     # Copy to clipboard instead of typing

# Transcribe an existing audio file
whisp-away stop --audio-file recording.wav
```

## Models & Performance

| Model | Size | Speed | Quality | Use Case |
|-------|------|-------|---------|----------|
| **tiny.en** | 39 MB | Instant | Basic | Quick notes, testing |
| **base.en** | 74 MB | Fast | Good | Casual dictation |
| **small.en** | 244 MB | Moderate | Better | Daily use (recommended) |
| **medium.en** | 769 MB | Slow | Excellent | Professional transcription |
| **large-v3** | 1550 MB | Slowest | Best | Maximum accuracy, multilingual |

Models download automatically on first use and are stored in:

- `~/.cache/whisper-cpp/models/` (GGML models for whisper.cpp)
- `~/.cache/faster-whisper/` (CTranslate2 models for faster-whisper)

## Hardware Acceleration

| Type | Backend Support | Hardware |
|------|----------------|----------|
| **vulkan** | whisper.cpp | Most GPUs (AMD, NVIDIA, Intel) |
| **cuda** | Both backends | NVIDIA GPUs only |
| **openvino** | whisper.cpp | Intel GPUs and CPUs |
| **cpu** | Both backends | Any CPU (slow) |

**Note**: `faster-whisper` only supports CUDA and CPU. The `whisper.cpp` backend supports all acceleration types.

## Configuration

### NixOS Module Options

```nix
services.whisp-away = {
  enable = true;
  defaultModel = "small.en";        # sets WA_WHISPER_MODEL
  defaultBackend = "faster-whisper"; # sets WA_WHISPER_BACKEND
  accelerationType = "vulkan";      # GPU acceleration type
  useClipboard = false;             # sets WA_USE_CLIPBOARD
  autoStartDaemon = true;           # Start daemon on login
  autoStartTray = true;             # Start tray on login
};
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `WA_WHISPER_MODEL` | Model to use | `base.en` |
| `WA_WHISPER_BACKEND` | Backend (`whisper-cpp` or `faster-whisper`) | `faster-whisper` |
| `WA_USE_CLIPBOARD` | Output mode (`true`/`false`) | `false` |
| `RUST_LOG` | Log level (`warn`, `info`, `debug`, `trace`) | `warn` |
| `WHISPER_VAD` | Enable VAD filter (`true`/`false`) | `true` |

## Troubleshooting

### Debug Mode

Enable verbose logging to diagnose issues:

```bash
RUST_LOG=debug whisp-away start
RUST_LOG=debug whisp-away stop
```

### No Speech Detected?

The VAD (Voice Activity Detection) filter may be too aggressive. Try:

1. **Check your microphone** - Record and play back to verify:

   ```bash
   pw-record /tmp/test.wav
   # Speak for a few seconds, then Ctrl+C
   pw-play /tmp/test.wav
   ```

2. **Check the right input device is selected** using `pavucontrol` or your desktop audio settings

3. **Disable VAD temporarily** to test:

   ```bash
   WHISPER_VAD=false whisp-away daemon
   ```

### Tray Icon Doesn't Appear?

- Ensure you have a system tray (GNOME needs an extension like AppIndicator)
- Check if the app is running: `ps aux | grep whisp-away`
- Try starting manually: `whisp-away tray`

### Transcription is Slow?

- Use a smaller model (`tiny.en` or `base.en`)
- Enable GPU acceleration if available
- Use daemon mode (`autoStartDaemon = true`) to keep the model preloaded

### No Text Appears After Recording?

- Check the notification for errors
- For typing mode (Wayland): Verify `wtype` is installed
- For typing mode (X11): Verify `xdotool` is installed
- For clipboard mode: Verify `wl-copy` (Wayland) or `xclip` (X11)
- Try toggling output mode: `whisp-away stop --clipboard true`

### Recording Issues?

- Ensure PipeWire is running: `systemctl --user status pipewire`
- Check audio permissions and that your user is in the `audio` group
- Test recording directly: `pw-record --channels 1 --rate 16000 /tmp/test.wav`

## Building from Source

### With Nix

```bash
nix build        # Build with default settings
nix develop      # Enter development shell
```

### With Cargo

```bash
cargo build --release --features vulkan
```

## Development

### Setup Git Hooks

Install pre-push hooks to catch dependency hash mismatches before pushing:

```bash
./scripts/setup-git-hooks.sh
```

### Updating Dependencies

The `whisper-rs` dependency is pinned to a specific commit in `Cargo.toml` for reproducible builds.

**To update to a newer version:**

```bash
# 1. Find the latest commit from the whisp-away branch
git ls-remote https://codeberg.org/madjinn/whisper-rs.git whisp-away

# 2. Update the rev in Cargo.toml to the new commit hash

# 3. Update Cargo.lock
cargo update -p whisper-rs

# 4. Update Nix hashes
./scripts/update-git-deps.sh

# 5. Commit all changes
git add Cargo.toml Cargo.lock git-deps.nix
git commit -m "chore: update whisper-rs to <new-commit>"
```

The pre-push hook will verify hashes are correct before pushing.

## TODO

nothing yet

## Project Status

This project is actively maintained and primarily tested on NixOS. Contributions are welcome!

## License

MIT License

## Credits

- [OpenAI Whisper](https://github.com/openai/whisper) - Original speech recognition models
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) - C++ implementation
- [faster-whisper](https://github.com/guillaumekln/faster-whisper) - CTranslate2 optimized implementation
- [whisper-rs](https://github.com/tazz4843/whisper-rs) - Rust bindings
- [madjinn's implementation](https://github.com/madjinn/whisp-away) - original implementation, this repo is a fork of `madjinn's` implementation.

