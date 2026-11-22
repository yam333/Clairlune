"""The trailer's soundtrack: an original, slow lo-fi piece made from scratch
here (no samples, no one else's music), so it can be used anywhere.

    python scripts/trailer/music.py <out.wav>

72 BPM, sixteen bars (about 53 s): a warm electric piano over Fmaj9, Em7,
Dm9 and Cmaj9, a round bass, soft swung drums, a sparse music-box melody
with an echo, and vinyl crackle with rain underneath. Needs numpy.
"""

import sys
import wave

import numpy as np

SR = 44_100
BPM = 72
BEAT = 60 / BPM
BAR = 4 * BEAT
BARS = 16
LENGTH = BARS * BAR + 2.5  # the last chord rings out
N = int(LENGTH * SR)
rng = np.random.default_rng(7)


def midi(n):
    return 440.0 * 2 ** ((n - 69) / 12)


def t_of(n):
    return np.arange(n) / SR


def fft_filter(x, lo=None, hi=None, order=2):
    """Zero-phase Butterworth-shaped band limits, over the whole signal."""
    spec = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    h = np.ones_like(f)
    if hi:
        h /= np.sqrt(1 + (f / hi) ** (2 * order))
    if lo:
        h /= np.sqrt(1 + (lo / np.maximum(f, 1e-3)) ** (2 * order))
    return np.fft.irfft(spec * h, len(x))


def add(buf, at, sig, pan=0.0, gain=1.0):
    """Mixes a mono signal into the stereo buffer at `at` seconds."""
    i = int(at * SR)
    if i >= len(buf):
        return
    sig = sig[: len(buf) - i] * gain
    left = np.cos((pan + 1) * np.pi / 4)
    right = np.sin((pan + 1) * np.pi / 4)
    buf[i : i + len(sig), 0] += sig * left * np.sqrt(2)
    buf[i : i + len(sig), 1] += sig * right * np.sqrt(2)


# --- voices -----------------------------------------------------------------


def epiano(note, dur, vel):
    """A soft electric piano: a sine with fading overtones, a little bell on
    the attack, a slow tape wobble, and a gentle release."""
    n = int((dur + 1.2) * SR)
    t = t_of(n)
    f = midi(note)
    wobble = 1 + 0.0025 * np.sin(2 * np.pi * 0.45 * t + note)
    phase = 2 * np.pi * np.cumsum(f * wobble) / SR
    tone = (
        np.sin(phase)
        + 0.35 * np.sin(2 * phase) * np.exp(-t / 0.5)
        + 0.12 * np.sin(3 * phase) * np.exp(-t / 0.25)
        + 0.05 * np.sin(7.1 * phase) * np.exp(-t / 0.06)
    )
    env = np.minimum(t / 0.006, 1) * np.exp(-t / 2.4)
    release = np.clip((dur + 1.2 - t) / 1.2, 0, 1) ** 2
    return tone * env * release * vel


def bass(note, dur, vel):
    n = int((dur + 0.15) * SR)
    t = t_of(n)
    f = midi(note)
    tone = np.sin(2 * np.pi * f * t) + 0.25 * np.sin(4 * np.pi * f * t)
    env = np.minimum(t / 0.012, 1) * np.exp(-t / 1.6)
    release = np.clip((dur + 0.15 - t) / 0.15, 0, 1)
    return np.tanh(1.4 * tone) * env * release * vel


def pluck(note, vel):
    """A music box: bright, quick, round."""
    n = int(1.6 * SR)
    t = t_of(n)
    f = midi(note)
    tone = np.sin(2 * np.pi * f * t) + 0.3 * np.sin(2 * np.pi * 3 * f * t) * np.exp(-t / 0.08)
    return tone * np.minimum(t / 0.002, 1) * np.exp(-t / 0.45) * vel


def kick(vel):
    n = int(0.45 * SR)
    t = t_of(n)
    f = 45 + 75 * np.exp(-t / 0.035)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.22)
    click = rng.standard_normal(n) * np.exp(-t / 0.003) * 0.15
    return (body + click) * vel


def snare(vel):
    """A soft, dusty rim-and-brush snare."""
    n = int(0.3 * SR)
    t = t_of(n)
    noise = fft_filter(rng.standard_normal(n), lo=900, hi=3800) * np.exp(-t / 0.09)
    body = np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.05) * 0.5
    return (noise * 0.9 + body) * vel


def hat(vel):
    n = int(0.08 * SR)
    t = t_of(n)
    noise = fft_filter(rng.standard_normal(n), lo=5000, hi=8000)
    return noise * np.exp(-t / 0.022) * vel


# --- the piece --------------------------------------------------------------

# Chord voicings (MIDI) and bass roots, one chord a bar.
CHORDS = [
    (41, [57, 60, 64, 67]),  # Fmaj9
    (40, [55, 59, 62, 64]),  # Em7
    (38, [53, 57, 60, 64]),  # Dm9
    (36, [52, 55, 59, 62]),  # Cmaj9
]
# A sparse melody over two bars (beat, MIDI note), from C major pentatonic.
MOTIFS = [
    [(0.5, 76), (1.5, 79), (2.5, 81), (5.0, 79), (6.0, 76), (7.0, 74)],
    [(0.0, 81), (1.0, 79), (2.5, 76), (4.5, 74), (5.5, 76), (6.5, 72)],
]

keys = np.zeros((N, 2))
low = np.zeros((N, 2))
drums = np.zeros((N, 2))
melody = np.zeros((N, 2))
kick_env = np.zeros(N)

for bar in range(BARS):
    start = bar * BAR
    root, voicing = CHORDS[bar % 4]
    last = bar == BARS - 1
    dur = BAR + (2.2 if last else 0.05)
    # The chord, strummed a little, then pushed again on the "and" of 3.
    for k, note in enumerate(voicing):
        strum = k * 0.018 + rng.uniform(0, 0.008)
        pan = -0.35 + 0.7 * k / (len(voicing) - 1)
        add(keys, start + strum, epiano(note, dur, 0.17), pan)
        if not last and bar % 2 == 1:
            add(keys, start + 2.5 * BEAT + strum, epiano(note, 1.4 * BEAT, 0.07), pan)
    # Bass: in from bar 3, out for the last two.
    if 2 <= bar < BARS - 2:
        add(low, start, bass(root, 1.75 * BEAT, 0.42))
        add(low, start + 2 * BEAT, bass(root, BEAT, 0.3))
        nxt = CHORDS[(bar + 1) % 4][0]
        add(low, start + 3.5 * BEAT, bass(nxt + (1 if nxt < root else -1), 0.45 * BEAT, 0.22))
    # Drums: bars 3 to 14, a laid-back boom-bap with a swing.
    if 2 <= bar < BARS - 2:
        step = BEAT / 4
        swing = 0.06 * BEAT
        for s in range(16):
            at = start + s * step + (swing if s % 2 else 0) + rng.uniform(-0.004, 0.004)
            if s in (0, 6, 10) or (s == 15 and bar % 4 == 3):
                v = 0.85 if s == 0 else 0.6
                add(drums, at, kick(v))
                i = int(at * SR)
                env = np.exp(-t_of(int(0.4 * SR)) / 0.12) * v
                j = min(N, i + len(env))
                kick_env[i:j] = np.maximum(kick_env[i:j], env[: j - i])
            if s in (4, 12):
                add(drums, at, snare(0.45), 0.08)
            if s % 2 == 0 or rng.random() < 0.35:
                v = (0.11 if s % 4 == 2 else 0.07) * rng.uniform(0.7, 1.1)
                add(drums, at, hat(v), 0.3)
    # The melody, bars 7 to 14.
    if 6 <= bar < BARS - 2 and bar % 2 == 0:
        motif = MOTIFS[(bar // 2) % 2]
        for beat, note in motif:
            add(melody, start + beat * BEAT + rng.uniform(0, 0.01), pluck(note, 0.15), 0.25)

# An echo on the melody (a dotted eighth, three times, softer each time).
echo = np.zeros_like(melody)
d = int(0.75 * BEAT * SR)
for k, g in enumerate([0.42, 0.2, 0.09], start=1):
    echo[k * d :, 0] += melody[: -k * d, 1] * g
    echo[k * d :, 1] += melody[: -k * d, 0] * g
melody += echo

# Keys and bass breathe with the kick; everything a little dusty.
duck = (1 - 0.28 * kick_env)[:, None]
keys = keys * duck
low = low * duck
for ch in range(2):
    keys[:, ch] = fft_filter(keys[:, ch], hi=3800)
    drums[:, ch] = fft_filter(drums[:, ch], lo=30, hi=4800, order=4)
    melody[:, ch] = fft_filter(melody[:, ch], hi=5000)
    low[:, ch] = fft_filter(low[:, ch], hi=900)

# Vinyl crackle and rain.
t = t_of(N)
crackle = np.zeros(N)
pops = rng.random(N) < 28 / SR
crackle[pops] = rng.standard_normal(pops.sum()) * rng.uniform(0.2, 1, pops.sum())
crackle = fft_filter(crackle, lo=700, hi=6000) * 0.18
hiss = fft_filter(rng.standard_normal(N), lo=2000, hi=9000) * 0.004
rain = fft_filter(rng.standard_normal(N), lo=500, hi=3200) * 0.016
rain *= 0.8 + 0.2 * np.sin(2 * np.pi * 0.07 * t)
texture = np.stack([crackle + hiss + rain, crackle * 0.8 + hiss + rain * 0.9], axis=1)

mix = keys * 1.15 + low * 0.6 + drums * 0.6 + melody * 1.2 + texture

# The first two bars open up as if a filter lifts; in and out softly.
opening = np.clip(t / (2 * BAR), 0, 1)[:, None]
for ch in range(2):
    muffled = fft_filter(mix[:, ch], hi=900)
    mix[:, ch] = muffled * (1 - opening[:, 0]) + mix[:, ch] * opening[:, 0]
fade_in = np.clip(t / 1.2, 0, 1)
fade_out = np.clip((LENGTH - t) / 3.0, 0, 1)
mix *= (fade_in * fade_out)[:, None]
for ch in range(2):
    mix[:, ch] = fft_filter(mix[:, ch], hi=8500)
mix = np.tanh(mix * 1.3) / np.tanh(1.3)
mix *= 0.78 / np.max(np.abs(mix))

out = sys.argv[1] if len(sys.argv) > 1 else "trailer.wav"
with wave.open(out, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes((mix * 32767).astype("<i2").tobytes())
print(f"{out}: {LENGTH:.1f} s")
