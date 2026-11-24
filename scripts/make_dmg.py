"""Builds the Mac DMG: the app, a link to Applications and an alias to System
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
import subprocess
import sys
import tempfile

import dmgbuild

from dmg_layout import APP, APPLICATIONS, H, ICON, SECURITY, SECURITY_NAME, SECURITY_PANE, TEXT, W

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# A Finder alias rather than a symlink, which can't have an icon of its own
# (the pane's is a blank document).
ALIAS = """
ObjC.import("AppKit");
function run([target, path, icon]) {
  const data = $.NSURL.fileURLWithPath(target)
    .bookmarkDataWithOptionsIncludingResourceValuesForKeysRelativeToURLError(1 << 10, $(), $(), null);
  if (data.isNil()) throw new Error("no bookmark for " + target);
  if (!$.NSURL.writeBookmarkDataToURLOptionsError(data, $.NSURL.fileURLWithPath(path), 1 << 10, null))
    throw new Error("couldn't write " + path);
  const image = $.NSImage.alloc.initWithContentsOfFile(icon);
  if (!$.NSWorkspace.sharedWorkspace.setIconForFileOptions(image, path, 0))
    throw new Error("couldn't set the icon of " + path);
}
"""


def alias(target, path, icon):
    subprocess.run(["osascript", "-l", "JavaScript", "-e", ALIAS, target, path, icon], check=True)


def main():
    app, out = sys.argv[1], sys.argv[2]
    name = os.path.basename(app.rstrip("/"))
    icns = sorted(glob.glob(os.path.join(app, "Contents", "Resources", "*.icns")))
    pack = os.path.join(ROOT, "packaging", "macos")
    tmp = tempfile.TemporaryDirectory()
    security = os.path.join(tmp.name, SECURITY_NAME)
    alias(SECURITY_PANE, security, os.path.join(pack, "privacy-security.png"))
    settings = {
        "format": "UDZO",
        "files": [app, security],
        "symlinks": {"Applications": "/Applications"},
        "background": os.path.join(pack, "dmg-background.png"),
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
    tmp.cleanup()
    print("built", out)


if __name__ == "__main__":
    main()
