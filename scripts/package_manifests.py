#!/usr/bin/env python3
"""Write the package-manager manifests for one release (docs/distribution.md).

Reads the installers a release built (the Release workflow's artifacts),
hashes them, and writes ready-to-publish manifests:

    <out>/homebrew/Casks/clairlune.rb          Homebrew cask (own tap)
    <out>/scoop/clairlune.json                 Scoop bucket manifest
    <out>/winget/manifests/c/Clairlune/Clairlune/<version>/*.yaml   winget
    <out>/aur/clairlune-bin/PKGBUILD, .SRCINFO  AUR binary package

Nothing is published: these files are what a tap, a bucket, a winget-pkgs
pull request or the AUR receive once the repository is public.

Usage:
    python scripts/package_manifests.py --artifacts <dir> [--version 1.2.3]
        [--base-url URL] [--out target/manifests] [--strict]

The version defaults to the workspace version in Cargo.toml; the base URL to
the GitHub release of that version. Missing installers skip their manifests
(with a note) unless --strict.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = "https://github.com/yam333/Clairlune"
DESC = "Procedural, animated ASCII-art desktop wallpapers"
LICENSE = "MIT OR Apache-2.0"


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    section = text.split("[workspace.package]", 1)[1]
    m = re.search(r'^version\s*=\s*"([^"]+)"', section, re.M)
    if not m:
        sys.exit("no [workspace.package] version in Cargo.toml")
    return m.group(1)


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# The installers cargo-packager and the Release workflow make, by key
# (matched ignoring case: the DMGs carry the product name, "Clairlune").
ARTIFACTS = {
    "setup": "clairlune_{v}_x64-setup.exe",
    "portable": "clairlune_{v}_x64-portable.zip",
    "dmg_arm": "clairlune_{v}_aarch64.dmg",
    "dmg_intel": "clairlune_{v}_x64.dmg",
    "deb": "clairlune_{v}_amd64.deb",
    "appimage": "clairlune_{v}_x86_64.AppImage",
}


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")
    print(f"  wrote {path}")


def homebrew(v: str, base: str, h: dict[str, str], n: dict[str, str]) -> str:
    # The DMG names differ only in the architecture.
    dmg = n["dmg_arm"].replace(v, "#{version}").replace("aarch64", "#{arch}")
    return f'''cask "clairlune" do
  arch arm: "aarch64", intel: "x64"

  version "{v}"
  sha256 arm:   "{h["dmg_arm"]}",
         intel: "{h["dmg_intel"]}"

  url "{base.replace(v, "#{version}")}/{dmg}"
  name "Clairlune"
  desc "{DESC}"
  homepage "{REPO}"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :ventura"

  app "Clairlune.app"

  zap trash: "~/Library/Application Support/clairlune"

  caveats <<~EOS
    Clairlune runs in the menu bar (the moon icon).
  EOS
end
'''


def scoop(v: str, base: str, h: dict[str, str], n: dict[str, str]) -> str:
    manifest = {
        "version": v,
        "description": DESC,
        "homepage": REPO,
        "license": LICENSE,
        "notes": "Clairlune runs in the notification area (the moon icon).",
        "architecture": {
            "64bit": {
                "url": f"{base}/{n['portable']}",
                "hash": h["portable"],
            }
        },
        "bin": "clairlune.exe",
        "shortcuts": [["clairlune.exe", "Clairlune"]],
        "checkver": "github",
        "autoupdate": {
            "architecture": {
                "64bit": {
                    "url": f"{base}/{n['portable']}".replace(v, "$version")
                }
            }
        },
    }
    return json.dumps(manifest, indent=4) + "\n"


WINGET_SCHEMA = "1.28.0"
WINGET_ID = "Clairlune.Clairlune"


def winget(v: str, base: str, h: dict[str, str], n: dict[str, str]) -> dict[str, str]:
    header = f"# yaml-language-server: $schema=https://aka.ms/winget-manifest.{{kind}}.{WINGET_SCHEMA}.schema.json\n\n"
    version = header.format(kind="version") + f"""PackageIdentifier: {WINGET_ID}
PackageVersion: {v}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: {WINGET_SCHEMA}
"""
    installer = header.format(kind="installer") + f"""PackageIdentifier: {WINGET_ID}
PackageVersion: {v}
InstallerType: nullsoft
Scope: user
InstallModes:
  - silent
  - silentWithProgress
UpgradeBehavior: install
Installers:
  - Architecture: x64
    InstallerUrl: {base}/{n["setup"]}
    InstallerSha256: {h["setup"].upper()}
ManifestType: installer
ManifestVersion: {WINGET_SCHEMA}
"""
    locale = header.format(kind="defaultLocale") + f"""PackageIdentifier: {WINGET_ID}
PackageVersion: {v}
PackageLocale: en-US
Publisher: Clairlune contributors
PublisherUrl: {REPO}
PackageName: Clairlune
PackageUrl: {REPO}
License: {LICENSE}
LicenseUrl: {REPO}/blob/main/LICENSE-MIT
ShortDescription: {DESC}
Description: Clairlune makes a new procedural ASCII-art landscape for each monitor, with a small pixel-art character doing something in it, and can play it live behind the desktop icons.
Moniker: clairlune
Tags:
  - wallpaper
  - ascii-art
  - desktop
  - live-wallpaper
ManifestType: defaultLocale
ManifestVersion: {WINGET_SCHEMA}
"""
    return {
        f"{WINGET_ID}.yaml": version,
        f"{WINGET_ID}.installer.yaml": installer,
        f"{WINGET_ID}.locale.en-US.yaml": locale,
    }


AUR_DEPENDS = ["glibc", "gcc-libs", "libxkbcommon"]
AUR_OPTDEPENDS = ["vulkan-icd-loader: GPU drawing for the windows"]


def aur(v: str, base: str, h: dict[str, str], n: dict[str, str]) -> dict[str, str]:
    depends = " ".join(f"'{d}'" for d in AUR_DEPENDS)
    optdepends = "\n".join(f"  '{d}'" for d in AUR_OPTDEPENDS)
    pkgbuild = f"""# Maintainer: Clairlune contributors
pkgname=clairlune-bin
pkgver={v}
pkgrel=1
pkgdesc="{DESC}"
arch=('x86_64')
url="{REPO}"
license=('MIT' 'Apache-2.0')
provides=('clairlune')
conflicts=('clairlune')
depends=({depends})
optdepends=(
{optdepends}
)
source_x86_64=("{f"{base}/{n['deb']}".replace(v, "${pkgver}")}")
sha256sums_x86_64=('{h["deb"]}')

package() {{
  bsdtar -xf data.tar.* -C "$pkgdir"
}}
"""
    srcinfo = f"""pkgbase = clairlune-bin
\tpkgdesc = {DESC}
\tpkgver = {v}
\tpkgrel = 1
\turl = {REPO}
\tarch = x86_64
\tlicense = MIT
\tlicense = Apache-2.0
""" + "".join(f"\tdepends = {d}\n" for d in AUR_DEPENDS) + "".join(
        f"\toptdepends = {d}\n" for d in AUR_OPTDEPENDS
    ) + f"""\tprovides = clairlune
\tconflicts = clairlune
\tsource_x86_64 = {base}/{n["deb"]}
\tsha256sums_x86_64 = {h["deb"]}

pkgname = clairlune-bin
"""
    return {"PKGBUILD": pkgbuild, ".SRCINFO": srcinfo}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--artifacts", required=True, type=Path)
    ap.add_argument("--version")
    ap.add_argument("--base-url")
    ap.add_argument("--out", type=Path, default=ROOT / "target" / "manifests")
    ap.add_argument("--strict", action="store_true", help="fail when an installer is missing")
    args = ap.parse_args()
    v = (args.version or workspace_version()).lstrip("v")
    base = (args.base_url or f"{REPO}/releases/download/v{v}").rstrip("/")

    files = {p.name.lower(): p for p in args.artifacts.rglob("*") if p.is_file()}
    hashes: dict[str, str] = {}
    names: dict[str, str] = {}
    missing = []
    for key, pattern in ARTIFACTS.items():
        name = pattern.format(v=v).lower()
        if name in files:
            hashes[key] = sha256(files[name])
            names[key] = files[name].name
        else:
            missing.append(pattern.format(v=v))
    if missing:
        print("missing installers: " + ", ".join(missing))
        if args.strict:
            return 1

    out = args.out
    print(f"Clairlune {v}, downloads from {base}")
    if {"dmg_arm", "dmg_intel"} <= hashes.keys():
        write(out / "homebrew" / "Casks" / "clairlune.rb", homebrew(v, base, hashes, names))
    if "portable" in hashes:
        write(out / "scoop" / "clairlune.json", scoop(v, base, hashes, names))
    if "setup" in hashes:
        d = out / "winget" / "manifests" / "c" / "Clairlune" / "Clairlune" / v
        for name, text in winget(v, base, hashes, names).items():
            write(d / name, text)
    if "deb" in hashes:
        for name, text in aur(v, base, hashes, names).items():
            write(out / "aur" / "clairlune-bin" / name, text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
