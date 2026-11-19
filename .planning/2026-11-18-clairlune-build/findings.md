# Findings & Decisions

## Requirements (from PLAN.md, M1 focus)
- `scene` + `render` crates: canyon archetype, glyph atlas, palette-index compositing (section 4.5), static bake, indexed PNG export, prototype Clawd + campfire as placeholders.
- `clairlune-cli generate --seed 7 --theme dark --size 3840x2160 --out out.png`
- Golden tests at 640x360 for 10 seeds x 2 themes. Determinism test: byte-identical output in CI on all 3 OSes (libm, integer sampling, per-subsystem RNG streams from day one).
- Performance: 4K bake < 150 ms (release).
- Names: `APP_NAME = "Clairlune"`, `APP_ID = "clairlune"` in one place. Never hardcode "Clawd"/"Claude" in identifiers.
- Crate names to publish: `clairlune` (app), `clairlune-cli` (dev tool). The user has a saved cargo token.

## Key numbers from PLAN.md
- Grid: about 90-120 rows; 4K = 11x20 px cells (18 px font); min cell 6x11.
- Canyon shape: `d = |x - c| / 0.5`, `shape = smoothstep(0,1, clamp((d - gap)/(1 - gap), 0, 1))^p`.
- Glyph families: far `'-/|` (mirror `\` on right), mid `.-=\<` / `.-=/>`, near `.:ox=O#X%@`, platform `=o+x#X%@` with `^` top edge, sky `.'`,:*`.
- Holes: density thresholds about 0.28-0.34. Depth shade: far 0.35, mid 0.6, near 0.9. Top-edge fade over about 6 rows.
- Ledges below the platform line dimmed to 50%. Clear cells around Clawd + fire.
- Glow radius about 900 px at 4K (0.417 H); ember tint radius about 650 px (0.30 H). Vignette brightens centre up to 55%.
- Dark palette: bg `#151412`, ink `#E9E5D7`, ember `#FF9646`, Clawd `#D9785C`, eyes `#261A16`. Light: bg about `#ECE9DF`, near-black ink.
- Fire: O `#E86028`, Y `#FFAA3C`, W `#FFECAA`, B log `#784E30`, K dark log `#462E1E`.
- Clawd size: largest integer scale with height <= 9% of screen height.
- Palette <= 255 per render, index 255 reserved for GIF transparency.

## Research Findings
- Toolchain: rustc/cargo 1.98.1 stable MSVC on Windows 11. `cargo publish --workspace` (stable since 1.90) publishes workspace members in dependency order.
- The prototype files (`generate.py`, `wallpaper-7.png`) are NOT in the folder. The canyon is rebuilt from the PLAN.md description (sections 4.3, 13). The user should drop them in later so they can be copied to `reference/`.
- crates.io: a published crate's path dependencies must also be published, so `clairlune-scene` and `clairlune-render` must be published before `clairlune-cli`.
- `include_bytes!` files must live inside the package directory, or `cargo publish` fails verification.
- `rand_core`'s `seed_from_u64` expansion is a library detail; build the 32-byte ChaCha seed ourselves (splitmix64) to avoid drift.

## Visual review log (Phase 6)
- Round 1 (seed 7, 4K, dark + light): render 31 ms, PNG 534 KB / 483 KB, 111 colours. Works in both themes; glow, embers, '^' ledge edge and the character read well.
  - Problem: walls only hug the screen edges, so the upper middle is a big empty sky. Cause: `shape^p` with p in 1.3-2.2 is concave (slow rise). Fix: convex p in 0.45-0.85 and narrower layer gaps.
  - Problem: the ledge is a regular trapezoid, too bright and too dense (`%@X#` everywhere). Fix: noisier edges, lower shade (0.8 top to 0.4), density bias toward lighter glyphs, more holes below the top rows.
- Round 2 (seeds 7, 42 at 4K; seed 3 at 640x360 and 1080x1920 portrait): walls frame the scene, ledge reads as its own object, variety between seeds is good (fire side, thirds line, palette family, wall shapes). 640x360 is coherent.
  - Bug found: in portrait, sprites scaled from the height took 47 % of the width and the ledge ran off-screen. Fix: sprite scale, ledge margin, glow radius and ember unit use the short side `min(W, H)`. Same as before on landscape screens. PLAN.md says "6-9 % of screen height"; this is a deliberate deviation for portrait only.
  - Accepted for M1: big open sky in the upper middle (it's the canyon's open centre, with stars at night). Revisit with real art in M2.

## Test and tooling results (Phase 7-8)
- Tests: scene 19 unit, render 4 unit, 2 README doctests, determinism 5, golden 1 (20 images, 640x360, 641 KB total), properties 4 x 48 cases (7 aspect ratios incl. 21:9, 32:9, 9:16). All pass. Clippy `-D warnings` clean, rustfmt clean.
- Mutation check: changing one hash in `tests/golden/hashes.txt` makes the golden test fail with the exact image name.
- Perf (release, best of 5, 4K): build 3.0 ms, atlas 0.4 ms, bake 26.1 ms, total 29.5 ms (budget 150 ms). Frame composite 1.1 ms.
- cargo-deny: RUSTSEC-2026-0192 (ttf-parser unmaintained) -> switched to skrifa 0.50.0 (unscaled, unhinted outlines). Pixels unchanged. Now advisories/bans/licenses/sources all ok; only warning is two `miniz_oxide` versions via png.
- `cargo publish --workspace --dry-run`: all 4 crates package and verify. Package sizes: scene 31.7 KiB, app 8.2 KiB, render 146.5 KiB (font), cli 11.4 KiB (compressed).
- GitHub: repo github.com/yam333/clairlune exists, private, empty.

## CI results
- First GitHub Actions run (commit 800d7e4): Windows x64, Linux x64 and macOS ARM64 all pass, including the release-mode golden hashes. Cross-OS and cross-CPU byte-identical output is confirmed, not just designed for.

## M2 core (Phase 10a-10e, 10g)
- 3 activities: warm_hands (stationary, campfire + embers, unattended log seat), stroll (pace, lantern), stargaze (stationary, telescope, night only so theme-locked).
- Variety over 1,000 seeds is within 2x of the expected share for each activity. 4 of the 10 golden seeds are stargaze, so they have no light variant.
- Perf unchanged: 4K 29.6 ms.

## Trademark check (2026-11-19)
- USPTO (tmsearch backend `prod-stage-v1-0-0/tmsearch`, works with plain POST) and TMview (`tmdn.org/tmview/api/search/results`, needs a browser User-Agent). Controls found as expected.
- CLAIRLUNE: 0 in the US, 0 at EUIPO. 4 registrations in China (classes 21, 25, 35). No live similar mark in classes 9/42 in the US or EU. Details in docs/trademark-check.md.

## Platform facts (2026-11-19, web)
- macOS 27 is Apple Silicon only; macOS 26 Tahoe is the last Intel release (security updates about 2 more years).
- Windows 10 consumer ESU extended to 12 October 2027.

## Aseprite (2026-11-19)
- Lua API: `Slice` has `bounds`, `center`, `pivot`, `color`, `data`, `name` but no per-frame keys. Scripts can't write slice keys per frame.
- Community MCP servers: rkdfx/aseprite-mcp (Python), willibrandon/aseprite-mcp / pixel-mcp (Go), ext-sakamoro/AsepriteMCP. All need a local Aseprite and run `aseprite -b`.

## 0.2.0 release (2026-11-19)
- The user ran `--activity` against crates.io 0.1.0 and got an error: the README promised a flag only on main. Published 0.2.0 of all 5 crates; installed from crates.io and confirmed `--activity stroll` works.
- GitHub repo was renamed to yam333/Clairlune (capital C); remote and manifests updated.

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Background band = max(vignette band 0-3, glow band 3-7), Bayer 4x4 dithered via precomputed squared-distance thresholds | Exact palette entries, no per-pixel pow, fast enough for the 4K 150 ms budget |
| Ink entry index = INK_BASE + band * 6 + (level - 1) | Ink colour is a lookup on the pixel's band, so every pixel is a palette entry by construction |
| Ember particles get their own band x level entries (48) | Exact over any band; total stays under 255 |
| Placeholder sprites live in `clairlune-scene::placeholder` | `scene` needs the sprite dimensions for layout; M2 moves them into a `sprites` crate behind a `Character` trait |
| APP_NAME / APP_ID live in `clairlune-scene::ident` | Every other crate depends on scene; one place for the name |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| Prototype files missing | Rebuild canyon from the written spec; ask the user for the files |

## Open questions for the user (non-blocking for M1)
- GitHub repo URL (needed for the `repository` field before publishing).
- OK to publish 4 crates (`clairlune-scene`, `clairlune-render`, `clairlune-cli`, `clairlune`)? Required by crates.io.
- The published render/scene crates would contain the placeholder Clawd pixel map (fan art, section 15.2.4). Publish with it, or publish M1 with Buddy placeholder removed?
- Should I `git init` and commit?

## Resources
- PLAN.md (project root), sections 4 (generation), 5 (rendering), 11 (milestones), 12 (testing), 13 (prototype values), 15 (legal)
- JetBrains Mono (OFL-1.1): github.com/JetBrains/JetBrainsMono
