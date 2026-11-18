# Clairlune: Build Plan

A cross-platform desktop app, written in Rust, that procedurally generates animated ("live") ASCII-art desktop wallpapers featuring Clawd as hand-drawn pixel art made by this project (optional via a Buddy toggle). It runs in the background, rotates wallpapers on a schedule, supports light and dark themes, exports GIF and PNG, and archives past wallpapers with a retention policy. No AI and no network are used at runtime. Everything is seeded, deterministic, local procedural generation.

This document is self-contained. The session building this app has no other context.

**Name: Clairlune.** A nod to Claude Debussy's "Clair de Lune", and a fit for the night scenes, without using Anthropic's marks. Keep the name in one place: a single `APP_NAME = "Clairlune"` / `APP_ID = "clairlune"` constant pair used for window titles, the tray tooltip, config and data directory names, and the bundle identifier (reverse-DNS, for example `io.github.<owner>.clairlune`, owner decided at publish time). Crate and binary names: `clairlune` (app) and `clairlune-cli` (dev tool). Never hardcode "Clawd" or "Claude" in identifiers. Before the first public release, check "Clairlune" in the USPTO and EUIPO trademark databases (crates.io and GitHub were checked on 2026-11-18 and were free).

---

## 1. Product requirements

### 1.1 Core behavior
- Runs in the background as a tray / menu-bar app on **Windows, macOS and Linux**.
- Every N minutes or hours (set in Settings), it generates a new wallpaper and applies it to the desktop.
- Wallpapers are **live**: Clawd and scene elements such as fire, particles and stars animate on the desktop. A setting switches between **Live** and **Still frame** mode (§2, §8).
- Every wallpaper is **unique but coherent**. Generation is random but rule-constrained, so it never looks broken.
- Generation is **cheap**: low CPU, low memory and low power, including during idle animation.

### 1.2 Themes
- Light and dark palettes for every scene.
- Theme modes:
  - Light
  - Dark
  - Follow system
  - Schedule, using fixed times (for example dark from 19:00 to 07:00) or optionally sunrise and sunset computed locally from a user-entered latitude and longitude. No network is used.
- Switching theme **re-renders the same scene** in the other theme when the scene supports both (the recipe stores a time of day for each theme, §4.2). A scene whose activity only exists in one theme (for example stargazing, night only) is **theme-locked**: switching theme rolls a new scene for the new theme and archives the old one.

### 1.3 Resolution and aspect ratio
- Presets: 720p, 1080p, 1440p (2K), 2160p (4K), **Match screen** (default), and Custom (width × height).
- **Match screen = the monitor's physical pixel size** (for example 5120×2880 on a 5K iMac, not 2560×1440 points). Live windows are sized in physical pixels with a 1:1 texture mapping.
- Aspect ratio presets: **Match screen** (default), 16:9, 16:10, 21:9, 32:9, 4:3, 3:2, 9:16 (portrait), and Custom. A resolution preset sets the height, and the aspect ratio sets the width. For example, 1080p + 21:9 = 2520×1080.
- When the generated size differs from the screen, the wallpaper is set with the OS "fill" mode (scale to cover, crop the overflow). Live mode scales the same way, with nearest-neighbor sampling.
- Multi-monitor: see §2.1.

### 1.4 Export
- **Live export: GIF** (the user's choice). See §6.4 for how to keep 4K GIFs small.
- **Still export: PNG** by default. JPEG is optional with a quality slider.
  - Recommendation: PNG. ASCII art is sharp glyph edges on flat color, and PNG is lossless and compresses this content very well. JPEG blurs glyph edges and adds artifacts around text.
- Export actions: from the tray menu ("Save current as GIF / PNG"), from the library, and from the archive browser. The CLI (§3.1) is a developer tool and is not shipped to users.

### 1.5 History and archive
- When a wallpaper is replaced, the previous one is archived.
- An archive entry holds:
  - The **scene recipe**, a small JSON file with the seed and parameters. It reproduces the scene at any resolution or theme.
  - A **PNG thumbnail** (for example 480 px wide).
  - Optionally, the full PNG and/or GIF, depending on the user's setting.
- Retention is set in Settings (for example 1 day, 7 days, 30 days or forever), plus an optional max archive size in MB.
- Archive entries can be **pinned** (favorited). Pinned entries are never auto-deleted.
- Archive browser: a grid of thumbnails with actions to re-apply, export GIF, export PNG, pin and delete.

### 1.6 Clawd sprites (our own art)
- Clawd is the app's character. **All Clawd sprites are drawn by this project from scratch.** No third-party sprite files are used, downloaded or imported: not the davecurry.net `clawd.zip`, not frames captured from the Claude FM stream, and not assets from Anthropic or other fan projects.
- Our sprites are original drawings of the character, made in our own pixel style. They live in the repo as source files and exported sprite sheets (§7).
- **Buddy toggle**: Settings → Buddy → "Show Clawd in wallpapers" (default **on**). Off removes Clawd from new and current wallpapers (§7.2 step 7).
- Clawd is still Anthropic's character, so drawing our own sprites does not remove all legal risk. See §15.

### 1.7 Non-goals for v1
- No AI or ML, no network calls, no telemetry.
- No video formats (MP4 or WebM). GIF and PNG/JPEG only.
- No sound.

---

## 2. Critical platform reality: what "live wallpaper" means per OS

The OS wallpaper APIs only set **static images**. A live wallpaper requires the app to run its own **borderless window placed in the desktop layer, behind the desktop icons**, and render the animation into it. This is how Wallpaper Engine and Lively work. Every platform also needs a **static fallback**: set the still PNG via the native API.

| Platform | Static set (fallback) | Live (animated) host |
|---|---|---|
| **Windows 10/11** | `IDesktopWallpaper::SetWallpaper` (per-monitor) or `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` | Send `0x052C` to `Progman` to spawn a `WorkerW` window, find the `WorkerW` behind `SHELLDLL_DefView`, convert our window to `WS_CHILD`, then `SetParent` it into the `WorkerW`. **Windows 11 24H2 (build 26100) changed this tree**: the `WorkerW` and `SHELLDLL_DefView` became children of `Progman`, and Lively and Wallpaper Engine had to ship fixes. Implement both layouts and detect at runtime. The process must be Per-Monitor DPI Aware v2. Child coordinates are relative to the virtual screen, whose origin can be negative. Re-attach if Explorer restarts by listening for `TaskbarCreated` on a hidden **top-level** window (message-only windows don't receive it). On exit, re-set the static wallpaper, because a stale `WorkerW` can keep showing the last live frame. |
| **macOS** | `NSWorkspace.shared.setDesktopImageURL(_:for:options:)` per `NSScreen`. It only affects the **current Space** on that screen, so re-apply on `NSWorkspaceActiveSpaceDidChangeNotification`. | One borderless `NSWindow` per screen at level `kCGDesktopWindowLevel`, with `collectionBehavior` = `canJoinAllSpaces` + `stationary` + `ignoresCycle`, `ignoresMouseEvents = true`. |
| **Linux X11** | DE-specific (see below) or `feh`-style root pixmap | A window with `_NET_WM_WINDOW_TYPE_DESKTOP`, sized per output, below all others (the xwinwrap technique). **Limitation**: where a desktop-icon manager draws the wallpaper and icons together in one full-screen window (xfdesktop, nemo-desktop, caja, GNOME's DING extension), our window either covers the icons or is hidden. Detect these and offer "Live (hides desktop icons)" or fall back to Still. |
| **Linux Wayland, wlroots (Sway, Hyprland, river…)** | **No OS wallpaper API.** Still mode = our own layer-shell background surface showing the PNG with no animation. Optionally use `swaymsg output * bg` or hyprpaper IPC when present. | `wlr-layer-shell` surface on the **background** layer (Smithay Client Toolkit). |
| **Linux Wayland, GNOME** | `gsettings set org.gnome.desktop.background picture-uri` and `picture-uri-dark`. GNOME natively supports a separate dark-mode wallpaper, which maps nicely to our themes. | **No supported live path.** GNOME Mutter does not implement layer-shell. Fall back to static, swapping the PNG on schedule. Document this clearly in the UI. |
| **Linux KDE Plasma** | D-Bus `org.kde.PlasmaShell.evaluateScript` with a wallpaper script | layer-shell is supported on KDE Wayland, but plasmashell's own desktop view (wallpaper + icons) sits in the same background layer. A background surface may end up above the icons or hidden. **Verify in the M7 spike** before promising live on Plasma. If it fails, use Still, or a Plasma wallpaper plugin later. |
| **Linux others (XFCE, Cinnamon, MATE)** | `xfconf-query` / `gsettings` per DE | X11 desktop-type window, with the icon-manager limitation above |

Rules:
- A `WallpaperHost` trait abstracts this: `capabilities()` (supports live? per-monitor?), `set_static(monitor, path)`, `start_live(monitor) -> LiveSurface`, `stop_live()`.
- At runtime, detect the platform and DE and pick the best host. Show the active mode ("Live" or "Static fallback") in Settings with a reason when live is unavailable.
- **Always set the static wallpaper too**, even in live mode. If the app crashes or exits, the desktop shows the same scene instead of a stale one.
- **Unique file names**: macOS `setDesktopImageURL` and GNOME `picture-uri` don't reload an unchanged path, even if the file content changed. Write every static image to a new file name in `<data_dir>/current/`, set it, and only then delete the previous file.
- Live mode must **pause rendering** when:
  - a fullscreen app is focused
  - the desktop is fully occluded
  - the screen is locked or sleeping
  - the laptop is on battery, if the "Pause on battery" setting is on (default off; by default the app drops to Battery saver smoothness instead, §5.3)

  In these cases the last frame stays visible, so nothing appears broken.
- **Wallpaper mode setting: Live or Still frame.** This appears in Settings → Display and in the tray menu.
  - Switching applies immediately, with no new scene. Live → Still: the live window closes, and the hero-frame PNG (already set as the static wallpaper) remains. Still → Live: the live window opens over it, and the animation starts at the **hero time** `t_hero` (§4.4), so the first live frame is identical to the static image.
  - Still mode uses the least power: there's no window and no GPU work, and the app only wakes to generate on schedule.
  - If live is unsupported on the current platform (for example GNOME Wayland), the Live option is disabled with the reason shown, and Still is used.

The `wallpaper` crate covers static setting on many DEs and is a reasonable starting point. Verify each DE it claims and fill gaps (per-monitor on Windows, GNOME `picture-uri-dark`, macOS per-screen) with direct implementations.

### 2.1 Multi-monitor model
- **Default: per-monitor scenes.** Each monitor gets its own scene at its own physical resolution. The seed for monitor *i* is `hash(base_seed, monitor_id)`, where `monitor_id` is a stable id (EDID/connector name), not the enumeration index.
- **One archive entry per "apply"**, holding one recipe per monitor. Archive, library and timeline items are **multi-monitor aware**: a library item saved from a 2-monitor setup applies each recipe to the matching monitor. On a different setup, recipe 1 goes to the primary monitor and the rest are derived from it with `hash(seed, monitor_id)`.
- **Tray actions** (Save current, Previous wallpaper, Save to library, Export) act on **all monitors** as one entry. Export asks which monitor when there is more than one.
- **Span mode** (later milestone): one scene across the bounding box of all monitors, cut per monitor. Gaps and different DPIs are handled by composing in physical pixels.
- **Hotplug, resolution, rotation and DPI changes**: re-enumerate on OS notifications, re-render the current recipes at the new sizes (same seeds), and restart the live hosts. Mixed-DPI setups render each monitor at its own physical size.


---

## 3. Architecture

### 3.1 Cargo workspace

```
clairlune/
├── Cargo.toml                 # workspace
├── crates/
│   ├── scene/                 # pure, deterministic: seed -> SceneRecipe -> SceneGraph. No OS deps, no I/O.
│   ├── render/                # glyph atlas, CPU rasterizer, frame compositor, GIF/PNG encoders
│   ├── sprites/               # load our sprite sheets, catalog/tagging, atlas building
│   ├── platform/              # WallpaperHost impls per OS, monitor enumeration, system theme, power state, fullscreen detection
│   ├── live/                  # live renderer: wgpu surface in desktop-layer window, frame scheduler
│   ├── store/                 # settings (TOML), archive (recipes, thumbnails, files), retention cleanup
│   ├── app/                   # tray, settings window, scheduler, orchestration (the binary)
│   └── cli/                   # dev-only tool, not shipped: `clairlune-cli generate --seed 42 --theme dark --size 3840x2160 --out x.png`
├── assets/
│   ├── fonts/                 # OFL fonts (monospace for glyphs, sans for UI), bundled
│   ├── clawd/                 # our Clawd sprite sources (src/), exported sheets, catalog.toml
│   └── activities/            # one TOML file per activity (§4.7)
├── docs/                      # design.md, clawd-style.md, takedown.md, sprites notes
├── reference/
│   └── prototype_generate.py  # the Python prototype (§13)
└── tests/golden/              # golden-image snapshots at small resolution
```

`scene` and `render` must compile and run headless on every OS and in CI. All platform-specific code lives in `platform` and `live`.

### 3.2 Recommended crates

| Purpose | Crate |
|---|---|
| Deterministic RNG | `rand_chacha` (`ChaCha8Rng`) as the raw bit source only. Do **not** use `StdRng`, and do **not** use `rand` distributions (`gen_range`, `WeightedIndex`, float sampling): their output changed between `rand` versions. Hand-write integer sampling (rejection sampling for ranges, integer-weight picks). See §4.6. |
| Math in `scene` / `render` | `libm` for `pow`, `exp`, `sin` and similar. `f32::powf` and friends call the platform's math library, which differs between glibc, macOS and MSVC and would break cross-OS determinism. |
| Noise | `fastnoise-lite` (OpenSimplex2, FBM, ridged) or a small hand-written value/simplex noise (preferred if it keeps deps small) |
| Font rasterization | `fontdue` (rasterizes once into a glyph atlas) |
| Image buffers / JPEG | `image` (`jpeg` feature) |
| PNG (including indexed / paletted PNG) | `png` crate directly. `image` cannot write indexed PNGs. |
| GIF decode/encode | `gif` |
| Windowing | `winit` (one event loop for the whole process, §3.4) |
| GPU (live mode) | `wgpu` (low-power adapter preference) |
| Settings UI | `egui` with `egui-winit` + `egui-wgpu` (**not** `eframe`, which owns its own event loop), with a **fully custom theme**, or `Slint`. Decide with a spike at the start of M6 against the design bar in §10a and the event-loop rules in §3.4. egui's default look is the "programmer tool" style §10a forbids, so it is only acceptable if the restyle reaches the bar. |
| Tray icon and menu | `tray-icon` + `muda`. On Linux these need a GTK main loop and `libayatana-appindicator` (packaging dependencies). |
| Start at login | `auto-launch`. On macOS call `set_use_launch_agent(true)`, otherwise it uses AppleScript login items. |
| System dark/light | `dark-light` |
| Sunrise/sunset (local) | `sunrise` |
| Config / recipes | `serde`, `toml`, `serde_json` |
| Paths | `directories` |
| Single instance | `single-instance` to detect a running copy, plus a local socket (`interprocess`) so a second launch can tell the first to open Settings |
| Logging | `tracing`, `tracing-appender` (rolling file in the data dir) |
| Windows APIs | `windows` crate |
| macOS APIs | `objc2`, `objc2-app-kit` |
| Linux | `smithay-client-toolkit` (layer-shell), `x11rb`, `zbus` (KDE D-Bus) |

### 3.3 Data flow

```
seed (u64) + aspect ratio + content context (§4.6)
        │
        ▼
 scene::compose(seed, aspect, content_ctx) ──► SceneRecipe (serializable JSON, ~1–3 KB, theme-independent)
        │
        ▼
 scene::build(recipe, size, theme, buddy) ──► SceneGraph
        │                   ├── static layers: grid of (glyph_id, shade) per layer
        │                   └── animated elements: sprite instances, particle emitters, flickers
        ▼
 render::bake_static(graph) ──► RGBA background buffer (rendered ONCE)
        │
        ├──► still PNG/JPEG  = background + all animated elements at the hero time t_hero (§4.4)
        ├──► GIF             = background + animated elements over one loop (§6.4)
        └──► live            = background uploaded once as a GPU texture, animated elements drawn per frame
```

The **recipe is the source of truth**. Archive it, not the pixels. Any archived scene can be re-rendered at any size, in either theme it supports, with or without Clawd. That makes theme switches and the Buddy toggle free.

### 3.4 Event loop and threads
- winit allows **one `EventLoop` per process**, and on macOS it must run on the main thread. So the `app` crate owns a single event loop that hosts:
  - the tray and its menu events
  - the live wallpaper windows (one per monitor)
  - the UI windows (Settings, Library, Archive, Timeline), **created and dropped on that same loop** when opened and closed. This is why `eframe` is not used.
- **Linux**: run GTK for the tray on its own thread. Wayland layer-shell surfaces can't be created through winit, so the wlroots/KDE live host uses its own smithay-client-toolkit + `calloop` path on a dedicated thread, handing wgpu a surface through `raw-window-handle`.
- If **Slint** wins the M6 spike, Slint's event loop becomes the host loop, and the same rules apply.
- Background work (scene generation, GIF encoding, retention sweeps) runs on worker threads and reports back through the event loop's user events.
- The M6 spike's pass criteria include: tray + one live window + opening and closing Settings three times, all on one loop, on all three OSes.


---

## 4. The generation system (the core of the app)

Goal: wallpapers that are unique, creative and coherent, and cheap to produce. The approach is a **grammar of scene archetypes**, filled in by **seeded weighted choices**, with each layer textured by **noise mapped to glyph ramps**, all inside **hard composition constraints**.

### 4.1 Grid model
- The image is a grid of character cells. Cell size derives from the target resolution: aim for about **90–120 rows** regardless of resolution, so the look is consistent from 720p to 4K. At 4K that is roughly 11×20 px cells. At 720p, 108 rows would mean about 4×7 px cells, which is too small, so cap the minimum cell size (for example 6×11 px) and allow fewer rows at low resolutions.
- Each cell stores `(glyph: u8 index into glyph set, shade: u8 0..=N, tint: u8 palette slot)`. A 4K grid is about 350×108, so 38k cells × 3 bytes ≈ 115 KB.
- All layout math uses **normalized coordinates** (0..1). Any aspect ratio works, including portrait. Composition rules adapt: portrait stacks the scene vertically, ultrawide adds more side scenery.

### 4.2 Scene recipe (what the seed decides)

The `SceneRecipe` is produced by sequential, seeded, **weighted table** picks. Each pick constrains later picks.

**Clawd is integral to the scenes.** Each wallpaper is built around **what Clawd is doing**, such as roasting marshmallows at a campfire, skiing down a snowy slope, fishing off a pier, or stargazing on a rooftop. So the **activity is always picked first** and drives everything else (§4.7):

0. **Activity**: a weighted pick from the enabled activity catalog, down-weighted if used recently (§4.6). The activity declares which capabilities, weather and times of day it needs, so picks 1–6 below only choose **among the options the activity allows**.
   - **This step always runs, whether Buddy is on or off.** Buddy is a **render** flag (§3.3): with Buddy off, the chosen activity's **unattended variant** is drawn instead of Clawd, for example skis stuck in the snow beside fresh tracks, an empty boat rocking at a pier, or a telescope pointed at the stars. That is what lets the Buddy toggle keep the exact same scene.
   - **No eligible activity** (content filters or a small v1 catalog): fall back to an archetype-first scene with no activity and no character, and log it.

1. **Archetype** (landscape template). It defines the layer stack and the composition, and declares its **capabilities** (§4.7). Ship these 9 in v1, with snake_case ids:
   - `canyon` (cliffs left and right, open center). This matches the Claude FM look and is the reference scene.
   - `forest` (vertical trunk streaks, canopy band)
   - `mountains` (ridged noise silhouettes, 3–4 parallax ridges)
   - `shore` (horizon line, wave rows animated, pier platform)
   - `city_rooftops` (rectangular blocks, lit windows that flicker)
   - `desert` (smooth low-frequency ridges, heat shimmer particles)
   - `snowfield` (sparse ridges, falling snow particles)
   - `night_sky` (stars, moon, aurora bands)
   - `cave` (stalactites from the top, glowing crystals)
2. **Time of day, one per theme**: the recipe stores `light_tod` (one of `morning`, `noon`, `golden_hour`) and `dark_tod` (one of `dusk`, `deep_dusk`, `night`), both picked from the activity's allowed times. The active theme selects which one renders. It drives palette variant, sky elements (sun, moon, stars) and particle types. If the activity allows no time for one theme, that field is empty and the recipe is **theme-locked** (§1.2).
3. **Platform** where Clawd stands, compatible with the archetype: log, rock ledge, pier, rooftop edge, tree branch, floating island, dune crest, ice shelf.
4. **Prop(s)** next to Clawd: campfire, lantern, tent, desk with laptop, fishing rod, telescope, guitar, books, coffee mug, mailbox. Props are **procedurally drawn pixel sprites** we author in code or as tiny embedded pixel maps (like the prototype's campfire). They are drawn by this project, like the Clawd sprites.
5. **Clawd placement and motion** (Buddy on): where Clawd is placed and how he moves comes from the activity: a fixed spot, pacing along the platform, a path down a slope, or bobbing in a boat (§4.7).
6. **Weather / particles**: none, dust, embers, snow, rain, fireflies, falling leaves, stars twinkle, confetti. Each must be compatible with the archetype and the time of day.
7. **Composition variables**: horizon height (35–75% of height), Clawd x position (on a rule-of-thirds line or center, ±jitter), platform extent, layer count (2–4), gap width, mirrored underbelly or reflection on/off.
8. **Palette**: one palette family per theme (§4.5) plus one accent hue. The recipe stores both, and the active theme selects one.

**Weather and particles that depend on time of day** (stars, fireflies) are chosen per theme as part of the time-of-day pick, so each theme's render stays coherent.

Coherence comes from three mechanisms:
- **Activity definitions** (§4.7) plus compatibility tables (archetype × platform × prop × weather), declared in data files, with weights.
- **Hard constraints** checked after composition:
  - Clawd's feet sit on the platform's top surface, or on the activity's motion path (a slope surface, a boat deck). The same placement is used for the unattended variant, so both Buddy states share one composition.
  - Every prop the activity needs is present and placed at its declared anchor. For example, the fishing line ends in water, and the kite string reaches Clawd's hand.
  - Clawd's whole motion path stays inside the clear focal area and the safe frame. Nothing goes off-screen or behind a dense layer.
  - Props don't overlap Clawd.
  - Clawd plus props have a clear "breathing" margin of N cells where background layers are cleared or dimmed. The prototype does this, and it is what makes the character readable.
  - The focal point is never inside the darkest or densest layer.
- **Retry**: if a constraint fails, re-roll only the failing sub-choice with the next RNG draw, so it stays deterministic. Cap at 8 retries, then fall back to a known-good default for that slot.

### 4.3 Layers → glyphs

Each archetype defines layers back to front (sky, far, mid, near, platform, foreground). For each layer:

- **Silhouette**: a 1D height profile from fractal noise (ridged for mountains and cliffs, smooth for dunes), shaped by an archetype function. For example, canyon = high at edges, low in the center: `d = |x - c| / 0.5` (x and c normalized 0..1), `shape = smoothstep(0, 1, clamp((d - gap) / (1 - gap), 0, 1))^p`.
- **Fill texture**: 2D fractal noise with **anisotropic scale**. Tall thin cells give vertical streaks (cliffs, trees). Wide cells give horizontal strata (dunes, water).
- **Density → glyph**: each layer has its own **glyph family**, ordered light to heavy. Density selects the index, plus a small seeded jitter (±0.6 of a step) so it doesn't look banded. Proven families from the prototype:
  - far hatch: `'-/|` (mirror `\` on the right side)
  - mid hatch: `.-=\<` / `.-=/>`
  - near dense: `.:ox=O#X%@`
  - platform heavy: `=o+x#X%@` with a `^` top edge
  - water: `~-=≈` (stay ASCII-safe: `~-=`)
  - sky dust / stars: `.` `'` `` ` `` `,` `:` `*`
- **Holes**: density below a per-layer threshold leaves the cell empty, which shows layers behind. This is the main source of depth.
- **Depth shading**: the shade level scales by layer depth (far ≈ 0.35, mid ≈ 0.6, near ≈ 0.9 of max ink). A top-edge fade (first ~6 rows of a silhouette dimmer) softens ridgelines.
- **Painter's order**: nearer layers overwrite farther ones only where they place a glyph.

Clear the cells around the focal group (Clawd, props) before painting particles.

### 4.4 Animated elements (the "live" part)

Keep the animated set **small and cheap**. The static background never re-renders.

- **Clawd**: driven by the activity's animation state machine and motion path (§4.7). Scale with **integer nearest-neighbor** only, to keep pixels crisp. Pick the largest integer scale that makes Clawd about 6–9% of screen height.
- **Activity effects**: particles and props tied to what Clawd does, positioned from his per-frame anchor points. Examples: snow spray behind the skis, a fishing line and ripples, music-note glyphs from the guitar, sand trailing a sandboard.
- **Prop animations**: fire flicker (2–4 hand-made pixel frames, randomized order), lantern glow pulse, lit windows toggling, wave rows shifting.
- **Particle emitters**: embers or smoke plume, snow, rain, fireflies, stars twinkling, confetti. Particles are **glyphs**, which keeps the ASCII look.
  - **Live (GPU)**: particles are glyph quads at **sub-cell pixel positions**, so higher Smoothness levels move them smoothly.
  - **GIF export, still frames and the CPU fallback**: particle positions snap to the cell grid. That keeps GIF frame differences small and the palette exact.
  - Fixed-size pools, for example ≤ 400 particles. No allocation per frame.
  - Seeded and **periodic**: each particle's motion is a function of `(t mod loop_length)`, so the animation loops seamlessly. This is required for GIF export and also free for live.
- **Glow**: a warm radial gradient around light sources (fire, lantern, torch), **quantized into a few discrete bands** (§4.5) and baked into the background. Glyphs inside the glow radius get tinted toward the accent color, also in discrete steps. An optional slow pulse in **live mode only** is drawn as a separate glow-band mask texture on the GPU. GIF and still frames never pulse.

**Two timelines, one scene:**
- **Live mode is not limited to a loop.** Clawd's state machine plays seeded variations over time. For example, while skiing he mostly glides, sometimes carves or jumps, and occasionally wobbles and tumbles into the snow. The desktop then doesn't look like a short repeating clip. The background effects (fire, snow, particles) stay periodic, because that is cheap.
- **GIF export** needs a seamless loop. The scene defines a **loop script**: a fixed sequence of activity states whose total length matches a loop period, for example 6 s. Each periodic element's own cycle is chosen to divide that period evenly, so the GIF loops with no jump.
- **Both modes are deterministic**: given the seed, the same moment in time always renders the same frame.
- **Time base**: `t` = integer milliseconds since `scene_epoch`, the moment the scene was applied (stored in the archive entry). The live renderer starts at `t_hero`, so it matches the static image. Pausing freezes rendering only; on resume it jumps to the current `t`.
- **Hero time** `t_hero`: picked deterministically from the seed. Among the first-state frames in the first 10 s, take the one where particles are most evenly spread (lowest clustering score). Still PNGs, thumbnails and the static wallpaper all use `t_hero`.

### 4.5 Palettes
- Hand-tuned palette families per theme, defined as data:
  - background
  - ink (glyph) color with N shade steps
  - accent (warm or cool)
  - sprite-friendly ground tone
- Light theme: cream or paper background (about `#ECE9DF`) with near-black ink. This matches the Claude FM look.
- Dark theme: warm near-black background (about `#151412`) with cream ink and orange ember accent. The prototype used these values.
- Clawd keeps its own sprite colors in both themes. Make sure the palette background contrasts with Clawd's salmon in both.
- **Limit total colors to ≤ 255 per scene rendering** (index 255 is reserved for GIF transparency). That lets GIF export use one global palette with **zero quantization cost** (§6.4). Every pixel must land on a palette entry, which requires compositing **by palette index, not by RGB blending**:

  | Budget | Entries |
  |---|---|
  | Background bands (vignette + glow quantized into bands) | 8 |
  | Ink levels per background band (glyph alpha quantized to 6 levels) | 8 × 6 = 48 |
  | Accent-tinted ink (glow region) | 4 tint steps × 6 levels = 24 |
  | Clawd sprite colors | ≤ 16 |
  | Prop and effect sprite colors | ≤ 48 |
  | Sky elements (sun, moon, aurora bands) | ≤ 24 |
  | Reserve | the rest, up to 255 |

  - The glyph atlas alpha is quantized to the 6 ink levels. A cell's pixel color is `palette[(background_band, ink_level, tint_step)]`, looked up, not blended.
  - The vignette and glow are quantized into the background bands (dithered with an ordered Bayer pattern at band edges if banding shows).
  - The live GPU path uses the same lookup, so live, still and GIF colors match exactly (§5.3).

### 4.6 Uniqueness without randomness drift
- Seed = `u64`. New scenes get a seed from the OS entropy, which is fine because the seed is stored.
- `compose` is a pure function of `(seed, aspect, content_ctx)`. `content_ctx` holds everything else that affects picks: enabled archetypes, weather and activities, the **recent-history list** (below), and a hash of the activity and catalog data files. The recipe records `content_ctx`'s hash. Pixels are a pure function of `(recipe, size, theme, buddy, t)`.
- **Separate RNG streams per subsystem**: `ChaCha8Rng::seed_from_u64(hash(seed, "activity"))`, `"terrain"`, `"props"`, `"particles"`, `"animation"` and so on. Adding an activity file or a retry then doesn't shift the landscape draws.
- Use `libm` and integer sampling (§3.2) so the same inputs give byte-identical output on every OS.
- Recipe JSON includes a `generator_version`. Changing generator logic bumps it, and old recipes re-render with the old behavior if feasible, or are flagged "approximate".
- Avoid near-duplicates: keep the last 20 `(activity, archetype, platform, prop)` tuples, persisted in `<data_dir>/state.json`, and down-weight repeats when picking. The activity is the strongest signal: avoid the same activity twice in a row, unless it is the only eligible one. The CLI and tests pass an empty history by default.

### 4.7 Activities: what Clawd is doing

Activities are the main unit of content. Adding a new activity, with its sprites and one data file, is how the app grows. The engine must make that easy, so the generator code never hardcodes a specific activity.

**No activity is special.** Campfires, skiing and the other activities named in this plan are illustrations. The engine treats every activity the same way. Default weights are equal, and a variety test (§12) checks that no single activity dominates.

**Definition** (`assets/activities/<id>.toml`, one file per activity). Requirements are **capabilities**, not archetype or platform ids. Archetypes declare what they offer (terrain, surface, water, platform types, room for props). A new archetype then works with existing activities automatically, and a new activity works with existing archetypes, with no cross-editing.

Example (skiing, chosen because it exercises path motion):

```toml
id = "ski_downhill"
weight = 1.0

[requires]
terrain = ["slope"]
surface = ["snow"]
time_of_day = ["morning", "noon", "golden_hour", "dusk", "night"]
weather = ["none", "snow"]

[motion]
type = "path"
path = "slope"
speed_cells_per_s = [5, 8]
at_path_end = "fade_and_restart"

[props.tracks]
attach = "path"
effects = ["ski_tracks"]

[animation]
start = "glide"

[animation.states.glide]
loop = true
dwell_s = [3, 8]
next = { carve = 0.6, jump = 0.25, wobble = 0.15 }

[animation.states.carve]
next = { glide = 1.0 }

[animation.states.jump]
next = { glide = 1.0 }

[animation.states.wobble]
next = { glide = 0.8, tumble = 0.2 }

[animation.states.tumble]
next = { glide = 1.0 }

[effects_by_state]
glide = ["snow_spray_small"]
carve = ["snow_spray_large"]
tumble = ["snow_puff"]

[gif]
loop_script = ["glide", "carve", "glide", "jump"]

[unattended]
props = ["ski_tracks", "skis_in_snow"]
```

**Semantics** (the building session implements exactly these):
- **Requirements**:
  - Each list must match at least one capability of the chosen archetype, time of day and weather.
  - The recipe picks a light time (`morning`, `noon`, `golden_hour`) and a dark time (`dusk`, `deep_dusk`, `night`) from `time_of_day` (§4.2). An activity that allows only one family produces theme-locked scenes.
- **States**:
  - A `loop = true` state repeats for a seeded duration within `dwell_s`, then picks `next` by weight.
  - A state without `loop` plays once, then picks `next`.
  - Weights in `next` are normalized. A state with no `next` returns to `start`.
- **Props**:
  - `attach` is one of `path`, `left_of_clawd`, `right_of_clawd`, `either_side` (seeded), or a named archetype prop slot.
  - For stationary activities, Clawd's sprite is mirrored so he faces his main prop.
- **Motion**: `speed_cells_per_s` and similar ranges are drawn once per scene from the seed.
- **GIF loop**: `loop_script` must start and end on the same pose (the first frame of its first state). Validation fails otherwise. To fill the chosen loop length (3, 6 or 10 s), the script's **first looping state is repeated** as many times as needed. Any remainder is absorbed by GIF delay dithering (§6.4).
- **Validation** at startup and in CI: every referenced state has a sprite tag, every effect and prop id exists, and every requirement uses a known capability.

**Engine responsibilities:**
- **Filtering**: activity → allowed archetypes, platforms, props, time of day and weather. Validate every activity file at startup and in CI. An activity whose sprites are missing is skipped, never crashed on.
- **Motion types** are implemented generically, so new activities reuse them:
  - `stationary`: fixed spot
  - `pace`: walk back and forth along the platform
  - `path`: follow a path the archetype exposes, for example a slope surface for skiing or sledding, a shoreline, or a rooftop edge. Paths are computed from the same silhouette data as the landscape, so Clawd really skis **on** the generated slope.
  - `bob`: float in a boat or on water, with wave motion
  - `fly`: on a kite or balloon, with a gentle drift
- **Archetypes expose anchors and paths**: platform surfaces, slope polylines, water lines, and prop slots, so activities can query them.
- **Anchor points per sprite frame**: hands, feet, head and held-item tip, exported from the art tool (§7.1). Effects and held props attach to them, so a fishing rod tip, a kite string or a torch flame follows the animation exactly.
- **State machine**: seeded transitions with dwell times, per the semantics above, as a pure function of `(seed, t)`. To keep evaluation cheap at any `t`, time is split into **episodes** of 60 s. Each episode is seeded by `hash(seed, "animation", episode_index)`, starts in `start`, and is simulated from its own start, so finding the state at `t` costs at most one episode of transitions. The state at an episode boundary is forced back to `start` through its neutral pose.

**Starter activity list.** Build these incrementally; the list is open-ended:

| Group | Activities |
|---|---|
| Cozy | roast marshmallows, warm hands at fire, sleep by the fire, read under a lantern, drink cocoa |
| Snow | ski down a slope, sled, build a snowman, catch snowflakes |
| Water | fish off a pier, row a boat, skip stones, surf a wave |
| Sky | stargaze with a telescope, fly a kite, ride a balloon, watch a meteor shower |
| City | code on a laptop on a rooftop, play guitar, water rooftop plants |
| Desert / cave | sandboard down a dune, explore with a torch, mine crystals |
| Celebration | fireworks, confetti, birthday cake |

---

## 5. Rendering and efficiency

### 5.1 Glyph atlas
- Bundle one OFL-licensed monospace font (for example JetBrains Mono). Don't rely on system fonts.
- At startup, or when the cell size changes, rasterize the ~100 glyphs used into an **alpha atlas** with `fontdue`. Quantize alpha to the shade ramp.
- Drawing a cell = blit the glyph alpha mask × palette color. No font shaping and no text layout.

### 5.2 Static bake (CPU)
- Render background + all static layers into one RGBA buffer once per scene. For a 4K scene, the target is **under 150 ms** single-threaded. Optionally parallelize by row bands with `rayon`.
- Peak memory for a 4K bake: one 33 MB RGBA buffer, plus small grids. Free it after uploading to the GPU (live) or after encoding (export).

### 5.3 Live renderer (GPU, minimal power)
- `wgpu` with `PowerPreference::LowPower`.
- Upload the baked background once as a texture. Each frame, draw:
  - background as one full-screen quad
  - the Clawd sprite frame from a small sprite atlas texture
  - prop frames
  - particles as instanced glyph quads sampling the glyph atlas
- **Smoothness** (the user-facing name for frame rate). Settings → Live → Smoothness:

  | Option | fps | Notes shown in UI |
  |---|---|---|
  | Battery saver | 6 | Lowest power. Clawd still animates, and motion is choppier. |
  | Balanced (default) | 12 | Matches the pixel-art cadence |
  | Smooth | 24 | Smoother skiing, particles and drift |
  | Very smooth | 30 | |
  | Ultra smooth | 60 | Highest power. Capped to the monitor's refresh rate. |

  - Sprites keep their drawn frame timing at every setting **above** their own rate. At Battery saver (6 fps), sprite frames are dropped to fit, so animations are choppier but keep their real speed. Higher settings interpolate **motion** (path movement, sub-cell particles, parallax drift, glow pulse) between sprite frames, so they look smoother without needing more art.
  - Show a small "power impact" hint next to each option.
  - **"Drop to Battery saver on battery"** is on by default. "Pause on battery" is a stricter option, off by default.
  - Present with `PresentMode::Fifo` and **sleep between frames** (`ControlFlow::WaitUntil`). Never busy-loop. Sprite and particle updates only run when their own timers tick.
- Pause rules from §2: fullscreen app, occluded, locked, on battery (optional).
- **Color consistency**: the live path must match the CPU-baked PNG exactly, or the switch from static to live visibly shifts. Upload textures as `Rgba8UnormSrgb`, sample with **nearest** filtering at 1:1, and draw sprites and particles from the same palette lookup with no alpha blending (§4.5). A golden test compares a GPU readback of `t_hero` with the CPU PNG.
- **Budgets** (targets; measure in an M7 spike before committing to them):
  - < 1–2% CPU and a negligible GPU share at 12 fps at 4K on a modern laptop
  - RSS in live mode: wgpu backend and driver initialization alone can cost 50–150 MB, plus about 33 MB per 4K background texture per monitor. Measure, then set the real number.
  - < 60 MB in static-only mode
- **CPU fallback**: if `wgpu` cannot initialize (some VMs or old GPUs), use `softbuffer`. Keep the baked background in RAM and composite only **dirty rectangles** around animated elements, at a lower default fps (8).

### 5.4 Scheduling generation
- New scene generation happens on a background thread at low priority. Build the new scene fully, then swap it atomically:
  1. set the static wallpaper
  2. swap the live texture
  3. archive the old scene
- Generate a few seconds **ahead of the scheduled time**, so the swap is instant.

---

## 6. Export formats

### 6.1 PNG (default still)
- The frame at `t_hero` (§4.4), rendered at the target resolution.
- Use `png` with adaptive filtering and default compression. ASCII scenes compress well, and the 4K prototype was about 2.2 MB. Write it as an **indexed (paletted) PNG** with the `png` crate, since every scene has ≤ 255 colors (§4.5). Files get much smaller.

### 6.2 JPEG (optional still)
- Quality slider, default 90. The UI notes that PNG is sharper for text art.

### 6.3 Where files go
- Live and static working files: `<data_dir>/current/`
- Exports: user-chosen folder, defaulting to `Pictures/Clairlune/`

### 6.4 GIF (live export)

GIF at 4K is normally huge and slow. Our design avoids both problems:
- **One global palette** for the whole file, built from the scene palette (≤ 255 colors plus one transparent index, by construction, §4.5). No per-frame quantization and no error-diffusion dithering.
- **Frame differencing**: for each frame, emit **exactly one** sub-image: the bounding box of changed pixels (the `gif` crate supports frame `left`, `top`, `width` and `height`). Multiple sub-images per frame would need zero-delay frames, which browsers clamp to 10 cs. Inside that box, unchanged pixels are set to the reserved transparent index, which helps LZW compression. Use `DisposalMethod::Keep`. Frame 0 is the full background.
- Exactly one **seamless loop** (§4.4), with `Repeat::Infinite`.
- **Smoothness also applies to GIF export**, capped by the format. GIF frame delays are whole centiseconds, and most viewers and browsers treat delays under 2 cs as 10 cs.
  - Battery saver ≈ 6.25 fps (16 cs)
  - Balanced 12.5 fps (8 cs)
  - Smooth 25 fps (4 cs)
  - Very smooth 33 fps (3 cs)
  - Ultra smooth is capped at 50 fps (2 cs)

  - **Delay dithering**: the delays above don't always divide the loop length evenly (for example 6 s at 16 cs = 37.5 frames). Alternate delays (for example 16 and 17 cs) so the total equals the loop length exactly.

  Higher smoothness means more frames and bigger files.
- Options:
  - Export size: Native, 1080p or 720p. Many GIF viewers struggle with 4K. Smaller sizes are **rendered natively from the recipe** at that size, not downscaled, because downscaling creates colors outside the palette.
  - Loop length: 3 / 6 / 10 s
- **Size**: scenes with full-screen weather (snow, rain, twinkling stars) change most of the frame every frame, so 4K GIFs can reach tens of MB. Estimate the size with a short trial encode (the first second), show it before exporting, and warn above 25 MB. Record real sizes in tests.
- Encode on a background thread with a progress indicator in the tray or settings UI.

The live wallpaper itself never plays the GIF. It renders natively (§5.3), because decoding a 4K GIF every frame would waste CPU. GIF is for sharing only.

---

## 7. Clawd sprites: creation and pipeline

### 7.1 Creating our own sprites
- **Draw from scratch.** Use the character's public look as the only reference. Never trace, color-pick from, or frame-step anyone else's sprites or the Claude FM stream. Keep the drawing history (source files and git commits) as a record that the art is our own work.
- **Tooling: Aseprite is required** for sprite authoring. Anchors use Aseprite **slices with per-frame keys**, which LibreSprite (forked before slices existed) and Pixelorama don't export. Source files live in `assets/clawd/src/*.aseprite`. Exported **PNG sprite sheets + JSON frame data** live in `assets/clawd/`, and are embedded in the binary with `include_bytes!`.
  - `just export-sprites` runs the Aseprite CLI locally. CI does **not** run Aseprite, because it is paid software. Instead, the export script records each source file's hash in the exported JSON, and CI fails if a committed `.aseprite` file's hash doesn't match its export.
- **Style guide** (`docs/clawd-style.md`, written before drawing):
  - Canvas: a fixed **height** and a fixed **origin** (feet baseline row + body center column) for every frame. The canvas **width can vary per activity**, so skis, a guitar or a fishing rod fit. For example, base canvas 24×20 px, a ski sheet 40×20, a fishing sheet 48×28. Every animation then anchors identically, with no normalization pass.
  - Starting proportions from the prototype's pixel map (§13): a 15×9 sprite whose body block is 11×7, with side arms, two eye blocks and four legs. Refine the proportions in the style guide.
  - A small fixed palette: body `#D9785C`, a shadow and highlight shade of it, eyes `#261A16`. No anti-aliasing, 1 px rule throughout. Optional 1 px outline for the light theme, decided in the style guide.
  - Sprite animation is drawn at 8–12 fps (the pixel-art cadence). Per-frame durations go in the JSON. Higher Smoothness settings (§5.3) make movement, particles and parallax smoother. They **do not** add sprite frames.
- **Organized by activity.** There will be many sprites, so the workflow must scale:
  - One source file per activity: `assets/clawd/src/<activity>.aseprite`. Each **state** of the activity state machine (§4.7) is an animation **tag** in that file, for example `glide`, `carve`, `jump`, `wobble`, `tumble` for skiing.
  - **Anchor points** (hands, feet, head, held-item tip) are drawn as **slices** in the art tool and exported per frame into the JSON. The engine attaches props and effects to them.
  - **Held items** (ski poles, fishing rod, kite, guitar) go on separate layers, exported separately when they need their own effects. Otherwise they are baked into Clawd's frames.
  - **Transitions** between states start and end on a shared neutral pose, so state changes never pop.
  - A `assets/clawd/src/_template.aseprite` file has the canvas, palette, baseline guide and standard slice names, so every new activity starts consistent.
- **Base animations** shared by all activities: idle (breathe, blink, look around), walk, and a "react" set (celebrate, love, alert, dizzy) that activities can borrow as occasional states.
- **Authoring loop**: `clairlune-cli preview-activity <id> --seed N --theme dark --smoothness smooth` opens a window with the activity in a few compatible scenes and hot-reloads the sheets on file change. The art loop is then draw → save → see it in a real wallpaper.
- **Order**: ship v1 with idle, walk, and about five activities that cover different motion types and scene groups. For example: ski (`path`), fish off a pier (`stationary` + water effects), fly a kite (`fly`), code on a rooftop (`stationary`, city), roast marshmallows (`stationary`, cozy). Add activities incrementally. A missing activity is skipped, never a crash.

### 7.2 Pipeline
1. **Load**: decode the embedded sheets and JSON at startup into frames with durations.
2. **Catalog**: the activity files (§4.7) reference sprite sheets by activity id and state tag. `assets/clawd/catalog.toml` lists the shared base animations. CI checks that every state referenced by an activity exists in its sheet and has the required anchor slices.
3. **Atlas**: pack all frames into one texture atlas, used by both the GPU renderer and the CPU compositor.
4. **Scale**: integer nearest-neighbor only. Pick the largest integer scale that makes Clawd about 6–9% of screen height.
5. **Procedural extras** (code, not art): bob offset, occasional blink overlay, movement along motion paths with sub-cell interpolation, and slight body tilt on slopes. These add variety and smoothness without more frames.
6. **Isolation**: all Clawd-specific code and data live in one module (`sprites::clawd`) behind a small `Character` trait (`animation(activity_id, state) -> &Animation`). If Clawd ever has to be replaced (§15), only this module and `assets/clawd/` change.
7. **Buddy toggle**:
   - **On** (default): every scene places Clawd per §4.2.
   - **Off**: scenes have no character. The focal point is the activity's unattended props and the landscape, and compositions must still look intentional, with no empty slot where Clawd would be.
   - Buddy is a **render flag, not part of the recipe** (§3.3, §4.2). Toggling re-renders the current recipe with or without Clawd: same landscape, same activity, with the unattended variant when off. Library items and timeline slots can override it with `buddy = "follow" | "on" | "off"` (§9a, §9b).

---

## 8. Settings

Stored at `<config_dir>/clairlune/settings.toml` (the directory name comes from `APP_ID`). All settings are editable in the settings UI. The groups below are also the settings sidebar sections (§10a).

| Group | Setting | Default |
|---|---|---|
| Rotation | Mode | Generate (alternative: Timeline, §9b) |
| Rotation | Timeline layout (Timeline mode) | Single timeline (alternative: Weekly) |
| Rotation | Open timeline editor | — |
| Schedule | Change wallpaper every (Generate mode only) | 1 hour (options 15 min … 1 day, or "Only on demand") |
| Schedule | Also change at login | on |
| Theme | Mode | Follow system |
| Theme | Schedule times (if Schedule mode) | dark 19:00–07:00 |
| Theme | Use sunrise/sunset + lat/long (if Schedule mode) | off |
| Display | Resolution | Match screen |
| Display | Aspect ratio | Match screen |
| Display | Per-monitor scenes / span | Per-monitor |
| Display | Wallpaper mode | **Live** where supported, else Still frame. Options: Live (animated) or Still frame (a single PNG set via the OS API; no app window and no per-frame work). |
| Display | Still frame choice (Still mode) | Hero frame (`t_hero`, §4.4) |
| Live | Smoothness | Balanced (Battery saver 6 / Balanced 12 / Smooth 24 / Very smooth 30 / Ultra smooth 60 fps) |
| Live | Drop to Battery saver on battery | on |
| Live | Pause on battery | off |
| Live | Pause when fullscreen app | on |
| Content | Enabled archetypes (checkboxes) | all |
| Content | Enabled weather | all |
| Content | Enabled activities | all |
| Archive | Keep wallpapers for | 7 days (1 day / 7 / 30 / 90 / forever) |
| Archive | Max archive size | 1 GB |
| Archive | Store in archive | Recipe + thumbnail + PNG (GIF optional) |
| Export | Default still format | PNG |
| Export | JPEG quality | 90 |
| Export | GIF size / loop length | 1080p / 6 s |
| Export | Export folder | Pictures/Clairlune |
| Buddy | Show Clawd in wallpapers | on |
| General | Start at login | on |
| General | About (version, licenses, logs, disclaimer) | — |

Changing a setting applies live where possible. Resolution, theme and Buddy changes re-render the current recipe instead of rolling a new scene, except for theme-locked scenes (§1.2). Content filters only affect new scenes.

---

## 9. Archive and retention

```
<data_dir>/archive/
└── 2026-11-18T17-56-03Z_seed-1234567890/   # UTC timestamp, so DST can't collide names
    ├── recipe.json        # always: one recipe per monitor, scene_epoch, content_ctx hash
    ├── thumb.png          # always (480 px wide)
    ├── wallpaper.png      # if enabled
    └── wallpaper.gif      # if enabled
```

- An `index.json` (or a small SQLite via `rusqlite`, if search or filters are wanted later) lists entries with timestamp, seed, theme, archetype and pinned status.
- **Retention sweep** runs at startup, after each archive write, and once a day:
  1. delete unpinned entries older than the retention period
  2. if over the max size, delete the oldest unpinned entries first
- **Automatic retention deletes permanently.** Moving them to the OS Trash would fill the Trash every hour and never free disk. Only **user-initiated** deletes go to the Trash (`trash` crate).
- **Pinned entries are exempt from the size cap.** If pinned entries alone exceed it, show a warning in the archive browser.
- **Re-applying** an archived entry moves it out of the archive and makes it current. When it is later replaced, it is archived again with a new timestamp, with no duplicate.
- Retention never touches the library (§9a). "Pin" keeps an entry in the archive, and "Save to library" copies it to the permanent library. Both are offered in the archive browser.

---

## 9a. Saved library

The archive is temporary history. The **library** is where the user keeps wallpapers on purpose. Nothing in the library is ever auto-deleted.

- **Saving:**
  - "Save to library" from the tray (current wallpaper), from the archive browser, and from the timeline editor's picker.
  - Saving **copies** the entry into the library. The archive entry still expires normally, and the library copy stays.
- **Location:**

  ```
  <data_dir>/library/<item_id>/
  ├── item.json      # id, name, created_at, tags, display, theme, buddy, generator_version
  ├── recipe.json
  ├── thumb.png
  └── wallpaper.png  # full-resolution hero frame rendered at save time (canonical look)
  ```

- **Item fields:**

  | Field | Values | Default | Meaning |
  |---|---|---|---|
  | `name` | text | `<archetype> #<short seed>` | Editable. |
  | `display` | `follow`, `live`, `still` | `follow` | Whether this item shows as a live or still wallpaper. |
  | `theme` | `follow`, `light`, `dark` | `follow` | `follow` renders in the active theme. For theme-locked items, only their theme is offered. |
  | `buddy` | `follow`, `on`, `off` | `follow` | Show Clawd for this item, or follow the global toggle. |
  | `tags` | list of text | empty | Used by timeline pools. |

- **Why the recipe is stored:** library items render at whatever resolution the screen currently has. The saved PNG is the canonical look. If the resolution or aspect ratio changes, the item is re-rendered from the recipe. If `generator_version` changed since saving, the re-render may differ, so the item shows an "approximate" badge and the saved PNG is used for Still mode when its size still fits. GIFs are not stored, because live playback renders from the recipe (§5.3). GIF export works from any library item.
- **Enums**: everywhere in this plan, override fields use exactly `follow` plus explicit values (`display`: `follow` / `live` / `still`; `theme`: `follow` / `light` / `dark`; `buddy`: `follow` / `on` / `off`). Timeline slot overrides use the same values, where `follow` means "follow the item, then the global setting".
- **Library browser:**
  - Grid of thumbnails with name, a live/still badge, and a theme badge.
  - Actions: apply now, rename, edit tags, set display/theme/buddy, export PNG/GIF, delete.
  - Filters: by tag, by theme, and "used in timeline".
- **Deleting an item used by a timeline:** show which days and times use it, then offer:
  - Cancel
  - Replace those slots with "Random"
  - Remove those slots
  - The item is also removed from any pool that lists it by id.

  Never leave a dangling reference.

---

## 9b. Rotation modes and the daily timeline

`Settings → Rotation → Mode` has two values:

| Mode | Behavior |
|---|---|
| **Generate** (default) | A new random scene every N minutes (§8 Schedule). |
| **Timeline** | A daily schedule of wallpapers, repeating every day or per weekday, like a Wallpaper Engine playlist but time-based. |

### Timeline model
- A timeline day is an ordered list of **slots**. Each slot has:
  - **Start time**, in local time, 15-minute snap by default and minute precision allowed. A slot runs until the next slot starts. The last slot wraps around midnight into the first one, so a day always fully covers 24 h.
  - **Content**, one of:
    - **Library item**: one saved wallpaper.
    - **Library pool**: a set of items, or a tag such as "evening", picked from with `order = "shuffle"` (seeded random, no immediate repeat) or `order = "sequential"` (next item each time the slot starts, position persisted in `state.json`).
    - **Random**: generate a fresh scene. Optionally constrained by archetype ids, activity ids, weather or theme, for example "random night scenes" for a 22:00 slot.
  - **Display override**: `follow` / `live` / `still`.
  - **Theme override**: `follow` / `light` / `dark`. This lets a timeline encode light mornings and dark evenings, which pairs naturally with the theme schedule.
  - **Buddy override**: `follow` / `on` / `off`.
- **Layout** (`Settings → Rotation → Timeline layout`):
  - **Single timeline**: one day, used for all 7 days.
  - **Weekly timeline**: a separate timeline per weekday, Monday to Sunday.
  - **Single → Weekly**: copies the single timeline into all 7 days.
  - **Weekly → Single**: asks which day becomes the single timeline (default: today), and warns that the other 6 days will be discarded. Keep a one-step undo of the discarded week.
- **Copy day** (weekly layout only):
  - A "Copy to…" button on each day opens a picker with day checkboxes plus quick picks: All days, Weekdays (Mon–Fri), Weekend.
  - Copying replaces the target days' slots. Confirm when overwriting non-empty days.
  - A "Clear day" button is also included.

### Storage

`<config_dir>/clairlune/timeline.toml`:

```toml
layout = "weekly"            # or "single"

[[day.monday.slot]]
start = "07:00"
content = { type = "item", id = "lib_8f2c" }
display = "follow"
theme = "light"
buddy = "follow"

[[day.monday.slot]]
start = "19:30"
content = { type = "pool", tag = "evening", order = "shuffle" }
display = "live"
theme = "dark"

[[day.monday.slot]]
start = "23:00"
content = { type = "random", archetypes = ["night_sky", "canyon"] }
```

`layout = "single"` uses a `[[single.slot]]` list instead of per-day lists. Validate on load: sorted starts, no duplicate starts, existing item ids, known archetype and activity ids. An empty day falls back to Generate mode for that day, with a warning badge in the editor.

### Runtime behavior
- **Resolving the active slot:** on startup, on wake from sleep, on timezone change, and after any timeline edit, the scheduler computes the slot that is active **right now** (today's day, the last slot with `start <= now`, else yesterday's last slot) and applies it immediately.
- **Resolve by UTC instants, not wall-clock comparisons**: for each day, map every slot's local start time to a UTC instant.
  - A **repeated** local time (fall back) uses its **first** occurrence, so the slot fires once.
  - A **skipped** local time (spring forward) uses the first valid instant after it.
  - When several slots map to the same instant, the latest-defined slot wins.
  - The active slot is the one with the latest instant `<= now_utc`.
- **Waiting for the next slot:** `std` sleeps are monotonic and drift across sleep and clock changes, so don't sleep until a deadline. Re-evaluate every 30 s, and immediately on wake, timezone change and clock change notifications.
- **Testability**: make the scheduler generic over `chrono::TimeZone` and inject the clock, so tests can use `chrono_tz` zones (`chrono::Local` can't be injected). Cover spring-forward, fall-back and a slot inside the repeated hour.
- **Pre-rendering:** render the next slot's wallpaper a few seconds before its boundary (§5.4), so the switch is instant.
- **Archive:**
  - Slots showing a **library item** do not create archive entries, since the item is already saved.
  - **Random** slots archive the generated scene as usual, so a good one can be saved to the library later.
- **Interaction with other settings:**
  - The global "change every N minutes" interval is ignored in Timeline mode.
  - "New wallpaper now" from the tray shows a random scene until the next slot boundary, then the timeline resumes.
  - The tray menu shows "Timeline: <current slot> → next at HH:MM".

### Timeline editor UI
- **Weekly layout:** 7 horizontal rows, one per day, each a 24-hour bar. **Single layout:** one bar.
- Slots appear as blocks with the item thumbnail as the background and the start time label.
- Interactions:
  - Drag a block edge to move the boundary, with 15-minute snapping (hold Shift for minute precision).
  - Click a block to edit its content, display and theme in a side panel.
  - Double-click inside a block to split it at that time, creating a new slot. Delete with a button or the Del key; the previous slot then extends to cover the gap.
  - Drag an item from a library sidebar onto a day bar to insert a slot at that time.
- Each day row has: Copy to…, Clear, and "Preview": scrub a time cursor to see which wallpaper would be active.
- A "Today" marker shows the current time and the active slot.

---

## 10. App shell

- **Tray / menu-bar menu**:
  - New wallpaper now
  - Theme ▸ Light / Dark / Follow system / Schedule
  - Wallpaper mode ▸ Live / Still frame
  - Pause live / Resume
  - Save current ▸ PNG / GIF / To library
  - Rotation ▸ Generate / Timeline (shows "current slot → next at HH:MM" in Timeline mode)
  - Previous wallpaper
  - Library…
  - Timeline…
  - Archive…
  - Settings…
  - Quit
- **UI windows** (Settings, Library, Timeline, Archive): created on demand on the single event loop and dropped on close, so no UI memory is held while idle (§3.4).
- **No tray available**: vanilla GNOME has no StatusNotifier/AppIndicator support without an extension (Ubuntu ships one; Fedora and stock GNOME don't). When no tray host is detected, open Settings on launch, install a `.desktop` launcher, and let a second launch open Settings. Document the AppIndicator extension.
- **Single instance**: a second launch tells the running instance to open Settings, over the local socket (§3.2).
- **No Dock / taskbar icon while idle**: on macOS set `LSUIElement` (Accessory activation policy), switching to Regular while a UI window is open.
- **Start at login**: `auto-launch` with `set_use_launch_agent(true)` on macOS, the Run registry key on Windows, and XDG autostart on Linux.
- **Logs**: rolling file in `<data_dir>/logs/`, viewable from Settings → About.
- **Crash safety**: the static wallpaper is always set first (§2), so a crash never leaves a blank desktop.

---

## 10a. UI/UX design: neomodern, and must not look AI-made

The settings window, library, archive browser and timeline editor are what users judge the app by. Treat design as a deliverable with its own research phase, not as styling added at the end.

### Required research before any UI code
Spend real time on this and write the findings to `docs/design.md`, including a moodboard (links or screenshots) and the decisions taken from each source:
- **Platform guidelines**: Apple Human Interface Guidelines (macOS), Microsoft Fluent 2 / Windows app design, GNOME HIG. Learn native conventions for windows, settings layout, tray/menu-bar behavior, keyboard shortcuts and accessibility. The app must feel at home on each OS while keeping one visual identity.
- **Fundamentals**:
  - *Refactoring UI* (hierarchy, spacing, color, depth)
  - Laws of UX (lawsofux.com)
  - Nielsen Norman Group's 10 usability heuristics
  - Butterick's *Practical Typography*
  - WCAG 2.2 AA contrast and focus rules
- **Reference products** known for craft:
  - Linear, Things 3, Raycast, Arc, Teenage Engineering product UIs, Panic's apps (Nova, Playdate)
  - Swiss / International Typographic Style posters
  - Dieter Rams' "less but better" principles

  For each, note *why* it works, not just how it looks.
- **What "AI-made" UI looks like**, so it can be avoided. Search for current critiques of generic AI-generated interfaces and list the tells.

### The neomodern direction
"Neomodern" here means modernist discipline with contemporary refinement:
- **Grid and alignment**:
  - a strict 4 px base unit and 8 px spacing scale
  - a visible underlying grid
  - left-aligned content
  - generous but intentional whitespace
- **Typography carries the hierarchy**:
  - one grotesk sans for UI (for example Inter Display, Geist, or IBM Plex Sans, under OFL or similar)
  - one monospace for data, times, seeds and labels that echo the ASCII wallpapers
  - a tight type scale of about 5 sizes
  - real weights and tracking, not color, to separate levels
- **Restrained color**:
  - neutral surfaces (paper white / warm black, matching the wallpaper palettes)
  - **one** accent color used only for primary actions, selection and the "now" marker on the timeline
  - full light and dark themes that follow the app theme setting
- **Flat with precise depth**: hairline 1 px borders and at most one subtle elevation level. No heavy drop shadows, glassmorphism or blur.
- **Geometric details**:
  - consistent small corner radii (for example 6 px controls, 10 px panels)
  - a single icon set at one stroke weight (Lucide, ISC license, or Phosphor, MIT)
  - pixel-art accents only where they connect to the wallpapers, for example Clawd in empty states
- **Motion**: short (120–200 ms), purposeful transitions with ease-out curves. No bouncing, and respect "reduce motion".
- **The wallpaper is the hero**: thumbnails and previews get the space. Chrome stays quiet.

### Anti-patterns that are not allowed (common AI-made tells)
- purple/blue gradients, gradient text, glowing buttons
- emoji used as icons or decoration
- everything centered in cards, card-inside-card nesting, identical card grids for every screen
- default framework styling left as-is (stock egui look, default fonts)
- vague filler copy ("Unleash your creativity", "Seamlessly…"), exclamation marks, marketing tone inside settings
- badges, pills and tooltips on everything; 4+ font sizes on one screen; inconsistent spacing or radii
- decorative illustrations or icons that carry no information
- modal dialogs where an inline control would do

### UX rules
- **Settings**: grouped into a sidebar whose sections match the groups in §8 (Rotation, Schedule, Theme, Display, Live, Buddy, Content, Archive, Export, General). Library, Timeline and Archive browsers are their own windows.
  - Every control has a one-line plain-language description.
  - Changes apply instantly with no "Save" button, with an undo toast for destructive changes.
- **Defaults are good enough** that most users never open Settings.
- **Every state is designed**: empty library, empty timeline day, live unavailable on this OS, Buddy off, generation in progress, export progress, errors with a clear next step.
- **Keyboard and accessibility**:
  - full keyboard navigation, visible focus rings
  - screen-reader labels (AccessKit, which egui and Slint support)
  - minimum 4.5:1 text contrast
  - hit targets ≥ 28 px on desktop
- **Timeline editor** gets usability-test attention: dragging, snapping, copying days and switching layouts must work without reading docs.

### Design process
1. Research and moodboard, written to `docs/design.md`.
2. Design tokens defined as one Rust module: colors per theme, spacing, radii, type scale, motion durations. No magic numbers in UI code.
3. Low-fidelity wireframes for every screen and state.
4. High-fidelity mockups of the 4 main screens in both themes, reviewed against the anti-pattern list and the heuristics.
5. Implement. Then do a screenshot review pass per screen, per theme, on each OS, fixing anything that hits the anti-pattern list.
6. Add the review checklist to the PR template, so later changes stay on-standard.

---

## 11. Milestones

Each milestone ends with a working, testable state.

**M0. Clawd style guide and first sprites**
- Write `docs/clawd-style.md` (§7.1).
- Create the art template (`_template.aseprite`: canvas, palette, baseline guide, standard anchor slices) and the export pipeline.
- Draw the base animations (idle, walk) and the first activity, with all its states, from scratch. Pick one that tests anchors and effects well, such as fishing or skiing.

**M1. Headless generator + CLI**
- `scene` + `render` crates: canyon archetype, glyph atlas, palette-index compositing (§4.5), static bake, indexed PNG export, with the prototype's pixel Clawd and campfire as placeholders.
- `clairlune-cli generate --seed 7 --theme dark --size 3840x2160 --out out.png`
- Golden tests at 640×360 for 10 fixed seeds × 2 themes. A determinism test: same inputs → byte-identical output, in CI on all 3 OSes (requires `libm`, integer sampling and per-subsystem RNG streams from day one).
- Performance test: 4K bake < 150 ms (release build).

**M2. Sprite pipeline + activity engine**
- Load our sheets with per-frame anchors, and build the atlas.
- Activity files, validation, activity-first composition, the state machine with episodes, and near-duplicate avoidance with `state.json`.
- `stationary` and `pace` motion types, and props and effects attached to anchors.
- Buddy toggle as a render flag, and unattended variants. Theme-independent recipes with the light/dark time-of-day pair.
- CLI: `--no-buddy`, `--activity <id>`, `preview-activity` with hot reload.

**M3. Animation + GIF export**
- Live state-machine playback vs GIF loop scripts, particles, fire flicker, and the Smoothness levels with motion interpolation.
- GIF export with a global palette, one sub-image per frame, delay dithering and native-size rendering. Record GIF size and encode time in tests.

**M4. More archetypes and activities**
- Add the remaining archetypes, platforms, props and weather, plus compatibility tables and constraint checks.
- `path`, `bob` and `fly` motion types, with archetypes exposing slopes, water lines and anchors.
- Activities: the v1 set from §7.1, then the starter list in §4.7. This is ongoing art work after v1.
- A "contact sheet" CLI command renders 48 random seeds into one grid image for quick visual QA.

**M5. Static wallpaper setting on all OSes**
- `platform` crate static hosts: Windows, macOS (per Space re-apply), GNOME (light + dark URIs), KDE, XFCE, Cinnamon, MATE, and wlroots (our own layer-shell still surface).
- Unique file names per set. Monitor enumeration with stable ids, physical-pixel Match-screen sizing, hotplug handling (§2.1).

**M6. App shell** (starts with the §10a design research, tokens, mockups, and the egui-vs-Slint spike)
- Single event loop (§3.4), tray (with the no-tray fallback), settings UI, settings persistence, scheduler, theme modes, archive + retention, archive browser, exports, start at login.
- At this point the app is fully usable in static-only mode.

**M6b. Library + Timeline**
- Saved library (§9a) and Timeline mode (§9b):
  - single/weekly layouts
  - copy day to other days
  - layout switching with undo
  - slot resolution on startup/wake
  - DST tests
  - the timeline editor UI
- Library deletion safety for items referenced by the timeline.

**M7. Live wallpaper hosts**
- Start with a spike: Windows WorkerW (classic and 24H2 layouts), macOS desktop-level windows, layer-shell on Sway and on KDE Plasma (icon stacking), X11 desktop window with and without an icon manager.
- `wgpu` renderer with palette lookup, pause rules, CPU `softbuffer` fallback, GPU-vs-CPU golden test.
- Measure CPU, GPU and RAM on each OS, then fix the budget numbers in §5.3.

**M8. Polish and packaging**
- Multi-monitor span, sunrise/sunset schedule.
- Installers: `cargo-packager` (it covers all the formats below; `cargo-dist` doesn't build `.app`/DMG, AppImage or `.deb`). MSI on Windows, `.app` in a DMG on macOS, AppImage and `.deb` on Linux.
- **Notarize** the macOS build for any public release. Since macOS 15, unnotarized downloads can't be opened with Control-click → Open.
- Release builds include only the project-drawn sprites in `assets/clawd/` (§1.6). CI fails if any exported sheet lacks a matching source file in `assets/clawd/src/`.

---

## 12. Testing strategy
- **Determinism**: `(seed, aspect, content_ctx)` → identical recipe JSON, and `(recipe, size, theme, buddy, t)` → identical pixel hash, across runs and OSes. Bundled fonts, ChaCha bits, integer sampling and `libm` make this achievable. Pin the `fontdue` version. Run in CI on all 3 OSes.
- **Golden images**: small-resolution snapshots, compared with a tolerance. Regenerate them intentionally when bumping `generator_version`.
- **Property tests** (`proptest`) over random seeds:
  - the recipe always passes constraints
  - Clawd's feet are on the platform or the activity's motion path
  - there are ≤ 255 colors, and every pixel is a palette entry
  - the GIF frame delays sum exactly to the loop length
  - toggling Buddy or theme (for non-locked scenes) keeps the landscape identical
- **Coverage test**: every archetype and both themes can produce at least one scene with the shipped activities, and the no-eligible-activity fallback works.
- **Variety test**: generate 1,000 seeds per theme with default weights. No single activity may exceed 2× its expected share, and every activity and archetype must appear at least once. This catches an accidental bias toward one scene, such as campfires.
- **Benchmarks** (`criterion`): bake at 1080p and 4K, GIF encode, per-frame live cost.
- **Manual QA matrix**:
  - Windows 10
  - Windows 11 before 24H2, and 24H2 or later
  - a mixed-DPI multi-monitor setup
  - macOS (latest + one older), with several Spaces
  - Ubuntu GNOME (static fallback) and Fedora GNOME (no tray)
  - KDE Plasma Wayland
  - Sway or Hyprland
  - an X11 DE with a desktop-icon manager (XFCE)

---

## 13. Reference: existing Python prototype

A working single-file prototype of the canyon archetype exists (Python + Pillow + numpy): `generate.py`, in the same folder as this plan. **Copy it into the repo as `reference/prototype_generate.py`.** Its 4K output for seed 7, `wallpaper-7.png` (also in this folder), is the visual reference for the dark canyon scene. Copy it to `reference/prototype_seed7_dark.png`:

![Prototype output: dark canyon, Clawd and campfire, seed 7](wallpaper-7.png)

Treat it as a baseline to beat, not a target to match pixel for pixel. Known weaknesses: the log still blends into the ledges, and the scene has one activity and no animation.

It validated these choices:
- 11×20 px cells at 4K with an 18 px Menlo font
- the three cliff layers with the hatch / dense glyph families listed in §4.3
- density thresholds of about 0.28–0.34 for holes
- dimming the cliff ledges to 50% below the platform line, so the log reads as its own object
- clearing cells around Clawd and the fire
- a warm radial glow with a radius of about 900 px at 4K, plus ember tinting of glyphs within about 650 px
- an upward-drifting ember plume that widens with height
- a radial **vignette** that brightens the center by up to 55% (it must be quantized into background bands, §4.5)

Port its logic as the canyon archetype in M1. Key values for the dark palette:
- background `#151412`
- ink `#E9E5D7`
- ember `#FF9646`
- Clawd body `#D9785C`
- Clawd eyes `#261A16`

Pixel maps from the prototype, as starting points only (`X` body, `E` eyes; fire `O` orange `#E86028`, `Y` yellow `#FFAA3C`, `W` white-hot `#FFECAA`, `B` log `#784E30`, `K` dark log `#462E1E`):

```
Clawd (15x9)        Campfire (9x11)
..XXXXXXXXXXX..     ....Y....
..XXXXXXXXXXX..     ...OY....
..XXEXXXXXEXX..     ..OOYO...
XXXXEXXXXXEXXXX     ..OYYO...
XXXXXXXXXXXXXXX     .OOYYYO..
..XXXXXXXXXXX..     .OYYWYYO.
..XXXXXXXXXXX..     OOYYWWYOO
..X.X.....X.X..     OYYWWWYYO
..X.X.....X.X..     .OYYYYYO.
                    BBBBBBBBB
                    KKKKKKKKK
```

---

## 14. Open questions to confirm with the user
Settled decisions are recorded in their sections: Clawd is drawn by this project behind a Buddy toggle (§1.6, §7), per-monitor scenes are the default (§2.1), and the archive stores recipe + thumbnail + PNG with GIF opt-in (§8).

Still open:
1. Minimum OS versions. Suggested: Windows 10 22H2+ (`IDesktopWallpaper` per-monitor support exists since Windows 8, so it is not the constraint), macOS 13+, and Linux distros from 2023 onward.

---

## 15. Legal and publishing (open source release)

This is a risk review, not legal advice.

**Decisions (user, 2026-11-18):**
- Clawd stays in the app.
- The Buddy toggle shows or hides him.
- **All Clawd sprites are our own drawings.** No third-party sprite files are used.

### 15.1 Findings

| Item | Status | Remaining risk with the rules in §15.2 |
|---|---|---|
| **Third-party Clawd GIFs (davecurry.net zip, Claude FM captures)** | Not used. | **None.** That risk is removed. |
| **Our own Clawd sprites** | Original drawings, but of Anthropic's character. Copyright in a character covers new drawings of it, so this is fan art. | **Medium.** Lower than reusing someone's files, but not zero. The same position as the popular fan projects in §15.3. |
| **"Clawd" / "Claude" / "Anthropic" in the app name, icon or listing** | Anthropic's trademark guidelines require prior approval for its names, logos and brand elements, and forbid implying sponsorship or affiliation. In January 2026, the open-source project "Clawdbot" was asked by Anthropic to rename, and became "Moltbot". | **Avoided.** The chosen name, Clairlune, uses none of these words or sound-alikes. The app icon must not be Clawd. |
| **ASCII landscape style inspired by Claude FM** | A visual style or technique is not protectable. Our scenes are generated by our own code. | **Low**, as long as we don't copy specific Claude FM frames or layouts pixel for pixel, its track-name overlay, or its branding. |
| **Palette colors** | Colors alone are generally not protected. | **Low.** |
| **Generated wallpapers** | Contain Clawd when the Buddy toggle is on. Users who publish or sell them take on the same fan-art risk. | Say so in the README. With Buddy off, wallpapers contain only our own content. |
| **Fonts** | The prototype used **Menlo**, which is Apple-proprietary and **cannot be bundled**. | Bundle only OFL fonts (JetBrains Mono, Iosevka, IBM Plex Mono) and include their `OFL.txt`. |
| **Rust dependencies** | Mostly MIT / Apache-2.0. Some may carry other licenses (MPL, GPL, Unicode). | Run **`cargo-deny`** in CI with an allowlist, and generate a third-party notices file (`cargo-about`). |
| **GIF format** | The LZW patents expired worldwide by 2004. | None. |
| **Windows WorkerW technique** | Undocumented Windows behavior, used by Wallpaper Engine and Lively. This is a stability risk, not a legal one. | None legally. Handle Explorer restarts. |

### 15.2 Rules for the public release
1. **Name: Clairlune, with an original icon.** No "Claude", "Clawd", "Anthropic" or sound-alikes in the icon, bundle id, crate names or domain. Check "Clairlune" in USPTO / EUIPO before the first release, and reserve the crate names `clairlune` and `clairlune-cli` early.
2. **Only our own art.** Never commit, bundle, download or import third-party Clawd assets. Contributors must confirm in the PR template that sprite contributions are drawn from scratch.
3. **Disclaimer.** README and About: "Unofficial fan project. Not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character." This lowers the confusion risk, but does not fix a name or icon that uses their marks.
4. **License scope.** Code is `MIT OR Apache-2.0`. Our Clawd sprites are fan art of a character we don't own, so **don't put them under an open license** such as CC BY or CC0, which would grant rights we don't have. Mark `assets/clawd/` as "fan art, all rights in the character belong to Anthropic", in a `LICENSE-ART` or `NOTICE` file. Other original art (props, icons, UI) can be CC BY 4.0.
5. **Takedown readiness.** If Anthropic asks, comply quickly. The Buddy toggle and the module isolation in §7.2 mean a compliant release is mostly "remove `assets/clawd/` and default Buddy to off", or swap in an original character later. Document the steps in `docs/takedown.md`.
6. **Ask before promoting widely.** Before a public launch (Product Hunt, Hacker News, a store listing), consider emailing Anthropic (`marketing@anthropic.com`, per their trademark guidelines) for permission. A yes removes most of the remaining risk.
7. **Fonts and dependencies** as in the table: OFL fonts with license files, `cargo-deny` + `cargo-about`.

### 15.3 Existing fan projects (precedent, not permission)
Several public repos use Clawd, as of 2026-11:
- clawd-on-desk, about 6.8k stars, AGPL-3.0, says it is unofficial
- clawd-mochi, about 790 stars, MIT, notes that "Claude" and "Clawd" are Anthropic trademarks
- smaller desktop-pet repos

No takedowns of these were found. This suggests Anthropic currently tolerates fan use of the character. Tolerance can change, and the Clawdbot rename shows Anthropic enforces its **names** once a project gets popular. Follow the same pattern these projects use: unofficial label, trademark notice, no Anthropic names in the product name.
