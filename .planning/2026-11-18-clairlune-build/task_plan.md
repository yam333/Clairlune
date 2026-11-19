# Task Plan: Build Clairlune (procedural live ASCII wallpaper app, Rust)

## Goal
Implement PLAN.md milestone by milestone. This session delivers M1 (headless deterministic generator + `clairlune-cli generate` that writes an indexed PNG), with tests and CI, and prepares the crates for crates.io. The user granted full access (no more approval stops), so publishing is part of this session.

## Next Step
Session roadmap (2026-11-19, usage reset), in order:
1. ~~Phase 23: Wave A4~~ done
2. Phase 12 (M4): refactor build into archetype modules (canyon byte-identical), weather particles, then mountains, snowfield, forest, desert, shore, city_rooftops, night_sky, cave; coverage + variety per archetype
3. Phase 13 (M5): static wallpaper setting, Windows first (testable here), monitor enumeration
4. Phase 14 (M6): app shell

## Current Phase
Phase 10 (Phase 9 waits on the user's art)

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
- [ ] Live state machine vs GIF loop scripts, particles, fire flicker, Smoothness levels
- [ ] Activity Wave A (docs/activity-system.md): frames by state (10h), anchor `particles` effects, held items, `light`; activities: wizard magic, making coffee (camp stove), reading by a lantern, guitar
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
- [ ] Remaining 8 archetypes, platforms, props, weather, compatibility tables, constraint checks
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
- [ ] `path`, `bob`, `fly` motion; contact-sheet CLI command
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
- [ ] 13c macOS host   - [ ] 13d GNOME / KDE / XFCE / Cinnamon / MATE hosts   - [ ] 13e wlroots still surface
- [ ] 13f hotplug / resolution / DPI change handling (needs the app event loop, M6)
- [ ] `platform` crate static hosts (Windows, macOS, GNOME, KDE, XFCE, Cinnamon, MATE, wlroots)
- [ ] Unique file names, monitor ids, physical-pixel sizing, hotplug
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
- [ ] 14d Settings window (sidebar groups from PLAN.md 8), every state designed; start at login toggle
- [ ] Frozen scene package (PLAN.md 9c): serialisable SceneGraph + background/sprite PNGs per theme; archive, library and timeline play from it, never from a GIF; test: a package saved by one generator version plays identically after GENERATOR_VERSION changes
- **Status:** pending

### Phase 15: M6b Library + Timeline
- [ ] Library (section 9a), timeline (section 9b) with DST tests and the editor UI
- **Status:** pending

### Phase 16: M7 Live wallpaper hosts
- [ ] Platform spikes, wgpu renderer with palette lookup, pause rules, softbuffer fallback, budgets
- **Status:** pending

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
- **Status:** in_progress

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
- [ ] App side (M6): rolling files in <data_dir>/logs (tracing-appender), 7 days / 50 MB retention, crash report files, 'Detailed logging' toggle, diagnostic bundle export
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
