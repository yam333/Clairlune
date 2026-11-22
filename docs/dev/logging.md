# Logging, diagnostics and QA

Goal: when something looks wrong (a broken scene, a stutter, a wallpaper that didn't apply), the logs say exactly **which scene**, **what was decided**, **what failed** and **how to reproduce it**. Status: foundation implemented in the CLI and the generator (2026-11-19); app-side pieces arrive with milestone M6.

## Principles

1. **Every scene event carries its identity**: `seed`, `recipe_hash`, `generator_version`, `activity`, `archetype`, `size`, `theme`. Any log line about a scene can be turned into a reproduction with `clairlune-cli generate --recipe`.
2. **Decisions are logged, not just errors.** Generated content fails in surprising ways, so composition logs why it picked what it picked: fallbacks, theme locks, missing art falling back to the placeholder.
3. **Structured, not prose.** Fields are key-value (`tracing`), so logs can be filtered and diffed. Files are JSON lines.
4. **Logging never changes output.** The generator stays deterministic; nothing it logs feeds back into it.
5. **Private by default.** No user names, paths inside the home folder are shortened to `~`, no network (the update check and an update download send nothing about the user; the check logs only the version it found). Report a problem puts the version, system, monitors and the last 30 log lines (paths already shortened) into a GitHub issue the user reads before sending. Logs stay on the machine unless the user exports a diagnostic bundle.

## Levels

| Level | Use | Examples |
|---|---|---|
| `error` | Something the user will notice failed | wallpaper couldn't be applied, a sprite sheet failed validation, a panic |
| `warn` | Degraded but handled | no eligible activity (archetype-first fallback), live mode unavailable so static is used, art missing so the placeholder is drawn, GPU init failed so the CPU path is used |
| `info` | One line per significant event | scene composed, scene applied, theme switched, archive swept, export written (with timing) |
| `debug` | The decisions behind an event | each weighted pick and its candidates, layout numbers, palette size, hero-time search |
| `trace` | Per-frame or per-cell detail, off by default | frame timings, particle counts |

## Targets

Log targets follow the crates: `clairlune_scene::compose`, `clairlune_scene::build`, `clairlune_render`, `clairlune_sprites`, and later `clairlune_platform`, `clairlune_live`, `clairlune_store`, `clairlune_app`. Filter by target, e.g. `CLAIRLUNE_LOG=clairlune_scene=debug,info`.

## Where logs go

| Surface | Output | Default |
|---|---|---|
| CLI (now) | stderr, human-readable; `--log-file <path>` adds JSON lines | `warn`; `-v` = info, `-vv` = debug, `-vvv` = trace; `CLAIRLUNE_LOG` overrides |
| App (M6) | rolling JSON-lines files in `<data_dir>/logs/` (`tracing-appender`, daily) | `info`; Settings → About → "Detailed logging" switches to `debug` for 24 h |

**Retention** (app): keep 7 days or 50 MB, whichever is smaller; oldest files are deleted first.

## Crash reports

- A panic hook logs the panic message, the location, the current scene identity and a backtrace at `error`, before the process exits.
- In the app (M6) the hook also writes `<data_dir>/crashes/<timestamp>.json`: version, OS and desktop environment, the last 200 log lines and the current recipes. The next launch offers to open the folder.
- The static wallpaper is always set before live mode starts, so a crash never leaves a blank desktop.

## Diagnostic bundle (app, M6)

Settings → About → "Export diagnostics…" writes one zip for a bug report:
- app version, `generator_version`, OS version, desktop environment, compositor, GPU adapter, monitor layout (sizes, DPI, stable ids);
- the last 3 days of logs and any crash reports;
- the current and last 5 archived recipes, so each scene can be reproduced exactly;
- settings (no paths beyond `~`-relative ones).

Nothing is uploaded; the user attaches the zip to a report themselves.

## What each subsystem logs

| Subsystem | `info` | `warn` / `error` | `debug` |
|---|---|---|---|
| compose | scene composed (seed, activity, archetype, times of day, recipe hash) | no eligible activity, fallback used | candidates and weights for each pick |
| build | graph built (size, grid, theme, Buddy) | theme-locked request, unknown activity or archetype | layout: ground row, focal box, light, emitters |
| render | still rendered (timings: build, atlas, bake, frame) | character art missing, so the placeholder is drawn | hero-time search result, palette size |
| sprites | sheets loaded (count, colours) | validation errors, with sheet and frame | per-sheet tags and anchors |
| platform (M5) | wallpaper applied (host, monitor, file) | host failed, fallback host used | DE detection, API calls |
| live (M7) | live started or paused (reason) | GPU fallback to CPU, frame budget overrun | `trace`: frame times |
| store (M6) | archive write, retention sweep (deleted n, freed MB) | disk full, corrupt entry skipped | file operations |
| scheduler (M6) | next change at T, slot resolved | missed deadline (sleep or wake) | timeline resolution steps |

## QA workflow

1. Reproduce with `clairlune-cli generate --seed N -vv --log-file qa.jsonl` (or `--recipe` from a bundle).
2. Check the decisions in the debug lines: which activity and archetype, which fallbacks fired.
3. Fix, add the seed to a test (or the golden set) if it exposed a generator bug.
4. Use `contact-sheet` to check that the fix doesn't hurt variety.
