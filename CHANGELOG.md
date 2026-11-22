# Changelog

What changed in each release of Clairlune, newest first. Saved scenes and
recipes from earlier versions always look the same as before. The downloads
are on the [releases page](https://github.com/yam333/Clairlune/releases).

## 0.17.3 (2026-11-22)

- **Mac: Login Items.** macOS lets you switch Clairlune off in System Settings >
  General > Login Items, and only you can switch it back on. When start at
  login is on but switched off there, Clairlune opens that pane for you, and
  Settings > General says so, with an Open Login Items button.
- **A proper DMG window:** opening the DMG shows a Clairlune night scene with
  the app and an arrow to Applications to drag it onto, and the one-time Open
  Anyway step written underneath.

## 0.17.2 (2026-11-22)

- **Mac: pause behind a zoomed window.** Pause behind full-screen apps now
  also pauses a screen when a window fills it but for the menu bar and the
  Dock (a double click on the title bar), or when windows tiled side by side
  fill it.
- Settings > General is quieter: Check for updates just says once a day, and
  Quit, Restore my wallpaper and Remove Clairlune need no explanation.

## 0.17.1 (2026-11-22)

- **Live on every Mac:** a Mac still on the old default (the still frame)
  switches to Live once, with a note saying how to go back (Settings >
  Display). A still frame chosen after that stays.

## 0.17.0 (2026-11-22)

- **A welcome on the first run:** one screen with what Clairlune does and the
  few choices that matter most: live or still, the theme, one scene per
  monitor or one across all, and whether it starts with the computer.
- **Remove Clairlune** (Settings > General): puts your wallpaper from before
  back, turns off starting with the computer, removes the Mac screen saver,
  deletes Clairlune's settings, archive, library and logs, and removes the app
  itself where it can (the Windows uninstaller runs, the Mac app goes to the
  Trash, an AppImage is deleted). It asks first, and PNGs and GIFs you
  exported stay.
- **Live by default on the Mac** for new installs, as on Windows (Linux keeps
  the still frame by default until Live is tried on real desktops there).
- A [user guide](docs/guide.md) with answers to common questions, this
  changelog, and a trailer on the project page.

## 0.16.0 (2026-11-21)

- **One-click updates:** when a newer version is out, Update now (Settings >
  General, or the menu) downloads it from GitHub, checks it against the
  checksum GitHub lists for it, installs it and starts Clairlune again. On the
  Mac this also skips the Open Anyway step. It works for copies installed with
  the Windows setup.exe, the Mac DMG or Homebrew, and the AppImage; Scoop,
  cargo and .deb installs show how to update them instead.
- **Report a problem:** Settings > General opens a new GitHub issue with the
  version, system, monitors, wallpaper mode and the last log lines filled in
  (your home folder written as ~). You can read them in Settings first, or copy
  them; nothing is sent until you send the issue yourself.
- **Settings per monitor:** each monitor can keep the still frame while the
  others play live (Display), and pick from its own landscapes, activities,
  weather, looks, contrast and zoom (Content).
- Links open through Windows itself, so long ones arrive whole.

## 0.15.0 (2026-11-21)

- **More to see:** six new landscapes (meadow, lake, volcano, islands, the Moon
  and a bamboo grove), five new kinds of weather (petals, fog, blizzard, ash
  and sparkles) and six new things for Clawd to do (blowing bubbles,
  meditating, dancing, catching snowflakes, sweeping, hot cocoa): sixteen
  landscapes and thirty-two activities in all.
- **Check for updates** once a day (Settings > General; it can be turned off),
  or by hand with Check now.
- Look, Contrast and Zoom moved to the Content page.
- **Mac screen saver** plays and animates in macOS's own screen saver host and
  ends cleanly on macOS 14 and later. Turning it on picks it on macOS 13 and
  opens System Settings > Screen Saver on 14 and later.
- **Lighter on the Mac:** no Metal device any more; only the changed pixels are
  written into two shared surfaces that macOS shows as they are. About 30%
  less memory, 6 threads instead of 10, about a quarter of the CPU.
- Lighter everywhere: no logging thread, only the theme on screen kept in
  memory, and the Windows frame timer sleeps when nothing plays.

## 0.14.1 (2026-11-21)

- The tray and menu bar icon is a single-colour crescent: white on Windows
  (black on a light taskbar), a template the Mac menu bar tints itself.

## 0.14.0 (2026-11-21)

- **A screen saver on the Mac** (Settings > General > Screen saver): the
  wallpaper keeps playing full screen when the Mac idles. Each new wallpaper
  becomes the screen saver too.

## 0.13.1 (2026-11-21)

- **Zoom:** Close shows Clawd as he has always been, Wide about half the size
  in a bigger landscape, Vast about a quarter. Each wallpaper picks one of
  those that are on.
- Mac: the download no longer reads as damaged (signed ad hoc; Open Anyway is
  still needed once). Battery saver and Pause on battery work on macOS and
  Linux. Scrolling scenes redraw about half as much.
- New installs get a different contrast for each wallpaper. While live plays,
  the log notes CPU, memory and frame times once a minute.

## 0.13.0 (2026-11-21)

- **Depth:** scrolling scenes usually have a big landmark far behind (a granite
  dome, a lone peak, twin peaks, a volcano, a mesa, a huge rising moon). Two
  flights (the broom at dusk, the balloon by day) and a desert trek.
- **Starting with your computer:** the first run asks whether to start quietly
  at sign-in, now on macOS and Linux too. The tray icon can be hidden, with a
  Quit button beside it.
- Fixes: Restore my wallpaper stays restored; Previous walks back through your
  wallpapers; picked wallpapers keep their theme; a typo in settings.toml no
  longer resets every setting; a damaged archive shows an error instead of
  crashing; Linux and macOS monitor fixes.

## 0.12.0 (2026-11-21)

- **A dithered look:** scenes drawn as light and dark masses of dots and bars,
  and a contrast setting.
- **Journeys:** ski touring, sledding and a forest walk; all journeys move at a
  walking pace, and are skipped at low frame rates.
- Daytime-only and night-only scenes no longer land in the wrong theme. "New
  wallpaper on" gives one monitor a new scene.
- Live keeps playing while you drag Clairlune's windows; double-click the tray
  icon for Settings; chips for picking content.

## 0.11.0 (2026-11-20)

- **The first journey:** Clawd hikes a mountain ridge while the land goes by in
  layers.
- **Indoors:** a room with a window onto another landscape.
- **Eleven new activities**, from skipping stones to a hot-air balloon ride.
- One scene across all monitors, and a theme schedule (set times, or sunrise
  and sunset).
- Linux live wallpapers on wlr layer-shell compositors and plain X11 window
  managers.

## 0.10.0 (2026-11-20)

- The same calm pace at every smoothness; Direct3D 11 live wallpapers on
  Windows that copy only what changed; windows give their memory back.
- Richer desert, shore and night sky; fishing and kite flying.
- Pause when an app covers the screen; the Archive window; crash reports and a
  diagnostic bundle.
- Wallpapers on each Mac screen, kept across Spaces.

Unofficial fan project. Not affiliated with or endorsed by Anthropic. Clawd is
Anthropic's character; see [NOTICE](NOTICE).
