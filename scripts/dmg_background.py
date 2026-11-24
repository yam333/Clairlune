"""Draws the DMG window's background (packaging/macos/dmg-background.png and
its @2x) over a Clairlune night scene: the four steps to get Clairlune open,
pictured around the icons Finder shows. One is dragging it into Applications;
the others are macOS's check of an app it doesn't know, which people meet
once, the first time they open it.

    python scripts/dmg_background.py

Needs Pillow and a release build of clairlune-cli. Where the icons are is in
scripts/dmg_layout.py.
"""

import os
import subprocess
import tempfile

from PIL import Image, ImageDraw, ImageFont

from dmg_layout import APP, APPLICATIONS, ICON, SECURITY, H, W

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLI = os.path.join(ROOT, "target", "release", "clairlune-cli" + (".exe" if os.name == "nt" else ""))
FONTS = os.path.join(ROOT, "crates", "app", "assets", "fonts")
OUT = os.path.join(ROOT, "packaging", "macos")

# Drawn this much larger than the window, then scaled down: smooth edges.
SUPER = 4

CREAM = (243, 235, 221)
ACCENT = (240, 158, 98)
INK = (24, 22, 26)
PANEL = (15, 14, 19)
# Finder writes the names black (light mode) or white (dark mode): on this
# mid tone both read at about 4.5:1.
PILL = (125, 112, 102)
# The pictures of macOS's own windows.
WINDOW = (236, 235, 238)
CARD = (250, 250, 251)
TEXT = (38, 38, 42)
GREY = (120, 120, 126)
BUTTON = (212, 211, 215)
BLUE = (126, 168, 228)
RED = (214, 64, 56)
WHITE = (255, 255, 255)

SERIF = "InstrumentSerif-Regular"
SEMI = "IBMPlexSans-SemiBold"
MEDIUM = "IBMPlexSans-Medium"

PANEL_TOP = 276
STEP_Y = 342
COLS = (134, SECURITY[0], 586)


class Pen:
    """Draws in points on a canvas SUPER times as large."""

    def __init__(self):
        self.img = Image.new("RGBA", (W * SUPER, H * SUPER), (0, 0, 0, 0))
        self.d = ImageDraw.Draw(self.img)
        self.fonts = {}

    def font(self, name, size):
        key = (name, size)
        if key not in self.fonts:
            self.fonts[key] = ImageFont.truetype(os.path.join(FONTS, name + ".ttf"), round(size * SUPER))
        return self.fonts[key]

    def width(self, text, font, size):
        return self.d.textlength(text, font=self.font(font, size)) / SUPER

    def text(self, x, y, text, font, size, fill, anchor="mm"):
        self.d.text((x * SUPER, y * SUPER), text, font=self.font(font, size), fill=fill, anchor=anchor)

    def box(self, x0, y0, x1, y1, r, fill=None, outline=None, width=0):
        self.d.rounded_rectangle([v * SUPER for v in (x0, y0, x1, y1)], radius=r * SUPER, fill=fill,
                                 outline=outline, width=round(width * SUPER))

    def disc(self, x, y, r, fill):
        self.d.ellipse([v * SUPER for v in (x - r, y - r, x + r, y + r)], fill=fill)

    def poly(self, points, fill):
        self.d.polygon([(x * SUPER, y * SUPER) for x, y in points], fill=fill)

    def line(self, points, fill, width):
        self.d.line([(x * SUPER, y * SUPER) for x, y in points], fill=fill, width=round(width * SUPER), joint="curve")
        for x, y in (points[0], points[-1]):
            self.disc(x, y, width / 2, fill)

    def step(self, x, y, number, text, size=17):
        """A numbered title, centred on x."""
        r = size * 0.78
        gap = 9
        total = 2 * r + gap + self.width(text, SEMI, size)
        left = x - total / 2
        self.disc(left + r, y, r, ACCENT)
        self.text(left + r, y, str(number), SEMI, size * 0.95, INK)
        self.text(left + 2 * r + gap, y, text, SEMI, size, CREAM, "lm")

    def plate(self, x, y, half):
        """Under the name Finder writes below an icon."""
        top = y + ICON / 2 + 4
        self.box(x - half, top, x + half, top + 22, 11, PILL)

    def ring(self, x0, y0, x1, y1, r):
        self.box(x0 - 3.5, y0 - 3.5, x1 + 3.5, y1 + 3.5, r + 3.5, outline=ACCENT, width=3)

    def pointer(self, x, y):
        """The arrow pointer, its tip at (x, y)."""
        shape = [(0, 0), (0, 17), (3.9, 13.4), (6.6, 19.6), (9.6, 18.3), (6.9, 12.2), (12.2, 12.2)]
        pts = [(x + px, y + py) for px, py in shape]
        self.line(pts + pts[:2], WHITE, 2.6)
        self.poly(pts, (10, 10, 12))


def blocked(p, x, top):
    """macOS's message, with Done to click and Move to Trash not to."""
    w, h = 176, 156
    x0, x1 = x - w / 2, x + w / 2
    p.box(x0, top, x1, top + h, 13, WINDOW)
    # The warning sign.
    cy = top + 14
    p.poly([(x, cy), (x + 18, cy + 31), (x - 18, cy + 31)], (245, 197, 36))
    p.line([(x, cy + 11), (x, cy + 20)], WHITE, 3)
    p.disc(x, cy + 26, 1.9, WHITE)
    p.text(x, top + 61, "“Clairlune” Not Opened", SEMI, 12, TEXT)
    for i, half in enumerate((66, 52)):
        p.box(x - half, top + 73 + i * 8, x + half, top + 76.5 + i * 8, 1.7, (190, 190, 196))
    # As macOS 26 stacks them; 15 has them side by side, Done on the left.
    by = top + 95
    p.box(x0 + 12, by, x1 - 12, by + 22, 7, BLUE)
    p.text(x, by + 11, "Move to Trash", MEDIUM, 11.5, WHITE)
    # A cross, where the pointer isn't to go.
    cx, cy = x1 - 13, by + 11
    p.disc(cx, cy, 12, WHITE)
    p.disc(cx, cy, 10.5, RED)
    for k in (-1, 1):
        p.line([(cx - 4, cy - 4 * k), (cx + 4, cy + 4 * k)], WHITE, 2.6)
    dy = by + 29
    p.box(x0 + 12, dy, x1 - 12, dy + 22, 7, BUTTON)
    p.text(x, dy + 11, "Done", SEMI, 12, TEXT)
    p.ring(x0 + 12, dy, x1 - 12, dy + 22, 7)
    p.pointer(x + 34, dy + 12)


def open_anyway(p, x, top):
    """The bottom of the Privacy & Security page, with Open Anyway to click."""
    w, h = 190, 156
    x0, x1 = x - w / 2, x + w / 2
    p.box(x0, top, x1, top + h, 13, WINDOW)
    p.text(x, top + 17, "Privacy & Security", SEMI, 11.5, TEXT)
    p.box(x0, top + 31, x1, top + 32, 0, (214, 213, 217))
    # Scrolled to the end.
    p.box(x1 - 10, top + 40, x1 - 5, top + h - 12, 2.5, (218, 217, 221))
    p.box(x1 - 10, top + 100, x1 - 5, top + h - 12, 2.5, (140, 139, 144))
    right = x1 - 18
    for i in range(2):
        y = top + 39 + i * 12
        p.box(x0 + 10, y, right, y + 8, 4, (224, 223, 227))
    c = (x0 + 10 + right) / 2
    p.text(c, top + 74, "scroll down", MEDIUM, 10.5, GREY)
    for dx in (-44, 44):
        p.line([(c + dx - 6, top + 72), (c + dx, top + 77), (c + dx + 6, top + 72)], GREY, 2)
    p.box(x0 + 10, top + 86, right, top + h - 10, 9, CARD)
    p.text(x0 + 19, top + 100, "“Clairlune” was blocked", MEDIUM, 10.5, TEXT, "lm")
    bx0, by0, bx1 = right - 104, top + 112, right - 10
    p.box(bx0, by0, bx1, by0 + 24, 7, BUTTON)
    p.text((bx0 + bx1) / 2, by0 + 12, "Open Anyway", SEMI, 11.5, TEXT)
    p.ring(bx0, by0, bx1, by0 + 24, 7)
    p.pointer(bx1 - 8, by0 + 17)


def overlay():
    p = Pen()
    p.text(W / 2, 40, "Clairlune", SERIF, 46, CREAM)
    p.step(W / 2, 88, 1, "Drag into Applications", 20)

    # From the app to Applications.
    y = APP[1]
    x0, x1 = APP[0] + ICON / 2 + 24, APPLICATIONS[0] - ICON / 2 - 24
    for i in range(5):
        x = x0 + i * (x1 - 26 - x0) / 5
        p.line([(x, y), (x + 9, y)], ACCENT, 5.5)
    p.line([(x1 - 18, y - 17), (x1, y), (x1 - 18, y + 17)], ACCENT, 5.5)
    p.plate(*APP, 52)
    p.plate(*APPLICATIONS, 52)

    p.box(18, PANEL_TOP, W - 18, H - 18, 20, PANEL + (228,), ACCENT + (150,), 1.5)
    p.text(W / 2, PANEL_TOP + 30, "The first time you open it", SERIF, 30, CREAM)
    p.step(COLS[0], STEP_Y, 2, "Open it, click Done")
    p.step(COLS[1], STEP_Y, 3, "Double-click this")
    p.step(COLS[2], STEP_Y, 4, "Click Open Anyway")
    top = STEP_Y + 26
    blocked(p, COLS[0], top)
    open_anyway(p, COLS[2], top)
    p.plate(*SECURITY, 64)
    for a, b in ((COLS[0] + 88, SECURITY[0] - ICON / 2), (SECURITY[0] + ICON / 2, COLS[2] - 95)):
        x = (a + b) / 2
        p.line([(x - 5, SECURITY[1] - 11), (x + 5, SECURITY[1]), (x - 5, SECURITY[1] + 11)], ACCENT, 4)
    return p.img


def scene(s):
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "scene.png")
        subprocess.run([CLI, "generate", "--seed", "5", "--archetype", "mountains", "--theme", "dark",
                        "--look", "line", "--zoom", "vast", "--no-buddy", "--size", f"{W * s}x{H * s}",
                        "--out", path], check=True, capture_output=True)
        img = Image.open(path).convert("RGB")
    # A little brighter, so the landscape shows behind the icons.
    return Image.eval(img, lambda v: min(255, int(v * 1.35)))


def main():
    os.makedirs(OUT, exist_ok=True)
    over = overlay()
    for s, name in ((1, "dmg-background.png"), (2, "dmg-background@2x.png")):
        img = scene(s).convert("RGBA")
        img.alpha_composite(over.resize(img.size, Image.Resampling.LANCZOS))
        img.convert("RGB").save(os.path.join(OUT, name), optimize=True)
    print("wrote", OUT)


if __name__ == "__main__":
    main()
