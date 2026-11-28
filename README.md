# Clairlune

[![CI](https://github.com/yam333/Clairlune/actions/workflows/ci.yml/badge.svg)](https://github.com/yam333/Clairlune/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/yam333/Clairlune)](https://github.com/yam333/Clairlune/releases/latest)
[![crates.io](https://img.shields.io/crates/v/clairlune)](https://crates.io/crates/clairlune)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS-lightgrey)

Procedural, animated ASCII-art desktop wallpapers for Windows and macOS, written in Rust.

Clairlune renders scenes locally from a seed into text glyphs, live weather, and an optional pixel-art Clawd behind your desktop icons. There is no telemetry; only the optional update check (daily by default) uses the network.

**[Try it in your browser](https://yam333.github.io/Clairlune/#try)** · **[Download](https://github.com/yam333/Clairlune/releases/latest)** · [User guide](docs/guide.md) · [CLI reference](docs/cli.md) · [Changelog](CHANGELOG.md)

[![Clairlune trailer preview](docs/media/trailer-preview.webp)](https://github.com/yam333/Clairlune/releases/download/v0.28.0/clairlune-trailer.mp4)

> Unofficial fan project, not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character.

## Install

### Windows 10/11

Run `clairlune_<version>_x64-setup.exe` from the [latest release](https://github.com/yam333/Clairlune/releases/latest) (per-user install, no admin rights), or use Scoop:

```powershell
scoop bucket add clairlune https://github.com/yam333/homebrew-clairlune
scoop install clairlune
```

### macOS 13+

```sh
curl -fsSL https://raw.githubusercontent.com/yam333/Clairlune/main/install.sh | sh
```

Or install via Homebrew (`brew install --cask yam333/clairlune/clairlune`) or the release `.dmg` (`aarch64` or `x64`); see the [first-launch Gatekeeper note](docs/guide.md#opening-it-the-first-time).

### Cargo and Linux

```sh
cargo install clairlune       # desktop app (Windows and macOS, Rust 1.95+)
cargo install clairlune-cli   # headless renderer (Windows, macOS, and Linux, Rust 1.90+)
```

`winget` manifests and uninstallation are covered in the [user guide](docs/guide.md#install).

## Features

- Deterministic seeded scenes with character activities and biome-specific weather, drawn as line art or dithered shading; the same seed and display settings produce identical pixels on every platform.
- Light and dark colour palettes (classic, Nord, Gruvbox, Catppuccin, Solarized, Tokyo Night, Everforest, Kanagawa, Dracula, phosphor, and others) and swappable character sets (ASCII, braille, blocks, box-drawing, katakana, shapes, hex).
- Live desktop playback that redraws only dirty tiles and pauses behind full-screen apps, on battery, or while the screen is locked.
- Per-monitor scenes at native resolution, or a single wide scene spanned across displays.
- Image to ASCII studio for converting dropped or pasted images with custom character ramps, palette or photo colours, live weather, and `.txt` or PNG export.
- Wallpaper archive, taggable library, weekly timeline scheduler, and theme switching by OS appearance, clock hours, or local sunrise and sunset.
- In-app updates verified against release SHA-256 checksums.

![Generated ASCII wallpapers in dark and light themes](docs/img/showcase.png)

## CLI

`clairlune-cli` renders stills, looping GIFs, contact sheets, and frozen scene packages without the desktop app, and can set or play wallpapers directly on Windows:

```sh
clairlune-cli generate --seed 7 --theme dark --size 3840x2160 --out wallpaper.png
clairlune-cli gif --seed 7 --activity warm_hands --size 960x540 --out campfire.gif
clairlune-cli package --seed 7 --size 1920x1080 --out scene7
```

See [docs/cli.md](docs/cli.md) for all subcommands and flags.

## Build and architecture

Building the desktop app requires Rust 1.95+ on Windows or macOS. All library crates, `clairlune-cli`, and the test suite also build on Linux (MSRV 1.90).

```sh
cargo run --release -p clairlune
cargo run --release -p clairlune-cli -- generate --seed 7 --out wallpaper.png
cargo test --workspace
```

Developer recipes (sprite verification, golden image blessing, perf checks) are in the [justfile](justfile). See [CONTRIBUTING.md](CONTRIBUTING.md) before sending a patch, or file bugs via [GitHub Issues](https://github.com/yam333/Clairlune/issues/new/choose) or Settings > General > Report a problem.

| Crate | Role |
|---|---|
| [`clairlune-scene`](crates/scene) | Pure seed-to-recipe-to-scene-graph pipeline; no I/O or clocks. |
| [`clairlune-render`](crates/render) | Deterministic `skrifa` glyph rasterizer, palette-index compositor, PNG/GIF export, scene packages, and image-to-ASCII conversion. |
| [`clairlune-sprites`](crates/sprites) | Aseprite sheet loader, pixel-art validation, and character rig. |
| [`clairlune-live`](crates/live) | Incremental live wallpaper presenter behind desktop icons. |
| [`clairlune-platform`](crates/platform) | Native monitor geometry, wallpaper APIs, login items, and OS theme/motion hooks. |
| [`clairlune-cli`](crates/cli) | Headless CLI renderer and activity preview tool. |
| [`clairlune`](crates/app) | Tray application and `egui` + `wgpu` studio windows. |
| [`clairlune-web`](crates/web) | The scene and render crates as WebAssembly, for the website's playground (not on crates.io). |

Design notes, determinism rules, and the activity system are in [docs/dev](docs/dev).

## License

Code is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

Bundled fonts are under the SIL Open Font License 1.1, and original prop and UI assets are CC BY 4.0; see [NOTICE](NOTICE) for full attribution. When Clawd is enabled, rendered wallpapers include fan art of Anthropic's character (not covered by the code license); turn off `Show Clawd` in Settings > Content or pass `--no-buddy` to render scenes containing only this project's work.
