"""Builds web/assets/sounds/*.pcm: the machine's sounds, cut from a recording of a typewriter.

Run from the repository root, in a virtualenv with `pip install librosa==0.11.0`, with ffmpeg
on the PATH:

    python sounds/build_sounds.py path/to/typewriter-sound.mp3

Each output file is raw mono PCM: signed 16-bit little-endian samples at 44.1 kHz, which
web/src/sound.rs embeds and copies straight into Web Audio buffers.

The recording is irregular typing with no steady pulse, so cues are found by onset detection,
not beat tracking. Key clacks are the cleanest isolated strikes in the whole recording. The bell
and the carriage return are rare, so their positions are pinned below; they were found by
inspecting spectrograms (the bell is a sustained tone near 3.2 kHz, the return a broadband
slide followed by a ratchet and a stop knock).
"""

import subprocess
import sys
from pathlib import Path

import librosa
import numpy as np
from scipy import signal

RATE = 44_100
OUT = Path("web/assets/sounds")

CLACKS = 6
CLACK_SECONDS = 0.16
PRE_ATTACK_SECONDS = 0.002
# A strike counts as isolated when no other onset falls this close before or after it.
GAP_BEFORE_SECONDS = 0.25
GAP_AFTER_SECONDS = 0.3
# Peak over the noise just before the strike, and over its own tail 150-250 ms later.
MIN_SNR_BEFORE = 40.0
MIN_SNR_AFTER = 20.0
# Typebar strikes on the platen are bright; duller isolated strikes are space bar candidates.
BRIGHT_CENTROID_HZ = 6_000.0

BELL_AT_SECONDS = 1_314.24
BELL_SECONDS = 1.4
RETURN_AT_SECONDS = 1_650.72
RETURN_SECONDS = 1.83

CLACK_PEAK = 0.8
BELL_PEAK = 0.35
RETURN_PEAK = 0.7


def decode(path: str) -> np.ndarray:
    """Decodes the recording to mono float32 at RATE with ffmpeg."""
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-ac", "1", "-ar", str(RATE), "-f", "f32le", "-"],
        check=True,
        capture_output=True,
    ).stdout
    return np.frombuffer(raw, dtype=np.float32)


def seconds(n: float) -> int:
    return int(n * RATE)


def rms(x: np.ndarray) -> float:
    return float(np.sqrt(np.mean(x**2)) + 1e-9)


def shaped(x: np.ndarray, fade_in: float, fade_out: float) -> np.ndarray:
    """Fades in linearly and out on a raised cosine, so no cue starts or ends in a click."""
    x = x.copy()
    n_in, n_out = seconds(fade_in), seconds(fade_out)
    x[:n_in] *= np.linspace(0.0, 1.0, n_in)
    x[-n_out:] *= 0.5 + 0.5 * np.cos(np.linspace(0.0, np.pi, n_out))
    return x


def from_attack(x: np.ndarray, fraction: float) -> np.ndarray:
    """Drops the lead-in before the first sample at `fraction` of the peak, so a cue never lags
    the key press that plays it."""
    attack = int(np.argmax(np.abs(x) >= fraction * np.abs(x).max()))
    return x[max(attack - seconds(PRE_ATTACK_SECONDS), 0) :]


def strikes(audio: np.ndarray) -> list[dict]:
    """Finds isolated, clean key strikes, aligned to their attack."""
    onsets = librosa.onset.onset_detect(
        y=audio, sr=RATE, units="samples", backtrack=True, hop_length=256
    )
    before = np.diff(onsets, prepend=-(10**12))
    after = np.diff(onsets, append=10**12)
    length, pre = seconds(CLACK_SECONDS), seconds(0.08)
    found = []
    for onset, gap_before, gap_after in zip(onsets, before, after):
        if gap_before < seconds(GAP_BEFORE_SECONDS) or gap_after < seconds(GAP_AFTER_SECONDS):
            continue
        if onset < pre or onset + seconds(0.3) > len(audio):
            continue
        window = audio[onset : onset + seconds(0.05)]
        peak = float(np.abs(window).max())
        # Backtracking can land well before the hit, and some hits have a soft precursor; the
        # attack is the first sample at half the peak.
        attack = onset + int(np.argmax(np.abs(window) >= 0.5 * peak))
        start = attack - seconds(PRE_ATTACK_SECONDS)
        snr_before = peak / rms(audio[start - pre : start])
        snr_after = peak / rms(audio[attack + seconds(0.15) : attack + seconds(0.25)])
        if snr_before < MIN_SNR_BEFORE or snr_after < MIN_SNR_AFTER:
            continue
        clip = audio[start : start + length]
        centroid = float(librosa.feature.spectral_centroid(y=clip[: seconds(0.05)], sr=RATE).mean())
        found.append({"at": start / RATE, "clip": clip, "snr": snr_before, "centroid": centroid})
    return found


def clack(clip: np.ndarray) -> np.ndarray:
    # A gentle high-pass removes the room rumble under the strike.
    high_pass = signal.butter(2, 40, "highpass", fs=RATE, output="sos")
    return shaped(signal.sosfiltfilt(high_pass, clip), 0.001, CLACK_SECONDS / 2)


def bell(audio: np.ndarray) -> np.ndarray:
    """Keeps only the bell's ringing partials, separated from the key strikes around it."""
    raw = audio[seconds(BELL_AT_SECONDS) : seconds(BELL_AT_SECONDS + BELL_SECONDS)]
    ring = librosa.effects.harmonic(np.ascontiguousarray(raw), margin=3.0)
    band = signal.butter(4, [2_500, 12_000], "bandpass", fs=RATE, output="sos")
    ring = signal.sosfiltfilt(band, ring)
    return shaped(from_attack(ring, 0.05), 0.002, BELL_SECONDS * 0.3)


def carriage_return(audio: np.ndarray) -> np.ndarray:
    raw = audio[seconds(RETURN_AT_SECONDS) : seconds(RETURN_AT_SECONDS + RETURN_SECONDS)]
    return shaped(from_attack(raw, 0.2), 0.002, 0.08)


def peaked(x: np.ndarray, peak: float) -> np.ndarray:
    return x * (peak / np.abs(x).max())


def write(name: str, x: np.ndarray) -> None:
    pcm = np.round(x * 32_767).astype("<i2")
    (OUT / f"{name}.pcm").write_bytes(pcm.tobytes())
    print(f"{name}: {len(x) / RATE:.3f} s")


def build(source: str) -> None:
    audio = decode(source)
    found = strikes(audio)
    bright = sorted(
        (s for s in found if s["centroid"] >= BRIGHT_CENTROID_HZ), key=lambda s: -s["snr"]
    )
    dull = sorted((s for s in found if s["centroid"] < BRIGHT_CENTROID_HZ), key=lambda s: -s["snr"])
    if len(bright) < CLACKS or not dull:
        sys.exit(f"too few clean strikes: {len(bright)} bright, {len(dull)} dull")

    OUT.mkdir(parents=True, exist_ok=True)
    clacks = [clack(s["clip"]) for s in bright[:CLACKS]]
    # Clacks share one loudness, so no variant stands out in a run of keys.
    clacks = [c / rms(c[: seconds(0.05)]) for c in clacks]
    gain = CLACK_PEAK / max(np.abs(c).max() for c in clacks)
    for i, (strike, c) in enumerate(zip(bright, clacks)):
        write(f"clack-{i}", c * gain)
        print(f"  from {strike['at']:.3f} s")
    write("space", peaked(clack(dull[0]["clip"]), CLACK_PEAK))
    print(f"  from {dull[0]['at']:.3f} s")
    write("bell", peaked(bell(audio), BELL_PEAK))
    write("return", peaked(carriage_return(audio), RETURN_PEAK))


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python sounds/build_sounds.py path/to/typewriter-sound.mp3")
    build(sys.argv[1])
