"""Builds the Mac DMG: the app, a link to Applications and a link to System
Settings > Privacy & Security (for Open Anyway) in a styled Finder window
(packaging/macos/dmg-background.png and its @2x, drawn by
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

from dmg_layout import APP, APPLICATIONS, H, ICON, SECURITY, SECURITY_NAME, SECURITY_PANE, TEXT, W

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    app, out = sys.argv[1], sys.argv[2]
    name = os.path.basename(app.rstrip("/"))
    icns = sorted(glob.glob(os.path.join(app, "Contents", "Resources", "*.icns")))
    settings = {
        "format": "UDZO",
        "files": [app],
        "symlinks": {"Applications": "/Applications", SECURITY_NAME: SECURITY_PANE},
        "background": os.path.join(ROOT, "packaging", "macos", "dmg-background.png"),
        # The background is drawn for this content area and these places;
        # the window's height counts its title bar too.
        "window_rect": ((200, 140), (W, H + 32)),
        "icon_locations": {name: APP, "Applications": APPLICATIONS, SECURITY_NAME: SECURITY},
        "icon_size": ICON,
        "text_size": TEXT,
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
