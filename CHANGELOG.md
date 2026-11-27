# Changelog

What changed in each release of Clairlune, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). Saved scenes and recipes from
earlier versions always look the same as before. The downloads are on the
[releases page](https://github.com/yam333/Clairlune/releases).

## [Unreleased]

## [0.33.2] - 2026-11-27

### Added

- Saved wallpapers in the Library now have a **Display** choice (Follow, Still,
  Live) and a **Tags** field, so tag pools in the Timeline can be filled without
  editing files by hand.

### Fixed

- **Restore my wallpaper** followed by resuming Clairlune in the same session no
  longer forgets your original wallpaper: a second Restore puts it back.
- Clicking **Resume** after Restore now clears the restored state across
  restarts and immediately puts a Clairlune wallpaper (or the active Timeline
  slot) back on screen instead of waiting for the next timer or slot boundary.
- Timeline slots and Library items now honour their **Display** choice (`Live`
  is enabled in the Timeline inspector, and both `Live` and `Still` take effect
  while that slot or wallpaper is on screen).
- Timeline random slots now apply their landscape, activity and weather choices
  to monitors that have their own Content choices too; missing items and empty
  pools keep the slot's theme, Clawd and display choices when generating a
  replacement; and unknown weather names in `timeline.toml` are reported.
- Dragging a monitor onto the seam between two touching monitors in **Spread
  across** no longer ping-pongs into an overlapping placement.
- Open windows now follow scheduled or system theme changes automatically, show
  finished exports and job errors without waiting for pointer movement, and
  finish drawing Content's preview tiles when the theme switches mid-render.
- On an Apple Silicon Mac running an Intel copy under Rosetta, in-app updates
  now fetch the native Apple Silicon DMG.

## [0.33.1] - 2026-11-27

### Changed

- **Much lighter on the Mac, in Live and Still frame:**
  - Scenes with snow, sandstorms, sea spray, spindrift or campfire embers now
    copy only the small tiles that changed each frame instead of the whole
    Retina screen, and IOSurfaces tag sRGB by name instead of re-parsing an
    ICC profile on every frame.
  - Watching for window changes behind a clear menu bar (macOS 26) ignores
    trackpad scrolling, backs off while the Mac is untouched, stops while the
    displays sleep, and makes no temporary allocations per window.
  - A scene spread across several monitors is composited once per frame instead
    of once per monitor, and unused memory is given back to the system after
    closing a window or finishing a render.
  - Settings and Timeline pause their animations while unfocused or covered,
    and app updates keep Content's preview tiles on disk instead of drawing
    them again.

### Fixed

- On the Mac, the event loop could spin at high CPU when a timer woke early
  (such as after sleep) or while a new wallpaper was being made.

## [0.33.0] - 2026-11-26

### Added

- **Weather that belongs to its landscape**, eleven kinds: sea spray at the
  shore, the lighthouse, the islands and the waterfall; sandstorms in the
  desert, the oasis and the savanna; tumbleweed through the canyon, desert and
  savanna; cinders rising over the volcano; sky lanterns over the lantern
  lake, the harbour and the great tree; fireworks over the harbour and the
  rooftops; glowing spores among the mushrooms, in the swamp and the crystal
  cave; wisps in the swamp and round the standing stones; bats about the cave,
  the ruins and the wizard's tower; spindrift off the mountains, the
  snowfield and the observatory's peak; dragonflies over the lake, meadow,
  swamp, waterfall, zen garden, windmills and countryside. They come with any
  activity that's fine with no weather, and each can be turned off under
  Content > Weather.

### Changed

- With the new weather in the mix, some seeds now come with other weather
  than before. Saved wallpapers look as they did.

## [0.32.0] - 2026-11-26

### Added

- **Six landscapes:** a lighthouse on a headland, its beam out over the bay
  after dark; a harbour with boats at the quay and lit windows in the water; a
  zen garden of raked gravel and rocks with a stone lantern; an observatory on
  a peak under the Milky Way; an oasis of palms round a pool in the dunes; and
  windmills over fields of flowers. 33 in all.
- **Palettes** (Content > Colours): Nord, Gruvbox, Catppuccin, Solarized, Rosé
  Pine, Tokyo Night, green phosphor and amber, each with a light and a dark
  version, beside Clairlune's own. Turn on several for a different one each
  wallpaper.
- **Letter sets** (Content > Letters): the scenes drawn in braille dots,
  blocks, box-drawing lines or katakana instead of text. Everything drawn in
  letters takes the set: the land, the sky, the weather, the embers.
- The command-line tool's `--palette` and `--letters`.

### Changed

- With the new landscapes in the mix, a seed may now make a different scene
  than before. Saved wallpapers (the archive, the library) look as they did.

## [0.31.1] - 2026-11-25

### Changed

- On macOS 26 the menu bar turns solid and clear again at once, as a window
  reaches it or starts to leave it, instead of up to a second later.

## [0.31.0] - 2026-11-25

### Changed

- On macOS 26, where the wallpaper shows through the menu bar, the bar turns
  one solid colour while windows almost fill the screen: dark grey over a dark
  wallpaper, white over a light one.

### Fixed

- On the Mac, with Live, the menu bar's text kept the colour for the other
  theme after the theme changed (white on a light wallpaper).

## [0.30.2] - 2026-11-24

### Fixed

- Windows: after an update the taskbar kept the older version's icon.

## [0.30.1] - 2026-11-24

### Fixed

- The Mac DMG's window drew an older version's picture behind its icons while
  that version's DMG was still open. Each version's disk now has its own name.

## [0.30.0] - 2026-11-24

### Changed

- **A new mark:** a square over a circle, in place of the crescent moon. One
  colour in the tray and menu bar, with a clear line around the square; an
  orange square over a pale circle in Settings; and on a dark tile as the app
  icon.

### Fixed

- The mark beside the name in Settings was a misdrawn crescent.

## [0.29.1] - 2026-11-24

### Added

- **A Terminal line that installs Clairlune on a Mac**, with nothing to allow
  afterwards: `curl -fsSL https://raw.githubusercontent.com/yam333/Clairlune/main/install.sh | sh`.
  It checks the download against the release's checksums, which releases now
  list in `SHA256SUMS`.

### Changed

- The Mac DMG shows how to open Clairlune the first time, step by step, with a
  shortcut to System Settings > Privacy & Security, where Open Anyway is. The
  Homebrew cask says how, too.

## [0.29.0] - 2026-11-23

### Added

- **The monitors' arrangement** (Display, with Spread across): each monitor
  drawn where it sits, showing its part of the wallpaper. Drag one to line it
  up with the others (it snaps to their edges; a gap stays in the scene, as
  the frames between screens do). The bar on the main monitor, where the scene
  centres, drags to another one, or double-click a monitor; the back arrow
  returns to the system's arrangement.

### Changed

- Switching between One each and Spread across, or moving a monitor, draws
  the wallpaper on screen again right away instead of at the next change.
- With Spread across, monitors no longer have their own Live or Still frame:
  the scene is one picture.

### Fixed

- **Spread across on a Mac with a Retina screen and ordinary monitors** left
  the picture broken up, with parts missing between monitors: positions were
  measured at the Retina density and sizes at each screen's own. Monitors are
  now laid out in points, as macOS arranges them, and a Retina screen shows
  its part pixel-doubled.

## [0.28.0] - 2026-11-23

### Changed

- **A shorter menu** (tray and menu bar): New wallpaper, Previous wallpaper,
  Theme, Live wallpaper, **Save wallpaper** (as PNG, as GIF, to library),
  Settings, About and Quit.
- **The menu is dark on a dark taskbar** (Windows), light on a light one, and
  follows it when it changes.
- The theme schedule's times have a dash between them.

### Removed

- Pause changes, Library, Timeline and Archive from the menu; they open from
  Settings.

### Fixed

- **Monitors keep their own mode.** Picking Live or Still frame for one
  monitor (Display) changed the other too: rows of buttons with the same words
  answered each other's clicks. Fixed for every such row.
- The theme schedule's times show in full ("19:00", not "9:00").

## [0.27.0] - 2026-11-22

### Changed

- **The Timeline, shown rather than told.** With nothing picked, its right
  side plays how it works, the way macOS's trackpad settings do: a pointer
  drops a wallpaper on a day, drags a slot's start, and opens a slot. The keys
  are drawn as keys. Empty time is hatched; under the pointer it shows the slot
  a click adds there. Slot edges show grips. Saved wallpapers and the random
  scene (a die) lift under the pointer to be dragged. A day's Add, Copy to… and
  Clear are marks; Copy to… picks days as chips; a switch beside the title
  turns the timeline on; Undo is a mark; a missing wallpaper is marked on its
  slot.
- Text in Clairlune's windows can't be selected by dragging over it (the
  problem report's details still can).
- "Show details" / "Hide details".

### Removed

- The Timeline's paragraphs of help and status; Display's Each monitor heading
  and note.

### Fixed

- **Animations keep moving** without the pointer: the embers on the signature
  and a long wallpaper name no longer wait for the mouse.

## [0.26.0] - 2026-11-22

### Fixed

- **Nothing cut off or under a scroll bar.** Settings can't be made narrower
  than its page needs (972 points: the sidebar, the page at full width and room
  for its scroll bar), the whole pane scrolls with its bar at the pane's edge,
  and every scroll bar in Clairlune keeps room of its own instead of covering
  the content. The Timeline's start time shows all of "00:00".

### Removed

- The lines of explanation under Timeline, Days without slots change every,
  Follow the sun and Dark between.

## [0.25.0] - 2026-11-22

### Added

- **About:** embers drift up off the signature, more and quicker under the
  pointer, to say it's a link.
- **Export folder** is picked in the system's folder picker (Choose…) and shown
  under it; Pictures goes back to the default.

### Changed

- **Theme moved into Display**, as Appearance, with the theme schedule.
- **What's on now:** a name too long for the sidebar drifts slowly by instead
  of being cut short; "next one in 55 min" reads "next: 55 min".
- **From a seed** empties its box once the wallpaper is on its way.

### Removed

- About's two GitHub buttons (the signature is the link).
- The lines of explanation under From a seed, Keep wallpapers for, Sign saved
  images, Logs, Detailed logging and Diagnostics, at the top of Content, at the
  foot of Live, and for a scene spread across monitors.

### Fixed

- **Sharper text.** Settings draws letters on whole pixels; small text was
  soft before.

## [0.24.0] - 2026-11-22

### Added

- **About.** A new section in Settings says who made Clairlune (yam333), with a
  link to their GitHub, the version, and the fan-project notice (moved from
  General). About Clairlune in the menu opens it.
- **Saved images are signed.** PNGs and GIFs you save carry a small "clairlune
  · by yam333" in the bottom-right corner, written into the scene's own grid of
  text so it sits in the picture. Settings > Export > Sign saved images turns
  it off. The wallpaper on your desktop is never signed.

### Changed

- General's summary says whether Clairlune starts with the computer.

### Fixed

- Archive's summary says "kept forever" instead of "kept for 0 days".

## [0.23.0] - 2026-11-22

### Added

- **A wallpaper from its seed.** Settings > Rotation > From a seed brings back
  the wallpaper with that seed: exactly as it was when it's in the archive or
  the library, otherwise made again (the same scenes, on every monitor, for the
  same Content choices and theme).
- The seed under the sections sits in a small box: hover it and it offers to
  copy, click it to copy the seed.

### Changed

- Seeds always mean the same wallpaper: the recent wallpapers decide which
  seeds come up instead of changing what a seed makes.
- Each section in the sidebar sits on a small pane of smoked glass, easier to
  read over the flutes.
- Shorter Settings: most settings lose the line under them; Smoothness shows
  its frame rate in its menu; Also change at login is now Change at login.
- Esc no longer closes Settings or the other windows (it still closes an open
  menu).
- On a short window the picture of the wallpaper in the sidebar shrinks, or
  goes, instead of crowding the sections.

### Removed

- Pause from Settings (it stays in the menu); while changes are paused,
  Rotation shows Resume.

## [0.22.0] - 2026-11-22

### Changed

- **Settings, redesigned** ("Moonglass"), the same on Windows and macOS:
  - The sections sit on a pane of reeded glass with your wallpaper showing
    through it, each flute bending the scene a little; the page is smoked glass
    over a blurred copy of it.
  - Page titles in a serif (Instrument Serif), with a one-line summary under
    each: how many landscapes are picked, what the theme follows, how many
    monitors.
  - What's on screen now moved to the foot of the sidebar, on every page: a
    thumbnail, the scene's name, LIVE or STILL, the seed and when it changes
    next, with New wallpaper, Previous and Save to library beside it.
  - Landscapes are picked from tiles that show each one (drawn in the
    background the first time Settings opens, a few seconds); weather and
    activities are chips.
  - New switches, segmented controls and buttons, warmer ember accents and
    cooler darks.
  - On Windows the window draws its own minimize and close buttons in place of
    the title bar; on macOS the title bar is see-through over the glass.
  - The Library, Archive and Timeline windows take the new colours, controls
    and serif titles.
- "Buddy" in Settings is now called Clawd.

## [0.21.0] - 2026-11-22

### Added

- **Six magical landscapes:**
  - **Giant mushrooms:** towering toadstools with spotted domed caps, spores
    drifting between them.
  - **Crystal cave:** clusters of tall glowing crystals on the floor, more
    hanging from the ceiling.
  - **The great tree:** a colossal tree with its canopy across the sky, a round
    door in its trunk, a lit window and lanterns hanging from its boughs.
  - **Wizard's tower:** a crooked tower on a crag, its roof wrapped in a spiral
    of sparks, rocks floating around it.
  - **Standing stones:** a ring of weathered stones with glowing runes, an
    altar in the middle, mist in the grass.
  - **Lantern lake:** paper lanterns floating on still water and rising into
    the sky, a gate standing in the lake, a pagoda on the far shore.

  Windows, lanterns and runes glow brighter from golden hour on.

### Changed

- **A fairer mix of landscapes.** Activities that only happen in one or two
  landscapes (the ridge hike, ski touring, sledding) made those landscapes come
  up far more often than the rest; they now give way a little, so every
  landscape gets closer to its share.

### Fixed

- Tall things in the scenery (giant mushrooms, towers, crystals, the ruins'
  columns) fit small images such as GIFs instead of running off the top.

## [0.20.0] - 2026-11-22

### Added

- **Five landscapes:** a waterfall pouring into a misty pool, countryside with
  patchwork fields, a windmill and a barn, old ruins (a colonnade, an arch,
  fallen blocks), a savanna with acacias and table hills, and a swamp of
  cypress trees in still water.
- **Six things for Clawd to do:** watering flowers, playing the flute by a
  music stand, taking photos of the view, birdwatching with binoculars (when
  birds are about), building a sandcastle, and yoga by a rolled mat.
- **Four kinds of weather:** small flocks of birds and butterflies by day,
  dandelion seeds on the breeze, and shooting stars after dark.
- All of them are in Settings > Content, on by default; `clairlune-cli` has a
  `--weather` option.

## [0.19.0] - 2026-11-22

### Removed

- **The Linux app.** Clairlune's desktop app is for Windows and macOS only. Its
  Linux side (setting the wallpaper on GNOME, KDE, XFCE, Cinnamon, MATE and
  Sway, live wallpapers on Wayland and X11, the Linux tray and start at login)
  had never run on a real Linux desktop, so it's gone, and so are the `.deb`,
  the AppImage and the AUR files. 0.18.0 stays on the releases page; a Linux
  copy of it will keep offering this update, which has nothing for Linux, so
  turn off Check for updates there. On Linux, `cargo install clairlune-cli`
  still renders the scenes as PNGs and GIFs, and the scene and render crates
  are still tested on Linux.

### Fixed

- **Homebrew no longer asks for a password on upgrades.** The cask's uninstall
  step ran on every upgrade: it asked for an admin password, turned start at
  login off and stopped Clairlune if it had started at login. The
  start-at-login entry now goes only with `brew uninstall --zap`. One last
  upgrade from 0.18.0 still asks, because Homebrew uses the cask that was
  installed.

## [0.18.0] - 2026-11-22

### Removed

- **The Mac screen saver**, with Lock Screen in the menu, the Screen saver
  setting and its Permissions. Clairlune stays the wallpaper; for locking, use
  macOS's own Lock Screen. If an earlier version installed Clairlune's screen
  saver, this one removes it from System Settings > Screen Saver the first time
  it starts; if it was the one picked, pick another there.

## [0.17.7] - 2026-11-22

### Added

- **Mac: Lighter on Retina screens** (Settings > Live, off by default). Live
  mode's memory follows the screens' pixels (two screen buffers plus the scene,
  about 10-12 bytes a pixel), so several large external displays add up to
  hundreds of MB. Turned on, each Retina screen's scene is drawn at half its
  resolution and shown pixel-doubled: about a quarter of the memory and work,
  with slightly larger, softer text. The still frame matches; the screen saver
  keeps full size.

## [0.17.6] - 2026-11-22

### Added

- **Settings > General > Permissions:** everything Clairlune needs you to allow
  in the system's settings, each with Granted or an Open Settings button to the
  right place. On the Mac: start at login (Login Items), and with the screen
  saver on, Clairlune picked as the screen saver and the password asked right
  away (so it locks). On Windows: start at login (Settings > Apps > Startup).
  It checks again when you come back to Settings.

## [0.17.5] - 2026-11-22

### Fixed

- **No more quit and reopen after `brew upgrade`.** When a new copy of
  Clairlune is put where the running one lives (Homebrew upgrading it, or a new
  copy dragged over from the DMG), Clairlune restarts into it on its own within
  about half a minute: quietly, or with Settings open if it was.

## [0.17.4] - 2026-11-22

### Added

- **Mac: lock with the scene playing.** macOS's own Lock Screen shows a still
  picture for any wallpaper that isn't Apple's, and runs no other screen saver
  once locked. Lock Screen in Clairlune's menu (and Lock now in Settings)
  starts the screen saver instead: macOS locks behind it and the scene plays
  until the display turns off. Settings also opens Hot Corners, and says when
  macOS asks for the password only minutes after the screen saver starts.

## [0.17.3] - 2026-11-22

### Added

- **Mac: Login Items.** macOS lets you switch Clairlune off in System Settings
  > General > Login Items, and only you can switch it back on. When start at
  login is on but switched off there, Clairlune opens that pane for you, and
  Settings > General says so, with an Open Login Items button.
- **A proper DMG window:** opening the DMG shows a Clairlune night scene with
  the app and an arrow to Applications to drag it onto, and the one-time Open
  Anyway step written underneath.

## [0.17.2] - 2026-11-22

### Added

- **Mac: pause behind a zoomed window.** Pause behind full-screen apps also
  pauses a screen when a window fills it but for the menu bar and the Dock (a
  double click on the title bar), or when windows tiled side by side fill it.

### Changed

- Settings > General is quieter: Check for updates just says once a day, and
  Quit, Restore my wallpaper and Remove Clairlune need no explanation.

## [0.17.1] - 2026-11-22

### Changed

- **Live on every Mac:** a Mac still on the old default (the still frame)
  switches to Live once, with a note saying how to go back (Settings >
  Display). A still frame chosen after that stays.

## [0.17.0] - 2026-11-22

### Added

- **A welcome on the first run:** one screen with what Clairlune does and the
  few choices that matter most: live or still, the theme, one scene per monitor
  or one across all, and whether it starts with the computer.
- **Remove Clairlune** (Settings > General): puts your wallpaper from before
  back, turns off starting with the computer, removes the Mac screen saver,
  deletes Clairlune's settings, archive, library and logs, and removes the app
  itself where it can (the Windows uninstaller runs, the Mac app goes to the
  Trash, an AppImage is deleted). It asks first, and PNGs and GIFs you exported
  stay.
- A [user guide](docs/guide.md) with answers to common questions, this
  changelog, and a trailer on the project page.

### Changed

- **Live by default on the Mac** for new installs, as on Windows (Linux keeps
  the still frame by default until Live is tried on real desktops there).

## [0.16.0] - 2026-11-21

### Added

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

### Fixed

- Links open through Windows itself, so long ones arrive whole.

## [0.15.0] - 2026-11-21

### Added

- **More to see:** six landscapes (meadow, lake, volcano, islands, the Moon and
  a bamboo grove), five kinds of weather (petals, fog, blizzard, ash and
  sparkles) and six things for Clawd to do (blowing bubbles, meditating,
  dancing, catching snowflakes, sweeping, hot cocoa): sixteen landscapes and
  thirty-two activities in all.
- **Check for updates** once a day (Settings > General; it can be turned off),
  or by hand with Check now.

### Changed

- Look, Contrast and Zoom moved to the Content page.
- **Lighter on the Mac:** no Metal device any more; only the changed pixels are
  written into two shared surfaces that macOS shows as they are. About 30% less
  memory, 6 threads instead of 10, about a quarter of the CPU.
- Lighter everywhere: no logging thread, only the theme on screen kept in
  memory, and the Windows frame timer sleeps when nothing plays.

### Fixed

- The Mac screen saver plays and animates in macOS's own screen saver host and
  ends cleanly on macOS 14 and later.

## [0.14.1] - 2026-11-21

### Changed

- The tray and menu bar icon is a single-colour crescent: white on Windows
  (black on a light taskbar), a template the Mac menu bar tints itself.

## [0.14.0] - 2026-11-21

### Added

- **A screen saver on the Mac** (Settings > General > Screen saver): the
  wallpaper keeps playing full screen when the Mac idles. Each new wallpaper
  becomes the screen saver too.

## [0.13.1] - 2026-11-21

### Added

- **Zoom:** Close shows Clawd as he has always been, Wide about half the size
  in a bigger landscape, Vast about a quarter. Each wallpaper picks one of
  those that are on.
- While live plays, the log notes CPU, memory and frame times once a minute.

### Changed

- New installs get a different contrast for each wallpaper.
- Scrolling scenes redraw about half as much.

### Fixed

- Mac: the download no longer reads as damaged (signed ad hoc; Open Anyway is
  still needed once). Battery saver and Pause on battery work on macOS and
  Linux.

## [0.13.0] - 2026-11-21

### Added

- **Depth:** scrolling scenes usually have a big landmark far behind (a granite
  dome, a lone peak, twin peaks, a volcano, a mesa, a huge rising moon). Two
  flights (the broom at dusk, the balloon by day) and a desert trek.
- **Starting with your computer:** the first run asks whether to start quietly
  at sign-in, now on macOS and Linux too. The tray icon can be hidden, with a
  Quit button beside it.

### Fixed

- Restore my wallpaper stays restored; Previous walks back through your
  wallpapers; picked wallpapers keep their theme; a typo in settings.toml no
  longer resets every setting; a damaged archive shows an error instead of
  crashing; Linux and macOS monitor fixes.

## [0.12.0] - 2026-11-21

### Added

- **A dithered look:** scenes drawn as light and dark masses of dots and bars,
  and a contrast setting.
- **Journeys:** ski touring, sledding and a forest walk; all journeys move at a
  walking pace, and are skipped at low frame rates.
- "New wallpaper on" gives one monitor a new scene; double-click the tray icon
  for Settings; chips for picking content.

### Fixed

- Daytime-only and night-only scenes no longer land in the wrong theme.
- Live keeps playing while you drag Clairlune's windows.

## [0.11.0] - 2026-11-20

### Added

- **The first journey:** Clawd hikes a mountain ridge while the land goes by in
  layers.
- **Indoors:** a room with a window onto another landscape.
- **Eleven activities**, from skipping stones to a hot-air balloon ride.
- One scene across all monitors, and a theme schedule (set times, or sunrise
  and sunset).
- Linux live wallpapers on wlr layer-shell compositors and plain X11 window
  managers.

## [0.10.0] - 2026-11-20

### Added

- Richer desert, shore and night sky; fishing and kite flying.
- Pause when an app covers the screen; the Archive window; crash reports and a
  diagnostic bundle.
- Wallpapers on each Mac screen, kept across Spaces.

### Changed

- The same calm pace at every smoothness; Direct3D 11 live wallpapers on
  Windows that copy only what changed; windows give their memory back.

[Unreleased]: https://github.com/yam333/Clairlune/compare/v0.33.2...HEAD
[0.33.2]: https://github.com/yam333/Clairlune/releases/tag/v0.33.2
[0.33.1]: https://github.com/yam333/Clairlune/releases/tag/v0.33.1
[0.33.0]: https://github.com/yam333/Clairlune/releases/tag/v0.33.0
[0.32.0]: https://github.com/yam333/Clairlune/releases/tag/v0.32.0
[0.31.1]: https://github.com/yam333/Clairlune/releases/tag/v0.31.1
[0.31.0]: https://github.com/yam333/Clairlune/releases/tag/v0.31.0
[0.30.2]: https://github.com/yam333/Clairlune/releases/tag/v0.30.2
[0.30.1]: https://github.com/yam333/Clairlune/releases/tag/v0.30.1
[0.30.0]: https://github.com/yam333/Clairlune/releases/tag/v0.30.0
[0.29.1]: https://github.com/yam333/Clairlune/releases/tag/v0.29.1
[0.29.0]: https://github.com/yam333/Clairlune/releases/tag/v0.29.0
[0.28.0]: https://github.com/yam333/Clairlune/releases/tag/v0.28.0
[0.27.0]: https://github.com/yam333/Clairlune/releases/tag/v0.27.0
[0.26.0]: https://github.com/yam333/Clairlune/releases/tag/v0.26.0
[0.25.0]: https://github.com/yam333/Clairlune/releases/tag/v0.25.0
[0.24.0]: https://github.com/yam333/Clairlune/releases/tag/v0.24.0
[0.23.0]: https://github.com/yam333/Clairlune/releases/tag/v0.23.0
[0.22.0]: https://github.com/yam333/Clairlune/releases/tag/v0.22.0
[0.21.0]: https://github.com/yam333/Clairlune/releases/tag/v0.21.0
[0.20.0]: https://github.com/yam333/Clairlune/releases/tag/v0.20.0
[0.19.0]: https://github.com/yam333/Clairlune/releases/tag/v0.19.0
[0.18.0]: https://github.com/yam333/Clairlune/releases/tag/v0.18.0
[0.17.7]: https://github.com/yam333/Clairlune/releases/tag/v0.17.7
[0.17.6]: https://github.com/yam333/Clairlune/releases/tag/v0.17.6
[0.17.5]: https://github.com/yam333/Clairlune/releases/tag/v0.17.5
[0.17.4]: https://github.com/yam333/Clairlune/releases/tag/v0.17.4
[0.17.3]: https://github.com/yam333/Clairlune/releases/tag/v0.17.3
[0.17.2]: https://github.com/yam333/Clairlune/releases/tag/v0.17.2
[0.17.1]: https://github.com/yam333/Clairlune/releases/tag/v0.17.1
[0.17.0]: https://github.com/yam333/Clairlune/releases/tag/v0.17.0
[0.16.0]: https://github.com/yam333/Clairlune/releases/tag/v0.16.0
[0.15.0]: https://github.com/yam333/Clairlune/releases/tag/v0.15.0
[0.14.1]: https://github.com/yam333/Clairlune/releases/tag/v0.14.1
[0.14.0]: https://github.com/yam333/Clairlune/releases/tag/v0.14.0
[0.13.1]: https://github.com/yam333/Clairlune/releases/tag/v0.13.1
[0.13.0]: https://github.com/yam333/Clairlune/releases/tag/v0.13.0
[0.12.0]: https://github.com/yam333/Clairlune/releases/tag/v0.12.0
[0.11.0]: https://github.com/yam333/Clairlune/releases/tag/v0.11.0
[0.10.0]: https://github.com/yam333/Clairlune/releases/tag/v0.10.0
