# Sounds

`web/assets/sounds/` holds the machine's sounds, cut from a recording of a real typewriter:
six key clacks, the space bar, the margin bell and the carriage return. `build_sounds.py`
generates them; see that file for how each cue is chosen.

Each file is raw mono PCM: signed 16-bit little-endian samples at 44.1 kHz. `web/src/sound.rs`
embeds them in the WebAssembly binary.

## Regenerate

Requires Python 3.10 or later, ffmpeg, and the source recording (`typewriter-sound.mp3`,
1 h 47 min). The recording is not in the repository.

```bash
python3 -m venv .venv-sounds && .venv-sounds/bin/pip install librosa==0.11.0
```

```bash
.venv-sounds/bin/python sounds/build_sounds.py path/to/typewriter-sound.mp3
```

The bell and carriage return positions are pinned to this recording. A different recording
needs new positions in `build_sounds.py`.
