#!/usr/bin/env python3
"""
Transcribe audio using faster-whisper.
Used by voice-input-rust for the start-faster/stop-faster commands.
"""

import sys
import os
from faster_whisper import WhisperModel

def main():
    if len(sys.argv) < 3:
        print("Usage: transcribe_faster.py <audio_file> <model>", file=sys.stderr)
        sys.exit(1)
    
    audio_file = sys.argv[1]
    model_name = sys.argv[2]
    
    # Check if audio file exists
    if not os.path.exists(audio_file):
        print(f"Error: Audio file not found: {audio_file}", file=sys.stderr)
        sys.exit(1)
    
    # Determine device and compute type
    device = os.environ.get('WHISPER_DEVICE', 'auto')
    compute_type = os.environ.get('WHISPER_COMPUTE', 'auto')
    
    # Auto-detect CUDA
    if device == 'auto':
        try:
            import ctranslate2
            cuda_types = ctranslate2.get_supported_compute_types('cuda')
            device = 'cuda'
        except:
            device = 'cpu'
    
    if compute_type == 'auto':
        compute_type = 'float16' if device == 'cuda' else 'int8'
    
    # Load model
    cache_dir = os.path.expanduser('~/.cache/faster-whisper')
    os.makedirs(cache_dir, exist_ok=True)
    
    try:
        model = WhisperModel(
            model_name,
            device=device,
            compute_type=compute_type,
            download_root=cache_dir
        )
        
        # Check if VAD should be disabled (for debugging)
        use_vad = os.environ.get("WHISPER_VAD", "true").lower() != "false"
        
        transcribe_kwargs = dict(
            language='en',
            beam_size=5,
        )
        
        if use_vad:
            transcribe_kwargs["vad_filter"] = True
            transcribe_kwargs["vad_parameters"] = dict(
                min_silence_duration_ms=300,
                speech_pad_ms=200,  # More padding around speech
                threshold=0.3,  # Lower threshold = more sensitive
            )
        
        # Transcribe
        segments, info = model.transcribe(audio_file, **transcribe_kwargs)
        
        # Output transcribed text
        text = ' '.join(segment.text.strip() for segment in segments)
        if text:
            print(text)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()