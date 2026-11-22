# Clairlune user guide

Clairlune puts a new landscape drawn in text on your desktop every hour, with
Clawd doing something in it. This guide covers installing it, the everyday
controls, and what to do when something isn't right.

- [Install](#install)
- [The first run](#the-first-run)
- [Everyday use](#everyday-use)
- [Settings](#settings)
- [More than one monitor](#more-than-one-monitor)
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
  then `scoop install clairlune`. With winget: until Microsoft accepts the
  package (then it's `winget install Clairlune.Clairlune`), install from the
  release's `clairlune_<version>_winget.zip`: extract it, allow local
  manifests once with `winget settings --enable LocalManifestFiles` in an
  administrator Terminal, then run `winget install --manifest <the folder>`
  ([the README](../README.md#winget) has a copy-paste version).
- **macOS 13 or later:** open the DMG (`aarch64` for Apple silicon, `x64` for
  Intel) and drag Clairlune to Applications. The first time you open it, macOS
  refuses: open System Settings > Privacy & Security, scroll down and click
  Open Anyway. Updates from inside the app don't ask again. With Homebrew:
  `brew install --cask yam333/clairlune/clairlune`.
- **Linux:** there's no desktop app (0.18.0 was the last). `cargo install
  clairlune-cli` renders the scenes as PNGs and GIFs to set yourself.
- **From source:** `cargo install clairlune` (Rust 1.90 or newer).

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
- switch the **theme**, or turn **live** on or off,
- **save** what's on screen as a PNG, a GIF, or to the library,
- open **Settings**, and see who made it (**About Clairlune**).

On Windows the menu is dark or light like the taskbar. The Archive (the last
week of wallpapers), the Library (the ones you saved for good) and the
Timeline (a plan for the day) open from Settings: the Open buttons on the
Archive page, and Edit timeline under Rotation (Mode: Timeline).

Opening Clairlune again while it runs brings up Settings.

## Settings

The sections are down the left, over a glass pane that lets the wallpaper
through. Under them is what's on screen now (its name, Live or Still, when it
changes next and its seed: click the seed to copy it) with New wallpaper,
Previous and Save to library. Esc closes a menu or the welcome, never the
window.

- **Rotation:** how often the wallpaper changes (every hour by default), or a
  Timeline that follows a plan for each day of the week; Resume when changes
  are paused; and From a seed: type a wallpaper's seed to get it back.
- **Display:** Live or Still frame; the appearance (light, dark, like the
  system, or on a schedule: set times, or sunrise and sunset for your
  location); and one scene per monitor or one across all.
- **Live:** smoothness (6 to 60 frames a second, 12 by default; higher is
  smoother and uses more power), what to do on battery, what pauses live
  when an app covers the screen, and on the Mac Lighter on Retina screens
  (half the resolution: about a quarter of the memory, with big external
  displays especially).
- **Clawd:** Clawd on or off.
- **Content:** which landscapes (each tile shows that landscape), activities
  and weather to pick from, the look
  (line art or dithered), contrast and zoom (Clawd up close, smaller, or tiny
  in a huge landscape).
- **Archive** and **Export:** how long past wallpapers are kept, and where
  saved PNGs and GIFs go (Choose… opens the system's folder picker). Saved images are signed in a corner, small, in the
  scene's own text ("clairlune · by yam333"); Sign saved images turns that
  off. The wallpaper on your desktop is never signed.
- **General:** start with the computer, the tray icon, Permissions (what to
  allow in the system's settings, each Granted or with an Open Settings
  button), updates, Restore my wallpaper, Report a problem, Remove Clairlune,
  logs and diagnostics.
- **About:** who made Clairlune (yam333, with links to GitHub), the version,
  and the fan-project notice.

Changes save as you make them; content changes show from the next wallpaper.

## The Timeline

A plan for the day, or for each day of the week (Settings > Rotation > Edit
timeline; the switch beside its title turns it on). With nothing picked, its right side
plays how it works:

- **Drag** a saved wallpaper, or the die (a random scene), onto a day.
- **Click** empty time (hatched: new scenes come on the Generate schedule
  there) to add a slot where the outline shows; double-click inside a slot
  to split it.
- **Drag a slot's edge** (the grip) to move its start; Shift for single
  minutes.
- **Click a slot** to choose what it shows, its theme and Clawd.
- Keys: arrows pick slots, Delete removes one, Ctrl+Z (Cmd+Z) undoes, Esc
  lets go. A day's marks add a slot, copy the day to others, or clear it.

## More than one monitor

Each monitor gets its own scene at its own resolution, or one wide scene spread
across all of them (Display > Monitors). With a scene each:

- **Display:** under Monitors, a monitor can keep the still frame while live
  plays on the others (for example live on the big screen, still on the
  laptop).
- **Content:** pick a monitor at the top and turn on **Its own choices** to
  give it its own landscapes, activities and the rest. Monitors without their
  own choices follow the ones for all.
- The menu's **New wallpaper on** gives one monitor a new scene.

## Updates

Once a day Clairlune asks GitHub whether a newer version is out (Settings >
General; it can be turned off, and **Check now** asks right away). When there
is one, **Update now** in Settings or **Update to ...** in the menu downloads
it, checks it against the checksum GitHub lists for it, installs it and starts
Clairlune again. Copies installed with Scoop or cargo show the command to
update them instead. After `brew upgrade`, or a new copy dragged over from the
DMG, Clairlune restarts into the new version on its own.

## Where things are kept

| | Windows | macOS |
|---|---|---|
| Settings (`settings.toml`, `timeline.toml`) | `%APPDATA%\clairlune` | `~/Library/Application Support/clairlune` |
| Archive, library, stills, logs | `%LOCALAPPDATA%\clairlune` | `~/Library/Application Support/clairlune` |
| Saved PNGs and GIFs | `Pictures\Clairlune` | `~/Pictures/Clairlune` |

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
fills the screen, maximized, zoomed or full screen (Settings > Live > Pause
behind full-screen apps), on battery if you asked for that, and while the
computer is locked.

**The wallpaper changed back to Clairlune's after I set my own.** Clairlune
changes the wallpaper every hour. Use Restore my wallpaper (Settings >
General), which also stops the changes; Resume in Settings > Rotation starts
them again.

**How do I get a wallpaper back?** Copy its seed (click it under the
sections in Settings) and type it into Settings > Rotation > From a seed. A
wallpaper still in the archive or the library comes back exactly as it was;
one that isn't is made again from the seed, the same as before as long as
the choices under Content and the theme are the same.

**It uses more power than I'd like.** Lower Settings > Live > Smoothness, turn
on Battery saver on battery, or use Still frame: a still costs nothing
between changes.

**Clairlune doesn't start with my Mac.** macOS may have it switched off in
System Settings > General > Login Items. Settings > General then says so under
Start at login, with an Open Login Items button: switch Clairlune on there.

**Something else is wrong.** Settings > General > Report a problem opens a
GitHub issue with the details filled in. Detailed logging (same page) records
more for a day if the problem is hard to catch, and Diagnostics > Export saves
a zip with the logs and settings to attach.

## Removing Clairlune

Settings > General > **Remove Clairlune** does it all after asking: your
wallpaper from before comes back, starting with the computer is turned off,
Clairlune's settings, archive, library and logs are deleted, and the app itself is removed where Clairlune can do it (the Windows
uninstaller runs, the Mac app goes to the Trash). PNGs and GIFs you saved
stay in Pictures/Clairlune.

Installed with Scoop or cargo? Remove the app afterwards with
`scoop uninstall clairlune` or `cargo uninstall clairlune`.
