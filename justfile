# Task runner (https://github.com/casey/just). Every recipe is a plain command,
# so you can also run them by hand.

# Export Clawd sprite sources with the Aseprite CLI (set ASEPRITE if not on PATH).
export-sprites *names:
    python scripts/export_sprites.py {{names}}

# Check that sprite exports match their sources (what CI runs).
check-sprites:
    python scripts/check_sprites.py

# Render a scene, e.g. `just render 7 dark 3840x2160`.
render seed="7" theme="dark" size="1920x1080":
    cargo run --release -p clairlune-cli -- generate --seed {{seed}} --theme {{theme}} --size {{size}} --out renders/seed-{{seed}}-{{theme}}.png

# Rewrite golden images after an intentional output change.
bless:
    CLAIRLUNE_BLESS=1 cargo test -p clairlune-tests --test golden --test determinism

test:
    cargo test --workspace

perf:
    cargo test --release -p clairlune-tests --test perf -- --ignored --nocapture
