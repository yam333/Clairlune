"""CI only (a throwaway macOS runner): makes a .saver the screen saver on
macOS 14 and later by writing it into the wallpaper store, the way System
Settings records the choice, so the real screen saver host can be checked.
Never run this on your own Mac: the app leaves the choice to System Settings.

    python3 scripts/ci_pick_saver.py <path/to/Name.saver>
"""

import os
import plistlib
import sys
import urllib.parse

saver = os.path.abspath(sys.argv[1])
store = os.path.expanduser("~/Library/Application Support/com.apple.wallpaper/Store/Index.plist")

url = "file://" + urllib.parse.quote(saver.rstrip("/") + "/")
config = plistlib.dumps({"module": {"relative": url}}, fmt=plistlib.FMT_BINARY)
choice = {"Configuration": config, "Files": [], "Provider": "com.apple.wallpaper.choice.screen-saver"}

try:
    with open(store, "rb") as f:
        index = plistlib.load(f)
except FileNotFoundError:
    index = {}
print("store:", store, "keys:", sorted(index.keys()))


def show(name, entry):
    idle = entry.get("Idle") if isinstance(entry, dict) else None
    if isinstance(idle, dict):
        choices = idle.get("Content", {}).get("Choices", [])
        print(f"  {name}: idle providers {[c.get('Provider') for c in choices]}")


def pick(entry):
    """Sets the Idle (screen saver) choice of one entry of the store."""
    idle = entry.setdefault("Idle", {})
    content = idle.setdefault("Content", {})
    content["Choices"] = [choice]


entries = []
for key in ("AllSpacesAndDisplays", "SystemDefault"):
    entry = index.get(key)
    if isinstance(entry, dict):
        entries.append((key, entry))
for key in ("Displays", "Spaces"):
    group = index.get(key)
    if isinstance(group, dict):
        for name, entry in group.items():
            if isinstance(entry, dict):
                entries.append((f"{key}/{name}", entry))
if not entries:
    index["AllSpacesAndDisplays"] = {"Type": "individual"}
    entries.append(("AllSpacesAndDisplays", index["AllSpacesAndDisplays"]))

print("before:")
for name, entry in entries:
    show(name, entry)
for _, entry in entries:
    pick(entry)
print("after:")
for name, entry in entries:
    show(name, entry)

os.makedirs(os.path.dirname(store), exist_ok=True)
with open(store, "wb") as f:
    plistlib.dump(index, f, fmt=plistlib.FMT_BINARY)
print("module:", url)
