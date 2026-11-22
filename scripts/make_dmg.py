"""Builds the Mac DMG: the app and a link to Applications in a styled
Finder window (packaging/macos/dmg-background.png and its @2x, drawn by
scripts/dmg_background.py), with no toolbar or sidebar, and the app's icon
on the disk. macOS only.

    python3 scripts/make_dmg.py <path/Clairlune.app> <out.dmg>

Needs dmgbuild (pip install dmgbuild==1.6.7), which writes the window's
layout directly, without scripting Finder.
"""

import glob
import os
import sys

import dmgbuild

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    app, out = sys.argv[1], sys.argv[2]
    name = os.path.basename(app.rstrip("/"))
    icns = sorted(glob.glob(os.path.join(app, "Contents", "Resources", "*.icns")))
    settings = {
        "format": "UDZO",
        "files": [app],
        "symlinks": {"Applications": "/Applications"},
        "background": os.path.join(ROOT, "packaging", "macos", "dmg-background.png"),
        # The background is drawn for a 660 x 420 point content area and
        # these places; the window's height counts its title bar too.
        "window_rect": ((200, 140), (660, 452)),
        "icon_locations": {name: (180, 205), "Applications": (480, 205)},
        "icon_size": 104,
        "text_size": 13,
        "label_pos": "bottom",
        "default_view": "icon-view",
        "show_status_bar": False,
        "show_tab_view": False,
        "show_toolbar": False,
        "show_pathbar": False,
        "show_sidebar": False,
        "arrange_by": None,
    }
    if icns:
        settings["icon"] = icns[0]
    if os.path.exists(out):
        os.remove(out)
    dmgbuild.build_dmg(out, "Clairlune", settings=settings)
    print("built", out)


if __name__ == "__main__":
    main()
