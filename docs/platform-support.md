# Platform support

Decided 2026-11-19 (resolves PLAN.md section 14, question 1).

| Platform | Minimum | Architectures | Live wallpaper |
|---|---|---|---|
| Windows | Windows 10 22H2 | x64 (ARM64 later) | yes (WorkerW, both the classic and the 24H2 layout) |
| macOS | macOS 13 Ventura | Apple Silicon and Intel (universal binary) | yes (desktop-level windows) |
| Linux | glibc 2.35: Ubuntu 22.04, Debian 12, Fedora 38 or newer | x86_64 (aarch64 later) | wlroots, KDE (after the M7 spike) and X11; GNOME is static only |

## Why these minimums

**Windows 10 22H2.**
- Microsoft extended free consumer security updates (ESU) for Windows 10 to 12 October 2027, so a large share of desktops still run it.
- Supporting it costs nothing extra:
  - `IDesktopWallpaper` per-monitor wallpapers exist since Windows 8.
  - Per-Monitor DPI Awareness v2 since Windows 10 1703.
  - The WorkerW technique works on Windows 10 and pre-24H2 Windows 11 the same way.
- Rust's MSVC targets require Windows 10 anyway, so nothing older is possible without extra work.
- Revisit after October 2027.

**macOS 13 Ventura.**
- macOS 26 Tahoe is the last release for Intel Macs, and macOS 27 runs on Apple Silicon only. Apple keeps shipping security updates for Intel Macs on Tahoe for about two more years, so Intel stays supported through a universal binary.
- 13 keeps 2017-2018 Intel Macs that can't upgrade past Ventura.
- Nothing in the stack needs newer:
  - wgpu's Metal backend needs 10.13.
  - Rust's Apple Silicon target needs 11.
  - `NSWorkspace.setDesktopImageURL`, desktop-level windows and launch agents are all older than 13.
- Build with `MACOSX_DEPLOYMENT_TARGET=13.0` (milestone M8, packaging).

**Linux, glibc 2.35.**
- AppImage and `.deb` builds link against the build machine's glibc. Building on Ubuntu 22.04 (glibc 2.35) covers every mainstream distribution released since 2022, which matches PLAN.md's "2023 onward" with one older LTS included.
- Ubuntu 22.04 is still in standard support until 2027.
- The tray needs `libayatana-appindicator` (packaging dependency), and live mode on Wayland needs `wlr-layer-shell`.

## What this means for the build

- CI already tests on `windows-latest`, `macos-latest` (Apple Silicon) and `ubuntu-latest`. Milestone M8 adds the packaging jobs:
  - a macOS universal build with the deployment target above,
  - a Linux build on `ubuntu-22.04`,
  - a Windows x64 MSI.
- The manual QA matrix in PLAN.md section 12 adds an Intel Mac on macOS 13 or 14 and Windows 10 22H2.
