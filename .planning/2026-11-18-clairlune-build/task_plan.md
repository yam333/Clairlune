# Task Plan: Build Clairlune (procedural live ASCII wallpaper app, Rust)

## Goal
Implement PLAN.md milestone by milestone. This session delivers M1 (headless deterministic generator + `clairlune-cli generate` that writes an indexed PNG), with tests and CI, and prepares the crates for crates.io. The user granted full access (no more approval stops), so publishing is part of this session.

## Next Step
Roadmap (2026-11-20, user: "Do all" after 0.9.0; the user's feedback from trying it comes first). Order: what can be tested here first, then content, then other OSes (compiled and unit-tested, untried), then packaging.
0. Phase 25: the user's feedback from trying the app (smoothness pace bug, weight, pause option, Buddy page, light theme, richer scenes, installers, Schedule/Timeline overlap, macOS wallpapers)
1. Phase 24: app gaps (24a-24e done): preview-activity dev window left
2. Phase 16 leftovers: smoother motion (sub-cell particles, interpolation), CPU fallback (16f)
3. Phase 12 content: motions path/bob/fly, Waves B-F, J
4. Phase 13/16h/14f: Linux live hosts, macOS static + live hosts, wlroots still surface
5. Phase 17 (M8): span mode, sunrise/sunset schedule, installers (cargo-packager), notarization setup, sprite-source CI check
Needs the user: a Mac, a Linux desktop, an Apple developer account (notarization).

## Current Phase
Phase 24

## Scope of this session
Phases 1-8 (through M1 and publish prep). Phases 9-17 are later milestones, listed so the whole roadmap is tracked in one place.

## Phases

### Phase 1: Requirements & Discovery
- [x] Read PLAN.md in full (1021 lines, sections 1-15)
- [x] Check toolchain: cargo/rustc 1.98.1 (stable, MSVC), git 2.47, Python 3.14
- [x] Check the folder: only PLAN.md. The prototype `generate.py` and `wallpaper-7.png` named in section 13 are NOT present
- [x] Record constraints and decisions in findings.md
- **Status:** complete

### Phase 2: Workspace scaffold & repo hygiene
- [x] Root `Cargo.toml` workspace (edition 2024, shared `[workspace.package]` metadata, pinned deps)
- [x] Crates: `crates/scene` (clairlune-scene), `crates/render` (clairlune-render), `crates/cli` (clairlune-cli), `crates/app` (clairlune, stub binary until M6)
- [x] `APP_NAME` / `APP_ID` constants in one place (clairlune-scene `ident` module)
- [x] LICENSE-MIT, LICENSE-APACHE, NOTICE (fan-art notice for Clawd, section 15.2.4), README with the disclaimer (section 15.2.3), docs/takedown.md stub, .gitignore
- [x] Bundle JetBrains Mono (OFL) + OFL.txt inside `crates/render/assets/fonts/`
- [x] `cargo build --workspace` passes (no warnings)
- **Status:** complete

### Phase 3: M1 scene crate (pure, deterministic)
- [x] `hash.rs`: stable FNV-1a + splitmix64 hashing (never std `DefaultHasher`)
- [x] `rng.rs`: ChaCha8 bit source seeded from our own 32-byte expansion; hand-written integer sampling (rejection `below`, ranges, integer-weight picks, 53-bit unit floats). Per-subsystem streams `hash(seed, "terrain")` etc.
- [x] `math.rs` + `noise.rs`: libm-only transcendental math, hand-written value noise 1D/2D, fbm, ridged
- [x] `recipe.rs`: `SceneRecipe` (generator_version, seed, aspect, content_ctx hash, archetype, light_tod/dark_tod, platform, props, weather, composition, palette per theme) + `ContentContext`
- [x] `compose(seed, aspect, ctx) -> SceneRecipe` with weighted tables (canyon only in M1)
- [x] `build(recipe, size, theme, buddy) -> SceneGraph`: grid sizing (about 108 rows, min cell 6x11), canyon layers (far/mid/near/platform), holes, depth shading, top-edge fade, 50% dim below platform line, focal clearing, sky dust/stars, ember emitter, glow + vignette params, `t_hero`
- [x] Placeholder pixel maps (prototype Clawd 15x9, campfire 9x11, plus a log seat as the Buddy-off variant), clearly marked as placeholders
- **Status:** complete

### Phase 4: M1 render crate
- [x] Glyph atlas: ttf-parser outlines + our own supersampled rasterizer (f64 basic ops only, so it is byte-identical on every OS)
- [x] Indexed palette (<= 255 entries, index 255 reserved): 8 background bands, ink (band x 6 levels, warm tint folded into glow bands), sprite colours, ember colours (band x level). 111 colours in practice.
- [x] Static bake: band per pixel (vignette + glow, ordered 4x4 Bayer dither, polynomial falloff so no per-pixel pow), glyph pass by palette index
- [x] Frame composite at time t: sprites (integer nearest-neighbour scale), particles snapped to cells
- [x] Indexed PNG writer (`png` crate), RGBA conversion helper
- **Status:** complete

### Phase 5: M1 CLI
- [x] `clairlune-cli generate --seed 7 --theme dark --size 3840x2160 --out out.png`
- [x] Extras: `--no-buddy`, `--recipe-out <json>`, `--recipe <json>` (re-render), `--time-ms <t>` (default `t_hero`)
- [x] Prints the seed, size, palette size and timings
- **Status:** complete

### Phase 6: Visual review & tuning
- [x] Render 4K dark/light and several seeds; inspect the PNGs
- [x] Tune glyph densities, thresholds, glow/vignette, platform readability (fix the prototype's "log blends into ledges" weakness)
- [x] Record what changed and why in findings.md
- **Status:** complete

### Phase 7: M1 tests & CI
- [x] Determinism: same `(seed, aspect, ctx)` gives byte-identical recipe JSON; same render inputs give byte-identical pixels
- [x] Golden images: 10 seeds x 2 themes at 640x360 in `tests/golden/`, plus a hash manifest; bless with `CLAIRLUNE_BLESS=1`
- [x] Property tests: palette <= 255 and every pixel is a palette entry; Buddy toggle keeps the landscape identical; Clawd's feet on the platform
- [x] Perf test: 4K `bake_static` < 150 ms in release (ignored by default, run with `--release -- --ignored`)
- [x] `.github/workflows/ci.yml` (Windows/macOS/Linux: fmt, clippy, tests, golden hashes) + `deny.toml` (cargo-deny passes locally: advisories, bans, licenses, sources)
- [x] `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` pass locally
- **Status:** complete

### Phase 8: crates.io publish prep (M1 exit)
- [x] Metadata on every published crate: description, license, repository (github.com/yam333/clairlune), readme, keywords, categories; per-crate README, LICENSE-MIT, LICENSE-APACHE, NOTICE
- [x] `cargo publish --workspace --dry-run` passes (order: clairlune-scene, clairlune, clairlune-render, clairlune-cli)
- [x] User authorised publishing ("you have complete access")
- [x] `git init`, first commit, push to the private repo (cargo records the commit in each package)
- [x] `cargo publish --workspace`: clairlune-scene, clairlune, clairlune-render, clairlune-cli 0.1.0 are live
- [x] End-to-end: `cargo install clairlune-cli` from crates.io renders a PNG byte-identical to the golden `seed-7-dark.png`
- **Status:** complete

### Phase 9: M0 Clawd style guide & sprite pipeline (needs the user + Aseprite)
- [x] `docs/clawd-style.md`: canvas, `origin` slice, proportions, 16-colour palette, no partial alpha, engine-drawn light-theme outline, tags = state ids, anchor slices, layers
- [x] `scripts/export_sprites.py` (+ `justfile` `export-sprites`): Aseprite CLI export into `crates/sprites/assets/clawd/`, records `meta.source_sha256`
- [x] `scripts/check_sprites.py` in CI: stale exports, exports without sources, sources without exports
- [x] Aseprite built from source; `_template`, `_base` (idle, walk, celebrate, alert) and `warm_hands` (warm, look_around) drawn as code and exported (Phase 20)
- **Status:** complete

### Phase 10: M2 Sprite pipeline + activity engine
Art-independent core first (10a-10e), then the sheet loader once art exists (10f).
- [x] 10a `scene::archetype`: capability declarations (terrain, surface, platforms, prop slots) for canyon
- [x] 10b `scene::activity`: activity TOML model (PLAN.md 4.7), embedded catalog (`crates/scene/activities/*.toml`), validation (known capabilities, states, transitions, loop script, effects, props); CI-tested
- [x] 10c Activity-first `compose`: eligibility filter, weighted pick, recent-history down-weighting (no repeat unless only one eligible), time of day and weather from the activity; theme-locked recipes; archetype-first fallback; GENERATOR_VERSION 2
- [x] 10d `scene::anim`: seeded state machine with 60 s episodes, dwell times, normalised `next` weights; pure `state_at(seed, t)`
- [x] 10e Motion `stationary` + `pace`; the character becomes a time-dependent track in the SceneGraph; activities: warm hands at the campfire (stationary), pace the ledge (pace), stargaze (stationary, night only = theme-locked); unattended props per activity
- [x] 10f `clairlune-sprites` crate: Aseprite json-array loader, `Character` trait, anchors from slices, palette/alpha validation; placeholder character behind the same trait
- [x] 10g CLI `--activity <id>`; variety test (no activity above 2x its share)
- [x] 10h Render through `Character`: draw frames by state at time t (state machine + sheet frames), anchor-attached effects, sheet colours in the palette, engine-drawn light-theme outline. Lands with M3 animation, since today every state shows the single placeholder frame
- [ ] Deferred: `preview-activity` window with hot reload (needs the winit loop from M6/M7)
- **Status:** in_progress

### Phase 11: M3 Animation + GIF export
- [x] Live state machine vs GIF loop scripts, particles, fire flicker, Smoothness levels (motion interpolation: Phase 16 leftovers)
- [x] Activity Wave A (docs/activity-system.md): frames by state (10h), anchor `particles` effects, held items, `light`; activities: wizard magic, making coffee (camp stove), reading by a lantern, guitar
  - [x] A1 Engine: `[[effects]]` in activity TOML (type particles, preset sparkles|steam, at = anchor, states), validated; seeded periodic particle presets in scene; render draws them at the current frame's anchor (mirrored), ink or ember colour
  - [x] A2 Engine: `[character] headroom` so tall poses (hat, raised wand) get cleared space; light props spellbook and camp_stove
  - [x] A3 Content: props drawn as code (spellbook, camp stove, mug, fallen hat); wizard_magic and make_coffee designs + TOML; export, goldens, visual check
  - [x] A4 Content: reading by a lantern, guitar (with a `notes` particle preset)
- [x] GIF: global palette, one sub-image per frame, delay dithering, native-size render; record sizes in tests (960x540 6 s: ~95-105 KB)
  - [x] 11a Loop length: multiples of the 6 s particle period (6 s, 12 s); 3 s and 10 s from PLAN.md 6.4 can't be seamless with 1-3 s particle lives
  - [x] 11b Character on the GIF clock: the activity's `gif.loop_script`, first looping state repeated, then time-scaled to fill the loop exactly; pace period snapped to divide the loop
  - [x] 11c Smoothness levels with centisecond delays (16/8/4/3/2 cs) and delay dithering so delays sum to the loop exactly
  - [x] 11d Encoder: global palette (index 255 transparent), frame 0 full, then one sub-image per frame (changed bounding box, unchanged pixels transparent), DisposalMethod::Keep, infinite repeat; identical frames merge into the previous delay
  - [x] 11e CLI `gif` subcommand; tests: delays sum, decode-and-recompose equals rendered frames, seam (t=0 equals t=loop), sizes recorded
- **Status:** in_progress

### Phase 12: M4 More archetypes & activities
- [x] Remaining 8 archetypes, platforms, props, weather, compatibility tables, constraint checks
  - [x] 12a Refactor `build` into `archetypes/` modules: shared pipeline (grid, focal, dome, sky, sprites, light, emitters) + per-archetype terrain and platform painters; canyon output byte-identical (goldens unchanged)
  - [x] 12b Weather particles (snow, rain, fireflies, leaves, dust): screen-space, seeded, periodic in the loop; archetype + activity + user filters already decide which
  - [x] 12c mountains   - [x] 12d snowfield   - [x] 12e forest   - [x] 12f desert
    (shared `terrain/layers.rs`: silhouette layers with sub-row outlines `_ - .` for gentle
    curves, `/ \ ^ |` for steep ones, slope-following hatch, snow caps, mesa plateaus,
    occluding holes, text stamps such as pines; ledge styles ICE and DUNE)
  - [x] 12g shore (sea with perspective, islands, pier)   - [x] 12h city_rooftops (skyline layers, windows lit by time of day, rooftop)
  - [x] 12i night_sky (moon phases, aurora curtains, milky way, islets, cloud sea, floating island)   - [x] 12j cave (arch, stalactites, crystals, no sky)
  - [x] `requires.sky` / `ArchetypeInfo.sky`: stargazing never lands in a cave
  - [x] 12k Coverage (tests/coverage.rs, all 9 landscapes), one golden per landscape (`land-<id>`), contact-sheet review 
- [x] contact-sheet CLI command
- [ ] `path`, `bob`, `fly` motion
- [ ] Activity Wave B: set pieces + `prop_anim` + `sit` + `ground_mark` (chess, easel painting, picnic, fishing)
- [ ] Activity Wave C: `path` on slopes/trails + `trail` (skiing, sledding, hiking, sandboarding)
- [ ] Activity Wave D: `projectile` (tennis, skipping stones, snowballs)
- [ ] Activity Wave E: `fly`/`bob` (broom flight, kite, balloon, rowing)
- [ ] Activity Wave F: `interior` archetype with a window view (coffee counter, chess indoors, coding, wizard's study)
- [ ] Wave J, journeys (docs/scrolling-scenes.md): periodic noise (done: noise::*_periodic, exact-repeat tests), periodic layer builder for `mountains`, `journey` motion with feet on the near layer, integer-pixel layer offsets with whole periods per loop, CPU composition; first journey: hiking a ridge. GPU layer grids with wrap addressing in M7
- **Status:** pending

### Phase 13: M5 Static wallpaper on all OSes
- [x] 13a `clairlune-platform` crate: `WallpaperHost` trait (monitors, current, set_static, snapshot, restore), `StillStore` (unique names, prune, first-takeover snapshot), `local_data_dir` without `dirs` (MPL dependency)
- [x] 13b Windows host: `IDesktopWallpaper` per monitor, device-path ids, physical rects under Per-Monitor DPI v2, Fill position; CLI `monitors`, `set`, `restore`; verified on a 2-monitor desktop (set, check, restore)
- [ ] 13c macOS host   - [x] 13d GNOME / KDE / XFCE / Cinnamon / MATE / Sway hosts (14e; unit-tested, untried)   - [ ] 13e wlroots still surface
- [x] 13f hotplug / resolution / DPI change handling (24a)
- [x] Unique file names, monitor ids, physical-pixel sizing
- **Status:** pending

### Phase 14: M6 App shell
Order: the app core first (usable in still mode with no UI), then the design work, then the UI.
- [x] 14a App core (crates/app), Windows first (verified live: set on 2 monitors, archive, logs, second launch refused, restore):
  - settings.toml model (serde, defaults from PLAN.md 8, unknown keys tolerated) in `<config_dir>/clairlune/`
  - one winit 0.30 event loop; tray-icon + muda menu (New wallpaper now, Theme, Previous, Save current PNG,
    Open archive folder, Pause schedule, Settings (interim: open settings.toml), Quit); tray icon is a pixel moon,
    not Clawd (the app's own mark must not borrow Anthropic's character)
  - scheduler (change every N minutes, change at login) with ControlFlow::WaitUntil; generation on a worker
    thread, results back as user events
  - apply = per-monitor compose + render + StillStore + set_static; theme Follow system via dark-light
  - archive entry per apply (recipes per monitor, thumb, PNG) + index.json + retention sweep (age, size cap)
  - previous wallpaper from the archive; export current PNG to Pictures/Clairlune
  - rolling log file in `<data_dir>/logs/`, panic hook; single instance via an OS file lock (opening Settings from a second launch waits for the IPC in 14d)
- [x] 14b docs/design.md (sources and why each works, AI-made tells, decisions, wireframes, states, review checklist); crates/app/src/tokens.rs with WCAG contrast tests. Fonts: IBM Plex Sans (UI) + JetBrains Mono (data); one accent, the ember orange
- [x] 14c Toolkit: egui (Slint's licences aren't permissive). `clairlune --ui-test` passes: Settings opened and closed 3x on one loop beside the tray, restyled from the tokens in both themes (docs/design.md section 8)
- [x] 14d Settings window: all sections (Schedule, Theme, Display, Live, Buddy, Content, Archive, Export, General); moon empty state; inline notes for results and errors; start at login via the per-user Run key (Windows; the OS is the source of truth); Restore my wallpaper (pauses the schedule); title follows the section; reopens on the last section
- [x] 14e Frozen scene package (below), second launch opens Settings (IPC), Linux tray (ksni), Linux static hosts (GNOME/Cinnamon/MATE/KDE/XFCE/Sway, unit-tested, not tried on hardware), package CLI and GIF export
- [ ] 14f macOS static host (NSWorkspace per screen), screenshot review on macOS/Linux (needs those machines)
- [x] Frozen scene package (PLAN.md 9c): render::package (format 1: package.json + background-<theme>.png + sprites.png; 0.4-1.1 MB for both themes at 4K). Art comes through render::art::SceneArt, so playback never touches the generator, catalog or built-in art. Tests: capture == live, save/load, ignores the recipe, newer formats refused, committed golden package. The archive stores one per monitor; Previous and theme changes replay them when the monitor matches
- **Status:** pending

### Phase 15: M6b Library + Timeline (complete)
- [x] 15a Library (PLAN.md 9a): `library.rs` (items copy an archive entry incl. frozen scenes; item.json with name, tags, display/theme/buddy choices), Save to library (tray, Settings), Library window (grid, selection action bar: use as wallpaper, rename, theme/Clawd choices, PNG/GIF export, two-step delete), applying an item replays its scenes with its choices
- [x] 15b Timeline model + timeline.toml (the plan's format) + resolver by UTC instants, generic over chrono::TimeZone; tests in America/New_York: spring-forward gap, repeated hour fires once, same-instant ties, midnight wrap, empty days, weekly layout
- [x] 15c Timeline runtime: Rotation mode (Generate/Timeline), 20 s re-evaluation, item/pool (shuffle with no repeat, sequential with positions in state.json)/random slots, slot > item > global choices, manual New wallpaper holds until the next slot, tooltip shows the next slot; deleting a library item the timeline uses asks to remove those slots or use random there
- [x] 15d Timeline editor window: a 24-hour bar per day (single or weekly), slots show their wallpaper, now marker; drag edges (15 min snap, Shift: minutes), double-click to split, drag saved wallpapers or Random from the sidebar, Add/Copy to.../Clear per day, weekly to single asks which day, 50-step Undo, hover preview; inspector for start (typed or Earlier/Later), saved/pool/random content, theme, Clawd, display; keyboard: arrows select, Delete, Ctrl+Z. Headless egui interaction tests (drag, split, drop, keys) run on CI; UI test screenshots in both themes
- **Status:** pending

### Phase 16: M7 Live wallpaper hosts
Design (2026-11-20): the CPU compositor stays the single source of pixels. A live compositor keeps one index frame and redraws only the rects that changed (last frame's and this frame's particles and sprites), so a frame costs a few small rects, not a 4K copy. The GPU presenter holds the frame as an R8 index texture plus a 256-entry palette and draws one full-screen triangle with an exact lookup (no filtering, no blending, non-sRGB surface), uploading only the changed rects. So live pixels equal the PNG by construction, and the softbuffer fallback can reuse the same rects.
- [x] 16a Incremental compositor in render: IncrementalFrame (restore last frame's rects, redraw, report changed rects), ScenePackage::incremental/advance; tests/live.rs: byte-identical to full frames for every landscape, effects and weather, both clocks, buddy on/off, a pause jump; changed rects cover every changed pixel (mutation-checked)
- [x] 16b Windows desktop-layer host: platform::windows_live (DesktopLayer for both layouts, LiveWindow per monitor placed under the icons, is_alive/is_attached for Explorer restarts, still re-set on exit), exe manifest with supportedOS; `clairlune-cli live` plays per-monitor scenes and prints the cost. Tried here on build 26300 (raised layout): only DirectX presents show (findings). Re-attach on TaskbarCreated: the app re-starts the host when `healthy()` turns false (16d)
- [x] 16c wgpu presenter (live::gpu): R8Uint index texture + palette uniform, exact lookup into an 8-bit unorm target, a frame's changed rects packed into one upload; DX12 on the adapter that drives the monitors; Mailbox/Immediate (DWM composes, no tearing); starts at the hero time; readback test equals the CPU frame byte for byte (skips without an adapter). Smoothness rates are the CLI's --fps for now (setting in 16d)
- [x] 16d App Live mode: settings [display] mode (live by default where supported) and [live] smoothness_fps; Settings Display (Still frame/Live, status line) and Live (Smoothness); tray "Live wallpaper"; Done::Applied carries each monitor's frozen scene, so every apply (new, Previous, theme, Clawd, library, timeline) plays live; at launch the scenes on screen play without a new one; Explorer restarts start it again; app exe manifest. Tried here: 147 MB steady live, animating. Found and fixed: Settings opened at every launch since 0.7.0 (the no-tray check ran before the tray existed), which also cost 356 MB; Still mode is 42 MB now
- [x] 16e Pause rules (Windows): covered monitors pause; everything pauses while the session is locked (input desktop can't be opened) or the displays are off (GUID_CONSOLE_DISPLAY_STATE to the live windows); on battery, Battery saver 6 fps by default or an optional pause. Settings Live has both battery toggles. Lock and display-off weren't triggered here (would lock the user's session)
- [ ] 16f softbuffer fallback when wgpu can't start (dirty rects, 8 fps)
- [x] 16g Budgets measured (findings): 2560x1440 + 1920x1080 at 12 fps: 0.64 ms per frame, ~1 % of one core, 139 MB peak RSS. A 4K monitor isn't available here to measure
- [ ] 16h Linux (X11 desktop window, wlr layer-shell) and macOS (desktop-level NSWindow) hosts: compile-checked, unit-tested where possible, marked untried
- **Status:** in progress

### Phase 17: M8 Polish & packaging
- [ ] Span mode, sunrise/sunset, cargo-packager installers, notarization, art-source CI check
- **Status:** pending

### Phase 18: Release 0.2.0 so crates.io matches main
- [x] Bump workspace to 0.2.0 (internal dep versions too), add `--activity` to the CLI README
- [x] Tests + dry run, `cargo publish --workspace` (adds clairlune-sprites): all 5 crates 0.2.0 live
- [x] Verify `cargo install clairlune-cli` then `--activity stroll` works from crates.io
- **Status:** complete

### Phase 19: Trademark check and minimum OS versions (PLAN.md 14, 15.2.1)
- [x] Search "Clairlune" in USPTO and EUIPO (and similar marks); recorded in docs/trademark-check.md
- [x] Decide minimum OS versions with reasons; recorded in docs/platform-support.md and PLAN.md section 14
- **Status:** complete

### Phase 20: Scripted Aseprite art (user installs Aseprite; Claude drives it)
- [x] Research: Lua API has no per-frame slice keys (Slice.bounds only), so anchors move to an `anchors` marker layer; MCP servers exist but only wrap the CLI, so drive the CLI directly
- [x] Build Aseprite v1.3.18.6 from source (user's choice; the EULA allows personal builds, no redistribution). Source zip + prebuilt Skia m124 (`m124-08a5439a6b`, Skia-Windows-Release-x64) unpacked in `C:/Users/yamzu/Developer/tools/`. VsDevCmd x64 with the v143 toolset (14.44), Windows SDK 10.0.26100, VS-bundled CMake + Ninja (not the MinGW ones in C:/gcc-14.2.0)
- [x] Smoke-test batch mode: `aseprite -b --script` creates a sprite and saves a .aseprite file
- [x] Write `scripts/aseprite/anchors.lua` (markers to per-frame slice keys in the export JSON)
- [x] Lua generator `scripts/aseprite/build_sprite.lua` + design module `assets/clawd/design/clawd.lua`; sheets reviewed at 8x; rebuild is byte-reproducible
- [x] Next art: stroll (`wave`) and stargaze (`look_up`, `adjust`) sheets; every activity state now has drawn art
- **Status:** complete

### Phase 21: Activity system at scale (user: Clawd skiing, making coffee, hiking, chess, tennis, wizard magic and many more, interacting with the scene)
- [x] 21a Design doc `docs/activity-system.md`: character rig, held items and costumes, set pieces, anchor effects, interaction types, scene fit (capabilities, new archetypes), schema extensions, content waves, per-activity workflow, budgets
- [x] 21b Rig v2 prototype in `assets/clawd/design/clawd.lua`: facing (front/side), lean, crouch/sit, arms posed by hand position, held items drawn as code, hats/costumes, automatic anchors (hands, item tip, head, seat)
- [x] 21c Pose catalogue `_template_poses.lua`: ski, coffee, hike, chess, tennis, wizard; render at 6x and review
- [x] 21d Fold the content waves into the milestone phases (M3 Wave A, etc.) and PLAN.md section 4.7
- **Status:** complete

### Phase 22: Logging system for debugging and QA (user request)
- [x] 22a Design `docs/logging.md`: levels, targets, structured fields (seed, recipe hash, generator version, activity), what to log where, privacy, rotation/retention, crash reports, diagnostic bundle for bug reports, QA workflow
- [x] 22b Instrument scene (compose/build decisions and fallbacks) and render (timings, character fallbacks) with `tracing`; no effect on output
- [x] 22c CLI: `-v`/`-vv`, `--log-file` (JSON lines), `CLAIRLUNE_LOG` filter, startup environment line, panic hook that logs location and scene context
- [x] 22d Verify: logs carry the seed and recipe hash; cargo-deny still passes; tests unaffected
- [x] App side: rolling files in <data_dir>/logs, 7 days / 50 MB, crash reports, Detailed logging, diagnostic bundle (24d)
- **Status:** complete

### Phase 23: Wave A4 activities: reading by a lantern, guitar by the fire
- [x] Rig: held items `book`, `guitar`; check `sit` pose reads well
- [x] `notes` particle preset (font glyph check for musical notes)
- [x] Props: closed book, guitar leaning (unattended variants)
- [x] read_lantern and play_guitar designs + TOML, export, visual check, goldens
- **Status:** complete

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Build M1 before M0 | M0 needs Aseprite and hand-drawn art from the user. M1 uses the prototype's placeholder pixel maps, so it isn't blocked. |
| Internal crates are named `clairlune-scene` / `clairlune-render` (dirs stay `crates/scene`, `crates/render`) | crates.io requires every dependency of a published crate to be published too, and the bare names `scene`/`render` are taken. |
| Glyph rasterizer = ttf-parser + our own supersampler instead of fontdue | fontdue has separate SIMD (x86) and scalar (ARM) accumulation paths, which risks different bytes on macOS ARM CI. Basic f64 ops are IEEE-exact everywhere. |
| Fonts live inside `crates/render/assets/fonts/` | `include_bytes!` paths must be inside the package for `cargo publish`. Same rule will apply to sprite sheets in M2. |
| Glow tint folded into background bands 4-7 | Keeps ink entries at 8 bands x 6 levels = 48 (matches the section 4.5 budget) while still tinting glyphs near the fire. |
| Planning files live in `.planning/` | Keeps the repo root clean. Not part of any published crate. |
| Font outlines from `skrifa` instead of `ttf-parser` | cargo-deny flagged ttf-parser unmaintained (RUSTSEC-2026-0192). skrifa is the named successor. Golden images stayed byte-identical after the switch. |
| Character size, ledge margin, glow and embers scale with `min(W, H)` | In portrait, scaling by height made the focal group 47 % of the width. No change on landscape screens. |
| Test crate `tests/` (`clairlune-tests`, publish = false) holds goldens and integration tests | Keeps repo-specific test data out of the published crates; matches the `tests/golden/` layout in PLAN.md. |
| Sprite exports go to `crates/sprites/assets/clawd/`, sources stay in `assets/clawd/src/` | `include_bytes!` must stay inside the published package; PLAN.md put both under `assets/clawd/` |
| Light-theme outline is drawn by the engine, not the art | Salmon on paper is about 2.5:1 contrast (dark theme about 5.9:1); `#8C4A36` outline gives about 5.4:1 |
| Commits use the GitHub noreply address (repo-local git config) | GitHub rejected the push: the account has email privacy on. |
| Character motion is not part of the 6 s loop in live mode | PLAN.md 4.4: live plays seeded variations; the loop test covers periodic elements (Buddy off). GIF loop scripts come in M3 |
| `clairlune-sprites` embeds whatever sheets exist via build.rs | Exporting art needs no code change; missing art falls back to the placeholder, never a crash |
| Anchors are marker pixels on an `anchors` layer, converted to per-frame slice keys at export | Aseprite's Lua API can't set per-frame slice keys, and the art is generated by scripts |
| Drive Aseprite through its CLI, not an MCP server | The MCP servers wrap the same CLI; scripts in the repo keep the drawing history reviewable |
| Minimum OS: Windows 10 22H2, macOS 13 (universal), Linux glibc 2.35 | See docs/platform-support.md |
| Clawd art is written as Lua (design files) and built into .aseprite by script | Reviewable drawing history in git; byte-reproducible rebuilds; can still be hand-edited in Aseprite |
| Activities scale through a rig + effect library + set pieces, delivered in waves (docs/activity-system.md) | Hand-made frames per activity don't scale; each wave adds one engine capability and a batch of activities |
| Saved wallpapers (archive, library, timeline) are frozen scene packages; GIF is export-only | GIFs loop and can't change theme or Buddy; recipes alone break when the generator changes (v2 recipes already fail on v3). See PLAN.md 9c |
| Scrolling scenes loop by making every layer periodic and moving whole periods per loop | Seamless with no fade, finite memory, deterministic integer offsets (docs/scrolling-scenes.md) |
| Logging via `tracing`: every scene line carries seed + recipe hash; JSON-lines files; logs never feed back into generation | Any logged problem can be reproduced with `generate --recipe`; output stays deterministic (docs/logging.md) |
| GIF loops are 6 s or 12 s (multiples of the particle period), not 3/10 s | Particle lives of 1-3 s only all divide multiples of 6 s, so only those loop seamlessly |
| Anchor effects draw after the character as an overlay (glyph pixels only) | Steam over a held mug and sparkles near the hat stay visible; embers keep replacing whole cells |
| An activity's GIF loop repeats its first looping state, so the showpiece state should be `start` | The wizard's loop was mostly pondering until `cast` became the start state |
| Theme-locked golden entries are recorded as `locked` in hashes.txt | Night-only stargaze seeds have no light image; the manifest still pins that they stay locked |
| Golden tolerance is 0 differing pixels | The generator is designed to be byte-identical everywhere, so any difference is a real change. |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| CI on 8670ff3: Windows-only `recipes_match_golden_json` failure | 1 | As predicted: CRLF checkout changed the activity-data hash. Fixed in dd2ec35 (eol=lf + normalised hashing) |
| Fixture raw string `r#"..."#` closed early by `"#0000ffff` | 1 | Use `r##"..."##` |
| Broken commit b729c44 pushed: shell heredoc turned `

` escapes into raw newlines, and `;` let the commit run after cargo fmt failed | 1 | Repaired via a script file (no shell escaping); fixed in the next commit. Commits now chain checks with `&&` |
| git push declined (email privacy) | 1 | Repo-local user.email = GitHub noreply; amended the unpushed commit |
| cargo publish verify: `unresolved import ttf_parser` | 1 | Stale unpacked clairlune-render 0.1.0 in cargo's temporary registry from the earlier dry run. Deleted `~/.cargo/registry/src/-da77451d61904e7d` and `target/package`; nothing had been uploaded |

### Phase 24: App gaps (user: "Do all", 2026-11-20)
- [x] 24a Monitor changes: a hidden top-level watcher window (Windows) hears WM_DISPLAYCHANGE / WM_DPICHANGED / work-area changes and TaskbarCreated; after 1.5 s of quiet the app compares the monitor layout (winit: name, position, size) with the one the wallpaper was made for and re-applies the current scenes if it differs (same seeds, saved scenes replayed where sizes match); elsewhere it polls every 5 s. Explorer restarts restart live at once. Tested by posting the messages to the watcher only
- [x] 24b Repeat avoidance: the worker keeps the last 12 picks in <data>/recent.json and composes each monitor with them, so a wallpaper never starts with the activity just shown and two monitors differ (test)
- [x] 24c Archive window (docs/design.md wireframe): newest first, grouped Today / Yesterday / date, the entry on screen marked; one action bar: Use as wallpaper, Pin/Unpin, Save to library, Save PNG/GIF, two-step Delete; header shows retention and size against the cap; tray "Archive..." and Settings open it (folder button inside); UI test screenshot
- [x] 24d Logging extras: crash reports in <data>/crashes (version, system, panic, backtrace, last 200 log lines, recipes on screen; the next launch says so once), Detailed logging for 24 h (reloadable filter), diagnostic bundle zip (about, 3 days of logs, crash reports, current + last 5 recipes, settings and timeline; home folder as ~), logs pruned to 50 MB at start. Tests for each
- [x] 24e Timeline: the next slot is made 4 s ahead and shown at its start (jobs carry a show-at instant; tried here: prepared at :56.000, shown at :00.05); the watcher window reports resume from sleep and clock or time-zone changes, which re-resolve at once
- [ ] 24f `preview-activity` developer window with hot reload
- **Status:** in progress

### Phase 25: The user's feedback from trying the app (2026-11-20)
- [ ] 25a Smoothness is a pace bug, not a feature: higher settings make effects faster (the fire looks bad at the smoothest setting). Every animation (fire, embers, particles, props, Clawd) keeps its own slow pace on every setting; Smoothness only changes how often a frame is drawn, so lower settings are choppier and lighter, never slower or faster. Find what samples raw time instead of each element's own frame timing, fix it, and slow the fire down. Test: an element's frame at time t is the same whatever the frame rate (sample 6 vs 60 fps, compare)
- [ ] 25b Make the app much lighter. The user saw CPU 0.2 %, GPU 2.5-5.5 %, memory 400 MB+. Measure each state (tray only, live, a Settings/Library/Timeline window open, after closing it) and cut: UI windows release their GPU device on close and use small memory hints (likely where the 400 MB comes from; the live part measured 147 MB here); draw nothing when a frame doesn't change; fewer full-screen redraws (partial presents, e.g. DXGI Present1 dirty rects, or one shared device); lower GPU time per frame; set budgets and test them
- [ ] 25c Pause option for maximized and full-screen apps: today a monitor pauses on its own when a maximized or full-screen app covers it (16e), with no setting. Add Settings > Live "Pause when an app is maximized or full screen": on that monitor (default) / on all monitors / off
- [ ] 25d Buddy page: remove the "Show Clawd" description. When the toggle changes, a faint note in Settings for 2 s, at most 5 words, saying the change applies to the next wallpaper (e.g. "Clawd skips the next wallpaper" / "Clawd joins the next wallpaper"); check that this matches what the toggle really does
- [ ] 25e Light theme: the ASCII is often too faint to notice. Raise the ink contrast and glyph density in the light palettes and bands so the art reads as clearly as in the dark theme; review every landscape in light, update goldens
- [ ] 25g Installers, no terminal needed (user asked, 2026-11-20; moved forward from M8): Windows: an MSI or setup .exe (cargo-packager) with a Start menu entry, optional start at login, uninstall from Settings > Apps; macOS: a .app in a DMG to drag to Applications (signed and notarized needs an Apple developer account: ask the user); built by CI on each release and attached to a GitHub release. README install steps for non-developers
- [ ] 25h Schedule vs Timeline overlap: both pages are always shown, but in Timeline mode the Schedule's "change every" is ignored except on days without slots, so the two can look contradictory. Fold Schedule into Rotation: Generate shows "Change every" and its options; Timeline shows the timeline, plus one line for what empty days do. Old settings files keep loading
- [ ] 25i macOS wallpapers don't appear (user's Mac): not a permission. The macOS host was never written (13c); `detect()` answers Unsupported. Write it: NSWorkspace setDesktopImageURL per screen, re-applied on Space changes and screen changes, unique file names. setDesktopImageURL needs no permission; check what else does (live desktop window, start at login via SMAppService, Automation if any AppleScript fallback) and, for any that does, ask at launch with a short guide that opens the right System Settings pane and re-checks. Tested by the user on their Mac (aarch64, Rust 1.100), steps sent to them
- [ ] 25f Richer scenes: some wallpapers look basic. Add complexity to some of them (more layers, detail, props and life in the plainer landscapes and variants), keeping the calm look; contact-sheet review, goldens, GENERATOR_VERSION bump if output changes
- **Status:** pending
