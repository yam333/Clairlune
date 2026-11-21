# Clairlune user guide

Clairlune puts a new landscape drawn in text on your desktop every hour, with
Clawd doing something in it. This guide covers installing it, the everyday
controls, and what to do when something isn't right.

- [Install](#install)
- [The first run](#the-first-run)
- [Everyday use](#everyday-use)
- [Settings](#settings)
- [More than one monitor](#more-than-one-monitor)
- [The Mac screen saver](#the-mac-screen-saver)
- [Updates](#updates)
- [Where things are kept](#where-things-are-kept)
- [Privacy](#privacy)
- [Questions and problems](#questions-and-problems)
- [Removing Clairlune](#removing-clairlune)

## Install

Downloads are on the [releases page](https://github.com/yam333/Clairlune/releases/latest).

- **Windows 10 or 11:** run `clairlune_<version>_x64-setup.exe`. It installs
  for your account only and needs no admin rights. If SmartScreen says it
  protected your PC, click More info, then Run anyway. With Scoop:
  `scoop bucket add clairlune https://github.com/yam333/homebrew-clairlune`,
  then `scoop install clairlune`.
- **macOS 13 or later:** open the DMG (`aarch64` for Apple silicon, `x64` for
  Intel) and drag Clairlune to Applications. The first time you open it, macOS
  refuses: open System Settings > Privacy & Security, scroll down and click
  Open Anyway. Updates from inside the app don't ask again. With Homebrew:
  `brew install --cask yam333/clairlune/clairlune`.
- **Linux:** the `.deb` (`sudo apt install ./clairlune_<version>_amd64.deb`) or
  the AppImage (make it executable and run it).
- **From source:** `cargo install clairlune` (Rust 1.90 or newer). This build
  has no Mac screen saver.

## The first run

The first time, Clairlune shows a welcome with four choices:

- **Wallpaper:** Live plays the scene behind your desktop icons (Clawd moves,
  fires flicker, snow falls). Still frame sets one picture and does nothing
  between changes.
- **Theme:** like the system, light or dark.
- **Monitors** (with more than one): a scene on each, or one wide scene across
  all of them.
- **Start with the computer:** Clairlune starts quietly when you sign in, with
  no window, just its icon.

Everything here can be changed later in Settings.

## Everyday use

Clairlune lives in the notification area on Windows (the moon icon) and the
menu bar on the Mac. On Windows, double-click it for Settings; right-click it
for the menu. On the Mac, click it for the menu.

From the menu you can:

- get a **new wallpaper** (on every monitor, or just one),
- go back to the **previous** one,
- switch the **theme**, or **pause** the hourly changes,
- **save** what's on screen as a PNG or a GIF (into Pictures/Clairlune),
- open the **Archive** (the last week of wallpapers), the **Library** (the
  ones you saved for good) and the **Timeline** (a plan for the day).

Opening Clairlune again while it runs brings up Settings.

## Settings

- **Rotation:** how often the wallpaper changes (every hour by default), or a
  Timeline that follows a plan for each day of the week.
- **Theme:** light, dark, like the system, or on a schedule (set times, or
  sunrise and sunset for your location).
- **Display:** Live or Still frame, and one scene per monitor or one across
  all.
- **Live:** smoothness (6 to 60 frames a second, 12 by default; higher is
  smoother and uses more power), what to do on battery, and what pauses live
  when an app covers the screen.
- **Buddy:** Clawd on or off.
- **Content:** which landscapes, activities and weather to pick from, the look
  (line art or dithered), contrast and zoom (Clawd up close, smaller, or tiny
  in a huge landscape).
- **Archive** and **Export:** how long past wallpapers are kept, and where
  saved PNGs and GIFs go.
- **General:** start with the computer, the tray icon, updates, the Mac screen
  saver, Restore my wallpaper, Report a problem, Remove Clairlune, logs and
  diagnostics.

Changes save as you make them; content changes show from the next wallpaper.

## More than one monitor

Each monitor gets its own scene at its own resolution, or one wide scene spread
across all of them (Display > Monitors). With a scene each:

- **Display > Each monitor:** a monitor can keep the still frame while live
  plays on the others (for example live on the big screen, still on the
  laptop).
- **Content:** pick a monitor at the top and turn on **Its own choices** to
  give it its own landscapes, activities and the rest. Monitors without their
  own choices follow the ones for all.
- The menu's **New wallpaper on** gives one monitor a new scene.

## The Mac screen saver

Settings > General > Screen saver makes the wallpaper on screen the screen
saver too. Turning it on adds Clairlune to System Settings > Screen Saver and
opens it there: pick Clairlune. (macOS 14 and later let only you choose the
screen saver; on macOS 13 Clairlune picks it for you.) When it starts and
whether it locks are set in System Settings, like any screen saver. The
**Open Screen Saver settings** button gets you back there.

## Updates

Once a day Clairlune asks GitHub whether a newer version is out (Settings >
General; it can be turned off, and **Check now** asks right away). When there
is one, **Update now** in Settings or **Update to ...** in the menu downloads
it, checks it against the checksum GitHub lists for it, installs it and starts
Clairlune again. Copies installed with Scoop, cargo or the `.deb` show the
command to update them instead.

## Where things are kept

| | Windows | macOS | Linux |
|---|---|---|---|
| Settings (`settings.toml`, `timeline.toml`) | `%APPDATA%\clairlune` | `~/Library/Application Support/clairlune` | `~/.config/clairlune` |
| Archive, library, stills, logs | `%LOCALAPPDATA%\clairlune` | `~/Library/Application Support/clairlune` | `~/.local/share/clairlune` |
| Saved PNGs and GIFs | `Pictures\Clairlune` | `~/Pictures/Clairlune` | `~/Pictures/Clairlune` |

`settings.toml` can be edited by hand; Clairlune picks up the change. A typo
keeps the settings as they were and says so in Settings.

## Privacy

Everything is made on your computer from a seed; there's no AI. Clairlune
sends nothing about you anywhere. It goes online only to ask GitHub for the
newest version once a day (if that's on) and to download an update when you
click Update now. Report a problem opens a GitHub issue in your browser for you
to read and send yourself.

## Questions and problems

**macOS says Clairlune "can't be opened" or "is damaged."** Open System
Settings > Privacy & Security and click Open Anyway (once). If it says
damaged, run `xattr -dr com.apple.quarantine /Applications/Clairlune.app` in
Terminal and open it again.

**Windows SmartScreen stops the installer.** Click More info, then Run anyway.

**Live doesn't move.** Check Settings > Display (Live, not Still frame) and the
note under it, which says what plays or why not. Live pauses while an app
covers the screen (Settings > Live > Pause behind full-screen apps), on
battery if you asked for that, and while the computer is locked. On Linux, GNOME, KDE Plasma,
XFCE, Cinnamon, MATE and LXQt draw their own desktop, so they keep the still
frame.

**The wallpaper changed back to Clairlune's after I set my own.** Clairlune
changes the wallpaper every hour. Use Restore my wallpaper (Settings >
General), which also pauses the changes, or pause them from the menu.

**It uses more power than I'd like.** Lower Settings > Live > Smoothness, turn
on Battery saver on battery, or use Still frame: a still costs nothing
between changes.

**The Mac shows a different screen saver.** Pick Clairlune in System Settings >
Screen Saver (the Open Screen Saver settings button in Settings > General goes
there).

**Something else is wrong.** Settings > General > Report a problem opens a
GitHub issue with the details filled in. Detailed logging (same page) records
more for a day if the problem is hard to catch, and Diagnostics > Export saves
a zip with the logs and settings to attach.

## Removing Clairlune

Settings > General > **Remove Clairlune** does it all after asking: your
wallpaper from before comes back, starting with the computer is turned off,
the Mac screen saver goes, Clairlune's settings, archive, library and logs are
deleted, and the app itself is removed where Clairlune can do it (the Windows
uninstaller runs, the Mac app goes to the Trash, an AppImage is deleted). PNGs
and GIFs you saved stay in Pictures/Clairlune.

Installed with Scoop, cargo or the `.deb`? Remove the app afterwards with
`scoop uninstall clairlune`, `cargo uninstall clairlune` or
`sudo apt remove clairlune`.
