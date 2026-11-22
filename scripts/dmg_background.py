"""Draws the DMG window's background (packaging/macos/dmg-background.png and
its @2x): a Clairlune night scene, the name, an arrow from the app to the
Applications folder, and the one-time Open Anyway note.

    python scripts/dmg_background.py

Needs Pillow and a release build of clairlune-cli. The layout matches
scripts/make_dmg.py (window 660 x 420 points, icons at y 205).
"""

import os
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = os.path.join(ROOT, "target", "release", "clairlune-cli" + (".exe" if os.name == "nt" else ""))
FONTS_APP = os.path.join(ROOT, "crates", "app", "assets", "fonts")
FONTS_MONO = os.path.join(ROOT, "crates", "render", "assets", "fonts")
OUT = os.path.join(ROOT, "packaging", "macos")

W, H = 660, 420          # points
APP_X, APPS_X, ICON_Y = 180, 480, 205
ICON = 104
CREAM = (243, 235, 221)
ACCENT = (232, 150, 96)
# Finder writes the names black (light mode) or white (dark mode): on this
# mid tone both read at about 4.5:1.
PILL = (125, 112, 102)


def draw(scale):
    s = scale
    with tempfile.TemporaryDirectory() as tmp:
        scene = os.path.join(tmp, "scene.png")
        subprocess.run([CLI, "generate", "--seed", "5", "--archetype", "mountains", "--theme", "dark",
                        "--look", "line", "--zoom", "vast", "--no-buddy", "--size", f"{W * s}x{H * s}",
                        "--out", scene], check=True, capture_output=True)
        img = Image.open(scene).convert("RGB")
    # A little brighter, so the landscape shows behind the icons, fading to
    # dark along the bottom, where the note is.
    img = Image.eval(img, lambda v: min(255, int(v * 1.35)))
    shade = Image.new("L", (1, H * s))
    for y in range(H * s):
        t = max(0.0, (y / (H * s) - 0.74) / 0.26)
        shade.putpixel((0, y), int(225 * min(1.0, t * 1.4)))
    dark = Image.new("RGB", img.size, (16, 15, 18))
    img = Image.composite(dark, img, shade.resize(img.size))
    d = ImageDraw.Draw(img, "RGBA")
    title = ImageFont.truetype(os.path.join(FONTS_APP, "IBMPlexSans-SemiBold.ttf"), 30 * s)
    sub = ImageFont.truetype(os.path.join(FONTS_APP, "IBMPlexSans-Regular.ttf"), 14 * s)
    note = ImageFont.truetype(os.path.join(FONTS_APP, "IBMPlexSans-Regular.ttf"), 11 * s)
    mono = ImageFont.truetype(os.path.join(FONTS_MONO, "JetBrainsMono-Regular.ttf"), 20 * s)

    def centred(y, text, font, fill):
        w = d.textlength(text, font=font)
        d.text(((W * s - w) / 2, y * s), text, font=font, fill=fill)

    centred(34, "Clairlune", title, CREAM)
    centred(78, "Drag Clairlune into Applications", sub, CREAM + (220,))
    # The arrow, in the wallpapers' own letters.
    arrow = "-  -  -  -  -  >"
    w = d.textlength(arrow, font=mono)
    d.text((((APP_X + APPS_X) * s - w) / 2, (ICON_Y - 14) * s), arrow, font=mono, fill=ACCENT)
    # Plates under the two names.
    for x in (APP_X, APPS_X):
        top = ICON_Y + ICON // 2 + 4
        d.rounded_rectangle(((x - 52) * s, top * s, (x + 52) * s, (top + 22) * s), radius=11 * s,
                            fill=PILL + (235,))
    centred(372, "The first time, macOS asks once: System Settings > Privacy & Security > Open Anyway.",
            note, CREAM + (170,))
    return img


def main():
    os.makedirs(OUT, exist_ok=True)
    big = draw(2)
    big.save(os.path.join(OUT, "dmg-background@2x.png"), optimize=True)
    draw(1).save(os.path.join(OUT, "dmg-background.png"), optimize=True)
    print("wrote", OUT)


if __name__ == "__main__":
    main()
