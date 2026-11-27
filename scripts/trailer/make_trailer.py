"""Builds the trailer (docs/media/trailer.mp4) from real renders: each shot
is a scene the app makes, played by `clairlune-cli gif` at 1920x1080 and
25 fps, with captions over it, the Settings window from the app's own UI
test, and the original soundtrack from music.py.

    python scripts/trailer/make_trailer.py --ffmpeg <ffmpeg> --ui <ui-test dir> --work <dir> --out <mp4>

Needs numpy and Pillow, a release build of clairlune-cli, and the Settings
screenshots `clairlune --ui-test <dir>` writes.
"""

import argparse
import json
import os
import subprocess
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

W, H, FPS = 1920, 1080, 25
BAR = 4 * 60 / 72
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CLI = os.path.join(ROOT, "target", "release", "clairlune-cli" + (".exe" if os.name == "nt" else ""))
FONTS = os.path.join(ROOT, "crates", "app", "assets", "fonts")
TITLE = ImageFont.truetype(os.path.join(FONTS, "IBMPlexSans-SemiBold.ttf"), 64)
BIG = ImageFont.truetype(os.path.join(FONTS, "IBMPlexSans-SemiBold.ttf"), 112)
SUB = ImageFont.truetype(os.path.join(FONTS, "IBMPlexSans-Regular.ttf"), 34)
SMALL = ImageFont.truetype(os.path.join(FONTS, "IBMPlexSans-Regular.ttf"), 24)
XF = 0.8  # crossfade, seconds

# (name, theme, generate options, title, subtitle). Cuts fall on bar lines
# of the music: the drums come in with the second shot.
SHOTS = [
    ("campfire", "dark", ["--seed", "3", "--activity", "warm_hands", "--archetype", "forest", "--zoom", "close", "--look", "line"],
     "Clairlune", "Cozy landscapes drawn in text, living on your desktop"),
    ("kite", "light", ["--seed", "2", "--activity", "fly_kite", "--archetype", "meadow", "--zoom", "close", "--look", "line"],
     "A new scene every hour", "Sixteen landscapes, thirty-two things to do"),
    ("bubbles", "light", ["--seed", "1", "--activity", "bubbles", "--archetype", "islands", "--zoom", "close", "--look", "line"],
     "Made on your computer", "Every scene grows from a seed, offline"),
    ("meditate", "dark", ["--seed", "2", "--activity", "meditate", "--archetype", "bamboo", "--zoom", "close", "--look", "line"],
     "Clawd lives in each one", "Fishing, reading, stargazing, or just breathing"),
    ("snow", "light", ["--seed", "1", "--activity", "catch_snowflakes", "--archetype", "snowfield", "--zoom", "close", "--look", "line"],
     "Weather, day and night", "Snow, petals, fog, rain and starry skies"),
    ("broom", "dark", ["--seed", "1", "--activity", "broom_journey", "--look", "line"],
     "Some scenes go on a journey", "The land drifts by while Clawd travels"),
    ("dance", "light", ["--seed", "2", "--activity", "dance", "--archetype", "meadow", "--zoom", "close", "--look", "dither"],
     "Line art, or light and shadow", "Two looks, a new mood each time"),
    ("cocoa", "dark", ["--seed", "2", "--activity", "hot_cocoa", "--archetype", "interior", "--zoom", "close", "--look", "line"],
     "Live behind your icons", "Or a still frame that costs nothing at all"),
    ("settings", "dark", None,
     "Make it yours", "Landscapes, weather, theme, and each monitor"),
    ("moon", "dark", ["--seed", "3", "--activity", "stargaze", "--archetype", "lunar", "--zoom", "close", "--look", "line"],
     "Light on your computer", "Nothing about you ever leaves it"),
    ("end", "dark", None, "", ""),
]
# Where each shot starts (seconds): bar 3, then every bar and a half; the
# drums drop out at bar 15, the end card comes with bar 16.
CUTS = [0, 2 * BAR, 3.5 * BAR, 5 * BAR, 6.5 * BAR, 8 * BAR, 9.5 * BAR, 11 * BAR, 12.5 * BAR, 14 * BAR, 15 * BAR]


def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"{' '.join(cmd)}\n{r.stdout}\n{r.stderr}")
    return r.stdout


def render_gifs(work):
    """One GIF per scene shot (cached in `work`)."""
    paths = {}
    for name, theme, opts, *_ in SHOTS:
        if opts is None:
            continue
        gif = os.path.join(work, f"{name}.gif")
        paths[name] = gif
        if os.path.exists(gif):
            continue
        recipe = os.path.join(work, f"{name}.json")
        run([CLI, "generate", "--size", "640x360", "--theme", theme, "--out", os.path.join(work, f"{name}.png"),
             "--recipe-out", recipe, *opts])
        run([CLI, "gif", "--recipe", recipe, "--theme", theme, "--size", f"{W}x{H}", "--smoothness", "smooth",
             "--loop-s", "6", "--out", gif])
        print("rendered", name, json.load(open(recipe))["archetype"])
    return paths


def frames(gif):
    """The GIF as RGB arrays, one per video frame, round and round. The
    export merges frames that don't change into one longer frame, so each
    is shown for its own duration (a still moment lasts as long as it did
    in the app), not for one video frame."""
    step = 1000 / FPS
    clock = 0.0  # video time into this pass, ms
    while True:
        im = Image.open(gif)
        start = 0.0
        for i in range(getattr(im, "n_frames", 1)):
            im.seek(i)
            end = start + im.info.get("duration", step)
            rgb = None
            while clock < end:
                if rgb is None:
                    rgb = np.asarray(im.convert("RGB"), dtype=np.float32)
                yield rgb
                clock += step
            start = end
        clock -= start


def caption(title, sub, theme, big=False):
    """The caption layer (RGBA), lower left, with a soft shadow."""
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ink = (243, 235, 221) if theme == "dark" else (42, 36, 32)
    shade = (0, 0, 0, 150) if theme == "dark" else (255, 250, 240, 170)
    font = BIG if big else TITLE
    x, y = 132, H - (330 if big else 250)
    shadow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(shadow)
    d.text((x, y), title, font=font, fill=shade)
    d.text((x + 2, y + (138 if big else 86)), sub, font=SUB, fill=shade)
    layer.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(10)))
    d = ImageDraw.Draw(layer)
    d.text((x, y), title, font=font, fill=ink + (255,))
    d.text((x + 2, y + (138 if big else 86)), sub, font=SUB, fill=ink + (225,))
    return np.asarray(layer, dtype=np.float32)


def desktop_icons():
    """A few plain desktop icons, to show the scene is the wallpaper."""
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    for i, label in enumerate(["Projects", "Photos", "Notes", "Music"]):
        x, y = 48, 44 + i * 132
        d.rounded_rectangle((x + 22, y, x + 98, y + 64), radius=10, fill=(232, 196, 120, 235))
        d.rounded_rectangle((x + 22, y - 8, x + 58, y + 6), radius=5, fill=(214, 172, 96, 235))
        w = d.textlength(label, font=SMALL)
        d.text((x + 60 - w / 2, y + 74), label, font=SMALL, fill=(240, 236, 228, 255))
    return np.asarray(layer, dtype=np.float32)


def settings_layer(shot):
    """The Settings window (two screenshots, cross-fading) with a shadow."""
    out = []
    for path in shot:
        win = Image.open(path).convert("RGBA")
        win = win.resize((int(win.width * 1.22), int(win.height * 1.22)), Image.LANCZOS)
        mask = Image.new("L", win.size, 0)
        ImageDraw.Draw(mask).rounded_rectangle((0, 0, win.width - 1, win.height - 1), radius=14, fill=255)
        win.putalpha(mask)
        layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        x, y = W - win.width - 130, (H - win.height) // 2 - 20
        shadow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
        ImageDraw.Draw(shadow).rounded_rectangle((x + 6, y + 18, x + win.width + 6, y + win.height + 18), radius=16,
                                                 fill=(0, 0, 0, 170))
        layer.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(28)))
        layer.alpha_composite(win, (x, y))
        out.append(np.asarray(layer, dtype=np.float32))
    return out


def end_card():
    img = Image.new("RGB", (W, H), (21, 18, 15))
    d = ImageDraw.Draw(img)
    moon = Image.open(os.path.join(ROOT, "crates", "app", "packaging", "icons", "256.png")).convert("RGBA")
    moon = moon.resize((150, 150), Image.LANCZOS)
    img.paste(moon, ((W - 150) // 2, 250), moon)
    def centred(y, text, font, fill):
        w = d.textlength(text, font=font)
        d.text(((W - w) / 2, y), text, font=font, fill=fill)
    centred(430, "Clairlune", BIG, (243, 235, 221))
    centred(580, "Living ASCII wallpapers for Windows, macOS and Linux", SUB, (214, 204, 190))
    centred(640, "Free and open source  ·  github.com/yam333/Clairlune", SUB, (232, 150, 96))
    centred(930, "Unofficial fan project, not affiliated with Anthropic. Clawd is Anthropic's character. Music made for this trailer.",
            SMALL, (140, 132, 122))
    return np.asarray(img, dtype=np.float32)


def blend(base, layer, alpha=1.0):
    a = layer[..., 3:4] / 255 * alpha
    return base * (1 - a) + layer[..., :3] * a


def ramp(t, start, end, fade=0.6):
    """1 between start and end, easing in and out over `fade` seconds."""
    if t < start or t > end:
        return 0.0
    v = min((t - start) / fade, (end - t) / fade, 1.0)
    return v * v * (3 - 2 * v)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ffmpeg", required=True)
    ap.add_argument("--ui", required=True, help="the folder `clairlune --ui-test` wrote")
    ap.add_argument("--work", required=True)
    ap.add_argument("--music", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--crf", default="24")
    args = ap.parse_args()
    os.makedirs(args.work, exist_ok=True)
    gifs = render_gifs(args.work)

    # The music's sixteen bars and its last chord ringing out.
    total = 16 * BAR + 2.5
    n = int(total * FPS)
    starts = CUTS + [total + XF]
    captions = []
    for (name, theme, _, title, sub), s, e in zip(SHOTS, starts, starts[1:]):
        captions.append(caption(title, sub, theme, big=(name == "campfire")) if title else None)
    icons = desktop_icons()
    windows = settings_layer([os.path.join(args.ui, "settings-dark-content.png"),
                              os.path.join(args.ui, "settings-dark-display.png")])
    card = end_card()
    streams = {}

    def shot_frame(i, t):
        name = SHOTS[i][0]
        if name == "end":
            return card
        if name == "settings":
            # The bamboo scene, blurred and dimmed, under the window.
            key = "settings-bg"
            if key not in streams:
                streams[key] = frames(gifs["meditate"])
            bg = next(streams[key])
            bg = np.asarray(Image.fromarray(bg.astype(np.uint8)).filter(ImageFilter.GaussianBlur(14)),
                            dtype=np.float32) * 0.55
            local = t - starts[i]
            mix = ramp(local, 2.6, 99, fade=0.7)
            out = blend(bg, windows[0], 1 - mix)
            return blend(out, windows[1], mix)
        if name not in streams:
            streams[name] = frames(gifs[name])
        f = next(streams[name])
        if name == "cocoa":
            f = blend(f, icons)
        return f

    ff = subprocess.Popen(
        [args.ffmpeg, "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}",
         "-r", str(FPS), "-i", "-", "-i", args.music, "-c:v", "libx264", "-preset", "slow", "-crf", args.crf,
         "-tune", "animation", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-shortest",
         "-movflags", "+faststart", args.out],
        stdin=subprocess.PIPE,
    )
    poster = None
    for k in range(n):
        t = k / FPS
        frame = None
        for i, (s, e) in enumerate(zip(starts, starts[1:])):
            lo, hi = s - (XF / 2 if i else 0), e + XF / 2
            if not (lo <= t < hi):
                continue
            f = shot_frame(i, t)
            if captions[i] is not None:
                f = blend(f, captions[i], ramp(t, s + 0.5, e - 0.45))
            # Cross-fade around the cut.
            w = 1.0
            if i and t < s + XF / 2:
                w = (t - (s - XF / 2)) / XF
            frame = f if frame is None else frame * (1 - w) + f * w
        # In from black, out to black.
        frame *= min(1.0, t / 1.2, max(0.0, (total - t) / 1.8))
        out = np.clip(frame, 0, 255).astype(np.uint8)
        if k == int(3.2 * FPS):
            poster = out
        ff.stdin.write(out.tobytes())
        if k % 100 == 0:
            print(f"frame {k}/{n}", flush=True)
    ff.stdin.close()
    ff.wait()
    if poster is not None:
        Image.fromarray(poster).save(os.path.splitext(args.out)[0] + ".jpg", quality=88)
    print(args.out, os.path.getsize(args.out) // 1024, "KB")
    # A silent preview that plays on its own in the README (GitHub shows an
    # animated image there, not a video from the repository): the first five
    # shots, smaller and at 12 fps.
    preview = os.path.splitext(args.out)[0] + "-preview.webp"
    end = CUTS[5] - 0.25
    subprocess.run(
        [args.ffmpeg, "-y", "-loglevel", "error", "-t", f"{end:.2f}", "-i", args.out,
         "-vf", f"fps=12,scale=960:540:flags=lanczos,fade=out:st={end - 0.8:.2f}:d=0.8", "-an",
         "-c:v", "libwebp_anim", "-lossless", "0", "-quality", "62", "-compression_level", "6",
         "-loop", "0", preview],
        check=True,
    )
    print(preview, os.path.getsize(preview) // 1024, "KB")


if __name__ == "__main__":
    main()
