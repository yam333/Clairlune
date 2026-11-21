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
| `--activity <id>` | Force an activity: `warm_hands`, `stroll`, `stargaze` (night only), `wizard_magic`, `make_coffee`, `read_lantern`, `play_guitar`, `fishing` (shore only), `fly_kite` (daytime), `skip_stones` (shore only), `broom_flight` (evening and night), `paint_easel` (daytime), `snowballs` (snow only), `balloon_ride` (daytime), `picnic` (daytime), `chess`, `tennis` (daytime), `rowing` (shore only), `skiing` (snow only), `hike_ridge`, `ski_tour`, `sledding`, `forest_walk`, `desert_trek` (journeys: the land scrolls past), `broom_journey` (evening and night), `balloon_journey` (daytime) (flying journeys: the land scrolls past below). |
| `--look <look>` | `line` (line art) or `dither` (light and dark masses of dots and bars). |
| `--contrast <c>` | `soft`, `normal`, `high`, or `vary` (one per seed). |
| `--no-buddy` | Same scene without the character. |
| `--recipe-out <path>` | Also write the recipe as JSON. |
| `--time-ms <t>` | Render a given animation time instead of the hero frame. |

To share an animated version, export a seamless looping GIF (6 s or 12 s; `--smoothness battery-saver|balanced|smooth|very-smooth|ultra-smooth`):

```sh
clairlune-cli gif --seed 7 --activity warm_hands --size 960x540 --out campfire.gif
```

To check variety, render many seeds into one grid:

```sh
clairlune-cli contact-sheet --count 12 --theme dark --out sheet.png
clairlune-cli contact-sheet --activity stroll --count 8 --out strolls.png
```

The same seed, size and theme always produce the same file, on any OS.

When drawing an activity, keep a preview open. Inside the repository, it plays the activity in the scenes it fits and reloads the activity files and sprite sheets each time you save them (N/P: next or previous scene, T: theme, B: Buddy, R: reload, Esc: close). With `--export`, saving a design script or an Aseprite source also runs `scripts/export_sprites.py` (Aseprite needed):

```sh
clairlune-cli preview-activity warm_hands --seed 3 --smoothness smooth
```

> Unofficial fan project. Not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character. With the buddy on, images contain fan art of Clawd; `--no-buddy` images contain only this project's own content.

License: MIT OR Apache-2.0.
