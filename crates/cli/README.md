# clairlune-cli

Developer CLI for [Clairlune](https://crates.io/crates/clairlune): renders procedural ASCII-art wallpapers to PNG from a seed, with no desktop app involved.

```sh
cargo install clairlune-cli
clairlune-cli generate --seed 7 --theme dark --size 3840x2160 --out wallpaper.png
```

| Flag | Meaning |
|---|---|
| `--seed <u64>` | Scene seed. Omit it for a random one (printed). |
| `--recipe <path>` | Render a saved recipe JSON instead of a seed. |
| `--theme light\|dark` | Palette and time of day. |
| `--size <W>x<H>` | Output size in pixels (default 1920x1080). |
| `--out <path>` | Output PNG (indexed, at most 255 colours). |
| `--activity <id>` | Force an activity: `warm_hands`, `stroll`, `stargaze` (night only). |
| `--no-buddy` | Same scene without the character. |
| `--recipe-out <path>` | Also write the recipe as JSON. |
| `--time-ms <t>` | Render a given animation time instead of the hero frame. |

The same seed, size and theme always produce the same file, on any OS.

> Unofficial fan project. Not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character. With the buddy on, images contain fan art of Clawd; `--no-buddy` images contain only this project's own content.

License: MIT OR Apache-2.0.
