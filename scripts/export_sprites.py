#!/usr/bin/env python3
"""Build and export Clawd sprites with the Aseprite CLI.

1. --build: regenerate assets/clawd/src/<name>.aseprite from the design files in
   assets/clawd/design/<name>.lua (art drawn as code; clawd.lua is the shared
   drawing module, not a sheet).
2. Export every source (except _template) to crates/sprites/assets/clawd/<name>.png
   and <name>.json, with anchors read from the `anchors` marker layer and written
   as per-frame slice keys, and the source's SHA-256 recorded as meta.source_sha256
   so CI can detect stale exports without Aseprite.

Usage:
    python scripts/export_sprites.py --build          # build sources, export all
    python scripts/export_sprites.py _base warm_hands # export some

Aseprite is found via $ASEPRITE, then PATH, then ../tools/aseprite/build/bin/
next to the repository (where a from-source build lands).
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DESIGN = ROOT / "assets" / "clawd" / "design"
SRC = ROOT / "assets" / "clawd" / "src"
OUT = ROOT / "crates" / "sprites" / "assets" / "clawd"
LUA = ROOT / "scripts" / "aseprite"
HELPER_MODULES = {"clawd"}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def find_aseprite() -> str:
    candidates = [os.environ.get("ASEPRITE"), shutil.which("aseprite")]
    for name in ("aseprite.exe", "aseprite"):
        candidates.append(str(ROOT.parent / "tools" / "aseprite" / "build" / "bin" / name))
    for c in candidates:
        if c and Path(c).is_file():
            return c
    sys.exit("Aseprite not found. Set ASEPRITE to its executable path.")


def run(exe: str, *args: str) -> None:
    subprocess.run([exe, "-b", *args], check=True)


def build_sources(exe: str, wanted: set[str]) -> None:
    SRC.mkdir(parents=True, exist_ok=True)
    for design in sorted(DESIGN.glob("*.lua")):
        if design.stem in HELPER_MODULES or (wanted and design.stem not in wanted):
            continue
        out = SRC / f"{design.stem}.aseprite"
        run(
            exe,
            "--script-param", f"root={ROOT.as_posix()}",
            "--script-param", f"def={design.relative_to(ROOT).as_posix()}",
            "--script-param", f"out={out.relative_to(ROOT).as_posix()}",
            "--script", str(LUA / "build_sprite.lua"),
        )


def export(exe: str, src: Path) -> None:
    name = src.stem
    png = OUT / f"{name}.png"
    data = OUT / f"{name}.json"
    run(
        exe, str(src),
        "--ignore-layer", "guides",
        "--ignore-layer", "anchors",
        "--sheet", str(png),
        "--data", str(data),
        "--format", "json-array",
        "--sheet-pack",
        "--list-tags",
        "--filename-format", "{tag}#{tagframe}",
    )
    with tempfile.TemporaryDirectory() as tmp:
        anchors_json = Path(tmp) / "anchors.json"
        run(
            exe,
            "--script-param", f"src={src.as_posix()}",
            "--script-param", f"out={anchors_json.as_posix()}",
            "--script", str(LUA / "anchors.lua"),
        )
        slices = json.loads(anchors_json.read_text(encoding="utf-8"))["slices"]

    doc = json.loads(data.read_text(encoding="utf-8"))
    meta = doc.setdefault("meta", {})
    meta["image"] = png.name
    meta["slices"] = slices
    meta["source"] = src.relative_to(ROOT).as_posix()
    meta["source_sha256"] = sha256(src)
    # Keep the JSON byte-stable across machines: no absolute paths or app version.
    meta.pop("app", None)
    meta.pop("version", None)
    data.write_text(json.dumps(doc, indent=1) + "\n", encoding="utf-8", newline="\n")
    print(f"exported {src.name} -> {png.relative_to(ROOT).as_posix()} ({len(doc['frames'])} frames, {len(slices)} anchors)")


def main() -> None:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    wanted = set(args)
    exe = find_aseprite()
    if "--build" in sys.argv:
        build_sources(exe, wanted)
    sources = sorted(p for p in SRC.glob("*.aseprite") if not p.stem.startswith("_template"))
    if wanted:
        sources = [p for p in sources if p.stem in wanted]
    if not sources:
        print(f"no sources to export in {SRC.relative_to(ROOT)}")
        return
    OUT.mkdir(parents=True, exist_ok=True)
    for src in sources:
        export(exe, src)


if __name__ == "__main__":
    main()
