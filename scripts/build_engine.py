#!/usr/bin/env python3
"""Builds the website's engine: crates/web as WebAssembly, into site/engine/.

  site/engine/clairlune.wasm  the scene and render crates, as the app runs them
  site/engine/catalog.js      what scenes can be made of (names, palette
                              colours, letter sets), for the page to use
                              before the engine has loaded

Needs the wasm32-unknown-unknown target (rustup target add
wasm32-unknown-unknown) and Node.js (to read the catalog out of the module).
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "site" / "engine"
BUILT = ROOT / "target" / "wasm32-unknown-unknown" / "web" / "clairlune_web.wasm"

CATALOG_JS = r"""
const { readFileSync, writeFileSync } = require('node:fs');
const [wasm, out] = process.argv.slice(1);
WebAssembly.instantiate(readFileSync(wasm), {}).then(({ instance }) => {
  const w = instance.exports;
  const n = w.cl_catalog();
  const text = new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.cl_output(), n));
  JSON.parse(text);
  writeFileSync(out, '// Made by scripts/build_engine.py from the engine itself.\nexport default ' + text + ';\n');
});
"""


def main() -> None:
    subprocess.run(
        [
            "cargo",
            "build",
            "--profile",
            "web",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "clairlune-web",
        ],
        cwd=ROOT,
        check=True,
    )
    OUT.mkdir(parents=True, exist_ok=True)
    wasm = OUT / "clairlune.wasm"
    shutil.copyfile(BUILT, wasm)
    node = shutil.which("node")
    if not node:
        sys.exit("node is needed to write site/engine/catalog.js")
    subprocess.run([node, "-e", CATALOG_JS, str(wasm), str(OUT / "catalog.js")], check=True)
    print(f"wrote {wasm.relative_to(ROOT)} ({wasm.stat().st_size // 1024} KB) and catalog.js")


if __name__ == "__main__":
    main()
