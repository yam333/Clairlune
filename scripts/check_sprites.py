#!/usr/bin/env python3
"""CI check for Clawd sprite exports (no Aseprite needed).

Fails when:
- an exported sheet (crates/sprites/assets/clawd/*.json) has no matching source
  in assets/clawd/src/,
- an export's recorded source_sha256 doesn't match its source (stale export),
- an export's PNG is missing,
- a source has no export.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "clawd" / "src"
OUT = ROOT / "crates" / "sprites" / "assets" / "clawd"


def main() -> int:
    errors: list[str] = []
    exports = sorted(OUT.glob("*.json")) if OUT.exists() else []
    sources = sorted(p for p in SRC.glob("*.aseprite") if not p.stem.startswith("_template")) if SRC.exists() else []

    for data in exports:
        meta = json.loads(data.read_text(encoding="utf-8")).get("meta", {})
        src = ROOT / meta.get("source", f"assets/clawd/src/{data.stem}.aseprite")
        if not src.exists():
            errors.append(f"{data.relative_to(ROOT)}: no source {src.relative_to(ROOT)}")
            continue
        actual = hashlib.sha256(src.read_bytes()).hexdigest()
        if meta.get("source_sha256") != actual:
            errors.append(f"{data.relative_to(ROOT)}: stale export; run scripts/export_sprites.py {src.stem}")
        if not (OUT / meta.get("image", f"{data.stem}.png")).exists():
            errors.append(f"{data.relative_to(ROOT)}: missing sheet image")

    exported = {d.stem for d in exports}
    for src in sources:
        if src.stem not in exported:
            errors.append(f"{src.relative_to(ROOT)}: not exported; run scripts/export_sprites.py {src.stem}")

    for e in errors:
        print(f"error: {e}")
    print(f"checked {len(exports)} export(s), {len(sources)} source(s): {'FAILED' if errors else 'ok'}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
