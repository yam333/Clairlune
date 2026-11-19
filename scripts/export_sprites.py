#!/usr/bin/env python3
"""Export Clawd sprite sources with the Aseprite CLI.

For every assets/clawd/src/<name>.aseprite (except files starting with "_template"),
writes crates/sprites/assets/clawd/<name>.png and <name>.json, and records the
source file's SHA-256 in the JSON as meta.source_sha256 so CI can detect stale
exports without running Aseprite (which is paid software).

Usage:
    python scripts/export_sprites.py            # export everything
    python scripts/export_sprites.py ski fish   # export some

Set ASEPRITE to the Aseprite executable if it isn't on PATH.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "clawd" / "src"
OUT = ROOT / "crates" / "sprites" / "assets" / "clawd"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def find_aseprite() -> str:
    exe = os.environ.get("ASEPRITE") or shutil.which("aseprite")
    if not exe:
        sys.exit("Aseprite not found. Install it or set ASEPRITE to its executable path.")
    return exe


def export(exe: str, src: Path) -> None:
    name = src.stem
    png = OUT / f"{name}.png"
    data = OUT / f"{name}.json"
    subprocess.run(
        [
            exe, "-b", str(src),
            "--ignore-layer", "guides",
            "--ignore-layer", "anchors",
            "--sheet", str(png),
            "--data", str(data),
            "--format", "json-array",
            "--sheet-pack",
            "--list-tags",
            "--list-slices",
            "--filename-format", "{tag}#{tagframe}",
        ],
        check=True,
    )
    # TODO(Phase 20, needs Aseprite installed): run scripts/aseprite/anchors.lua to read
    # the `anchors` layer markers and add them to meta.slices as per-frame keys.
    doc = json.loads(data.read_text(encoding="utf-8"))
    meta = doc.setdefault("meta", {})
    meta["image"] = png.name
    meta["source"] = src.relative_to(ROOT).as_posix()
    meta["source_sha256"] = sha256(src)
    data.write_text(json.dumps(doc, indent=1, sort_keys=False) + "\n", encoding="utf-8")
    print(f"exported {src.name} -> {png.relative_to(ROOT)}, {data.relative_to(ROOT)}")


def main() -> None:
    wanted = set(sys.argv[1:])
    sources = sorted(p for p in SRC.glob("*.aseprite") if not p.stem.startswith("_template"))
    if wanted:
        sources = [p for p in sources if p.stem in wanted]
    if not sources:
        print(f"no sources to export in {SRC.relative_to(ROOT)}")
        return
    OUT.mkdir(parents=True, exist_ok=True)
    exe = find_aseprite()
    for src in sources:
        export(exe, src)


if __name__ == "__main__":
    main()
