#!/usr/bin/env python3
"""The Earthquake's own sounds, synthesised (nothing recorded, nothing copied): Japan's coastal disaster-prevention
radio (bosai musen) as the mod plays it from its siren poles.

- `siren3`, `siren5`: a motor siren's blast, spinning up to its wail, held, cut and spinning down: 3 s for a Major
  Tsunami Warning's pattern (3 s on, 2 s off), 5 s for a Tsunami Warning's (5 s on, 6 s off), as the municipalities'
  disaster-prevention plans give them.
- `chime_up`, `chime_down`: the four bell notes a broadcast opens and closes with.
- `eew`: an alert tone for an earthquake early warning over the speakers (two tones, rapidly alternating: not NHK's).
- each also `_far`: heard from a pole a long way off, muffled and smeared by the distance and what it bounced off.

Writes Ogg Vorbis (mono, 48 kHz) into orbital-sounds/assets/orbital/sounds/quake/synth/ with ffmpeg.
    python3 tools/sounds_quake.py
    python3 tools/sounds_quake.py --eew-recording PATH   # also your own EEW chime, as quake/eew.ogg and eew_far.ogg

`--eew-recording` takes a recording of the Earthquake Early Warning's chime (someone else's: git-ignored, never
published) and writes it beside the others' recordings with its far version; tools/build_mod.py then plays it for
`quake.eew` in place of the synthesised tone.
"""
import argparse
import pathlib
import subprocess
import tempfile
import wave

import numpy as np

SR = 48_000
OUT = pathlib.Path(__file__).resolve().parent.parent / "orbital-sounds/assets/orbital/sounds/quake/synth"
RNG = np.random.default_rng(1923)


def t_of(seconds):
    return np.arange(int(seconds * SR)) / SR


def siren(on):
    """A motor siren: the rotor spins up (pitch and loudness rising together), holds at its wail with a slow beat,
    then is cut and spins down over a second and a half."""
    spin, down = 1.1, 1.6
    t = t_of(on + down)
    # the rotor's speed, 0 to 1
    up = np.clip(t / spin, 0, 1) ** 0.6
    fall = np.where(t > on, np.exp(-(t - on) / 0.55), 1.0)
    speed = up * fall
    f = 140 + 560 * speed + 6 * np.sin(2 * np.pi * 0.7 * t) * speed
    ph = 2 * np.pi * np.cumsum(f) / SR
    # its timbre: the ports chopping the air, a rich, slightly nasal tone; a second rotor a little off, beating
    tone = sum(a * np.sin(k * ph) for k, a in [(1, 1.0), (2, 0.55), (3, 0.38), (4, 0.2), (5, 0.12), (6, 0.06)])
    ph2 = 2 * np.pi * np.cumsum(f * 1.497) / SR
    tone += 0.35 * sum(a * np.sin(k * ph2) for k, a in [(1, 1.0), (2, 0.4), (3, 0.2)])
    air = np.convolve(RNG.standard_normal(len(t)), np.ones(24) / 24, mode="same") * 0.08
    env = np.clip(speed * 1.15, 0, 1) ** 1.4
    return (tone * 0.32 + air) * env


def bell(f, dur=1.6):
    t = t_of(dur)
    partials = [(1.0, 1.0, 1.6), (2.0, 0.42, 0.9), (2.76, 0.25, 0.6), (5.4, 0.1, 0.3)]
    s = sum(a * np.sin(2 * np.pi * f * m * t) * np.exp(-t / d) for m, a, d in partials)
    return s * np.minimum(t / 0.004, 1)


def chime(notes):
    out = np.zeros(int((0.55 * len(notes) + 1.8) * SR))
    for i, f in enumerate(notes):
        b = bell(f)
        o = int(i * 0.55 * SR)
        out[o:o + len(b)] += b * 0.32
    return out


def eew():
    t = t_of(2.6)
    sw = (np.floor(t / 0.16) % 2).astype(bool)
    f = np.where(sw, 1318.5, 987.8)
    ph = 2 * np.pi * np.cumsum(f) / SR
    s = np.sin(ph) + 0.3 * np.sin(2 * ph) + 0.12 * np.sin(3 * ph)
    gate = np.clip(np.sin(np.pi * ((t % 0.16) / 0.16)) * 3, 0, 1)
    return s * gate * 0.3 * np.minimum((2.6 - t) / 0.05, 1)


def far(x):
    """Heard from far off: the highs gone, smeared out by echoes off hills and buildings."""
    k = int(SR * 0.0012)
    y = np.convolve(x, np.ones(k) / k, mode="full")[: len(x)]
    y = np.convolve(y, np.ones(k) / k, mode="full")[: len(x)]
    tail = np.zeros(len(y) + int(1.6 * SR))
    tail[: len(y)] += y
    for d, a in [(0.11, 0.5), (0.27, 0.38), (0.46, 0.3), (0.73, 0.22), (1.05, 0.15), (1.4, 0.09)]:
        o = int(d * SR)
        tail[o:o + len(y)] += y * a
    return tail * 0.7


def write(name, x, out=OUT):
    x = x / max(1e-9, np.abs(x).max()) * 0.89
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".wav") as w:
        with wave.open(w.name, "wb") as f:
            f.setnchannels(1)
            f.setsampwidth(2)
            f.setframerate(SR)
            f.writeframes((x * 32767).astype("<i2").tobytes())
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", w.name, "-c:a", "libvorbis", "-q:a", "5", str(out / f"{name}.ogg")], check=True)
    print(f"{name}.ogg {len(x) / SR:.2f} s")


def load(path):
    """A recording as mono floats at SR."""
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"], capture_output=True, check=True).stdout
    return np.frombuffer(raw, np.float32).astype(np.float64)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eew-recording", help="your own recording of the EEW chime (kept out of the repository)")
    args = ap.parse_args()
    if args.eew_recording:
        x = load(args.eew_recording)
        # (trimmed of silence at its ends, a short fade so it never clicks; its clipped peaks brought under full scale)
        on = np.flatnonzero(np.abs(x) > 0.01)
        x = x[on[0]:on[-1] + 1] if len(on) else x
        f = np.minimum(1.0, np.arange(len(x))[::-1] / (0.03 * SR))
        x = np.tanh(x * f * 1.2)
        rec = OUT.parent
        write("eew", x, rec)
        write("eew_far", far(x), rec)
        return
    sounds = {
        "siren3": siren(3.0),
        "siren5": siren(5.0),
        "chime_up": chime([349.2, 440.0, 523.3, 698.5]),
        "chime_down": chime([698.5, 523.3, 440.0, 349.2]),
        "eew": eew(),
    }
    for name, x in sounds.items():
        write(name, x)
        write(f"{name}_far", far(x))


if __name__ == "__main__":
    main()
