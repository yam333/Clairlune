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
- Phase 8: per-crate READMEs (compiled as doctests), licence copies, repository URL. Dry run passes. User said to stop asking for permission, so publishing proceeds.

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| cargo test --workspace | all pass | 19 + 4 + 2 doc + 5 + 1 + 4 prop pass, perf ignored | pass |
| golden mutation check | fails on tampered hash | failed naming seed-2-dark | pass |
| perf 4K (release) | < 150 ms | 29.5 ms | pass |
| clippy -D warnings / fmt --check | clean | clean | pass |
| cargo deny check | ok | ok (after skrifa switch) | pass |
| cargo publish --workspace --dry-run | ok | ok, 4 crates | pass |

### Errors
| Error | Resolution |
|-------|------------|
| clippy: `chunks_exact_mut` with constant size (rng.rs) | Use `as_chunks_mut::<8>()` |
| cargo-deny: RUSTSEC-2026-0192 ttf-parser unmaintained | Switched to skrifa 0.50.0 |
| Portrait: focal group 47 % of width, ledge off-screen | Scale objects by min(W, H) |
