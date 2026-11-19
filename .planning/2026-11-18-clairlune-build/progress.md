# Progress Log

## Session: 2026-11-18

### Current Status
- **Phase:** 8 - publish (M1 exit)
- **Started:** 2026-11-18

### Actions Taken
- Read PLAN.md (all 1021 lines). Checked toolchain (Rust 1.98.1). Prototype files are missing from the folder.
- Created the named plan `.planning/2026-11-18-clairlune-build/` with 17 phases; validated.
- Phase 2: workspace `Cargo.toml` (4 members, pinned determinism deps), crate manifests with crates.io metadata, LICENSE-MIT, LICENSE-APACHE (from apache.org), README with disclaimer, NOTICE (fan-art carve-out), docs/takedown.md, .gitignore.
- Downloaded JetBrains Mono v2.304 Regular TTF + OFL.txt (TrueType magic 00010000, SHA-256 A0BF60EF…138F) into `crates/render/assets/fonts/`.
- Checked rand_core 0.10 API: the trait is now `Rng` (was `RngCore`); `SeedableRng::from_seed` unchanged. Using it via `rand_chacha::rand_core`, so the direct rand_core dep is dropped.

- Phase 3: scene crate (hash, rng, math, noise, glyphs, palette, placeholder, recipe, compose, graph, particles, build). 19 unit tests pass on first compile.
- Phase 4-5: render crate (atlas with own rasterizer, indexed palette, bake, frame, PNG) and CLI. First 4K render: 31 ms, 534 KB, 111 colours.
- Phase 6: two tuning rounds (convex canyon walls, lighter ledge, portrait fix). See findings.md.
- Phase 7: tests crate, goldens blessed, CI workflow, deny.toml. Installed cargo-deny locally. Swapped ttf-parser for skrifa after the advisory.
- Phase 8 done: pushed to github.com/yam333/clairlune (commit 800d7e4) and published all 4 crates 0.1.0. Installed clairlune-cli from crates.io: output byte-identical to the golden image.
- Phase 8: per-crate READMEs (compiled as doctests), licence copies, repository URL. Dry run passes. User said to stop asking for permission, so publishing proceeds.

- Phase 9 (M0 tooling) committed 0397022: style guide, export + check scripts, justfile. Art itself waits on the user.
- CI on the M1 commit: all 3 OSes green.
- Phase 10 core: archetype capabilities, activity TOML model + validation, activity-first compose (GENERATOR_VERSION 2), episode state machine, focal layout with pace motion, new props (telescope, lantern), CLI --activity, variety test. Goldens re-blessed (16 images + 4 locked).

- 10f: clairlune-sprites crate (Aseprite json-array loader, slice anchors, alpha/colour/origin validation, Character trait, placeholder + sheet-backed implementations, build.rs embedding). 6 tests.

- Phase 18-19: published 0.2.0 (5 crates), trademark check, platform minimums.
- Phase 20: user chose to build Aseprite from source. Downloaded v1.3.18.6 source + Skia m124 to C:/Users/yamzu/Developer/tools/, build script tools/build-aseprite.cmd (VS 2026 with the 14.44 toolset, SDK 26100, VS-bundled CMake/Ninja). Configure OK (145 s), 1575 build steps.
- While compiling: scripts/aseprite/anchors.lua (markers to slice keys), scripts/aseprite/build_sprite.lua (design to .aseprite), assets/clawd/design/ (clawd.lua drawing module, _template, _base: idle/walk/celebrate/alert, warm_hands: warm/look_around), export_sprites.py --build. Placeholder origin moved to the lowest foot row to match the art.

- Phase 11 GIF export: clock.rs (Clock::Live/Loop, LoopScript, loop_track), gif_export.rs (global palette, changed-box sub-images, merged identical frames, dithered delays), CLI `gif`. Tests: decoded frames equal rendered frames for all 3 activities, no seam at 6/12 s, sizes ~100 KB at 960x540.

- Phase 12c/d/f (2026-11-19): mountains, snowfield, desert. Generic layer painter (terrain/layers.rs) with
  slope outlines, hatch, snow, plateaus, stamps; glyph families DISTANT/RIDGE/SNOW/SAND/FOLIAGE/WATER/ICE/EARTH
  and glyphs `_ v ! &`. CLI `--archetype` on generate and contact-sheet. Activities gained dust/leaves/snow
  weather where it fits. tests/coverage.rs (every landscape x theme, feet on platform, share of seeds).
  Goldens: 7 seeds moved to new landscapes and were re-blessed; the 3 canyon seeds are byte-identical.

- Phase 12e-k (2026-11-19): forest, shore, city_rooftops, night_sky, cave. All nine landscapes paint.
  Platforms beyond ledges: pier (shore), rooftop (city), floating island (night_sky). Activities gained
  `requires.sky`. Goldens gained `land-<id>` per landscape. Bug found and fixed while building the forest:
  `unit_from_hash(k >> n)` reads the high bits, so shifted hashes gave values near 0; use salted hashes.

- Release 0.6.0 published (nine landscapes); verified with cargo install.
- Phase 13 (M5, Windows): clairlune-platform (WallpaperHost, StillStore, snapshot/restore, known folders,
  system theme); CLI monitors/set/restore; dirs dropped for its MPL dependency. Verified on 2 monitors.
- Phase 14a (M6 core): the tray app runs on Windows: settings.toml (live reload), schedule, worker thread,
  per-monitor apply, archive + retention, previous, export, follow-system theme, logs, single instance.
  The user's wallpaper is remembered before the first change (never Clairlune's own). Verified live, then
  restored the user's wallpaper and removed the test data.

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| cargo test --workspace | all pass | 19 + 4 + 2 doc + 5 + 1 + 4 prop pass, perf ignored | pass |
| golden mutation check | fails on tampered hash | failed naming seed-2-dark | pass |
| perf 4K (release) | < 150 ms | 29.5 ms | pass |
| clippy -D warnings / fmt --check | clean | clean | pass |
| cargo deny check | ok | ok (after skrifa switch) | pass |
| cargo publish --workspace --dry-run | ok | ok, 4 crates | pass |
| CI (GitHub, 3 OSes) on M1 | green | green | pass |
| CI on 1687ca0 (latest, 3 OSes + cargo-deny) | green | green | pass |
| M2: cargo test --workspace | all pass | scene 30, render 4, determinism 5, golden 16 img, properties 5, doc 2 | pass |

### Errors
| Error | Resolution |
|-------|------------|
| clippy: `chunks_exact_mut` with constant size (rng.rs) | Use `as_chunks_mut::<8>()` |
| cargo-deny: RUSTSEC-2026-0192 ttf-parser unmaintained | Switched to skrifa 0.50.0 |
| Portrait: focal group 47 % of width, ledge off-screen | Scale objects by min(W, H) |
| Loop test failed for pacing scenes | By design (live motion isn't looped); test periodic elements with Buddy off |
| Bless run skipped golden (determinism failed first) | Re-ran with --no-fail-fast |
