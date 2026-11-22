# Platform support

Decided 2026-11-19. Linux was
dropped on 2026-11-22 (0.19.0): its wallpaper and live code had never run on
a real Linux desktop, and there was no machine to try it on. The scene,
render and CLI crates still build and pass their tests on Linux in CI, and
`clairlune-cli` renders PNGs and GIFs there; the Linux code is in the git
history before 0.19.0 if it comes back.

| Platform | Minimum | Architectures | Live wallpaper |
|---|---|---|---|
| Windows | Windows 10 22H2 | x64 (ARM64 later) | yes (WorkerW, both the classic and the 24H2 layout) |
| macOS | macOS 13 Ventura | Apple Silicon and Intel (universal binary) | yes (desktop-level windows) |

## Status (milestone M5, static wallpapers)

| Platform | Static wallpaper | Notes |
|---|---|---|
| Windows | done (`IDesktopWallpaper`) | One image per monitor at its physical size (Per-Monitor DPI Aware v2). Monitor ids are device paths, stable across reboots. The first set saves the previous wallpaper and fit mode for `restore`. |
| macOS | written, not yet tried on a Mac | `NSWorkspace.setDesktopImageURL` per screen (no permission needed), re-applied on Space changes; ids from the display's vendor, model and serial numbers. |

## Status (milestone M7, live wallpapers)

| Platform | Live wallpaper | Notes |
|---|---|---|
| Windows | done, the default | A window per monitor in the desktop layer (both layouts), Direct3D 11 dirty-rect presents; wgpu DirectX 12, then GDI (classic desktop only, at most 12 fps) as fallbacks. Pauses behind maximized and full-screen apps, while locked, with the displays off, and optionally on battery. |
| macOS | done, tried on a Mac | A borderless window per screen at the desktop picture's level (below the icons), on every Space, ignoring the mouse. Its view shows one of two IOSurfaces: each tick writes only the changed pixels into the one off screen and swaps them, and the window server composites it as it is: no GPU device or swapchain in the app, so it stays light on memory, threads and wake-ups. A screen whose window is covered (zoomed app, lock screen, sleeping display) reports itself occluded and isn't drawn. Battery saver and pause on battery as on Windows. |

Stills live in the local data directory (`%LOCALAPPDATA%\clairlune\current` on Windows), each under a new file name because some systems don't reload a changed file at the same path. Old stills for a monitor are deleted after the new one is set.

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
  - wgpu's Metal backend (the Settings window) needs 10.13.
  - Rust's Apple Silicon target needs 11.
  - `NSWorkspace.setDesktopImageURL`, desktop-level windows and launch agents are all older than 13.
- Build with `MACOSX_DEPLOYMENT_TARGET=13.0` (milestone M8, packaging).

## What this means for the build

- CI tests on `windows-latest`, `macos-latest` (Apple Silicon) and `ubuntu-latest` (everything but the app there). The Release workflow builds:
  - the macOS app for Apple Silicon and Intel with the deployment target above,
  - a Windows x64 per-user setup.exe and a portable zip.
- The manual QA matrix adds an Intel Mac on macOS 13 or 14 and Windows 10 22H2.
