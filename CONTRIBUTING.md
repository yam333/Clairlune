# Contributing to Clairlune

Thanks for helping. Bug reports, ideas, art and code are all welcome.

## Reporting a bug

In the app, Settings > General > Report a problem opens a GitHub issue with the
version, system, monitors and recent log lines filled in; you read them first,
and nothing is sent until you send the issue. Otherwise, use the
[bug report form](https://github.com/yam333/Clairlune/issues/new/choose).

Security problems go through a private report instead: see
[SECURITY.md](SECURITY.md).

## Setting up

- Rust 1.95 or newer (`rustup update stable`).
- The app builds on Windows 10/11 and macOS 13+. Everything else (the scene
  and render crates, the CLI, the tests) also builds on Linux.
- Python 3 for the sprite scripts, and [just](https://github.com/casey/just)
  if you like task runners (every recipe is a plain command too).

```sh
cargo run --release -p clairlune          # the app
cargo run --release -p clairlune-cli -- generate --seed 7 --out wallpaper.png
```

`clairlune --ui-test <dir>` opens every window with sample data in a scratch
folder, saves screenshots into `<dir>` and quits. It never touches your
wallpaper or settings, so it's the way to look at a UI change.

## Before a pull request

CI runs these on Windows, macOS and Linux; running them first saves a round
trip:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --release -p clairlune-tests --test golden --test determinism
cargo deny check
python scripts/check_sprites.py
```

On Linux, add `--exclude clairlune` to clippy and the tests (the app doesn't
build there).

### Determinism

The same seed must give the same pixels on every platform. The scene and
render crates use their own stable hashing and integer or carefully ordered
float arithmetic, never the OS, the clock or a `HashMap`'s order. The golden
tests check this. If a change is meant to alter the output, re-bless the
images with `just bless` (or `CLAIRLUNE_BLESS=1` on the golden tests) and say
so in the pull request. The pinned `tests/golden/recipes-v3` files must not
change: saved wallpapers have to look the same after an update.

### Art

Every sprite is drawn by this project; see
[docs/dev/clawd-style.md](docs/dev/clawd-style.md). Sprite sheets only use the
character's existing colours, and new prop colours go at the end of the
palette, so older scenes keep their colours. `python scripts/check_sprites.py`
checks the exports against their sources.

### Interface

The windows follow [docs/dev/design.md](docs/dev/design.md): the design
tokens, one accent, and as little text as possible. A setting's name should
say what it does; show (an icon, a preview, an animation) rather than
explain.

### Commits

A short summary line saying what changed, and a body saying why when it isn't
obvious. One topic per pull request. Add a line to `CHANGELOG.md` under
"Unreleased" for anything a user would notice.

## Licence

Unless you say otherwise, anything you contribute is dual-licensed under
MIT OR Apache-2.0, like the rest of the code, with no additional terms. The
Clawd artwork is fan art under its own terms; see [NOTICE](NOTICE).
