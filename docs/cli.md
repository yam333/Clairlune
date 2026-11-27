# The command-line tool

`clairlune-cli` renders Clairlune's scenes without the app, on Windows, macOS and Linux. Install it with `cargo install clairlune-cli`, or run it from a checkout with `cargo run --release -p clairlune-cli -- <command>`.

## Stills

```sh
clairlune-cli generate --seed 7 --theme dark --size 3840x2160 --out wallpaper.png
```

| Flag | Meaning |
|---|---|
| `--seed <u64>` | Scene seed. Leave it out for a random one (printed). |
| `--theme light\|dark` | Palette and time of day. |
| `--size <W>x<H>` | Output size in pixels. |
| `--out <path>` | Output PNG (indexed, at most 255 colours). |
| `--activity <id>` | Force an activity, such as `stroll` or `hike_ridge`. An unknown id lists them all. |
| `--archetype <id>` | Force a landscape: `canyon`, `mountains`, `forest`, `desert`, `snowfield`, `shore`, `city_rooftops`, `night_sky`, `cave`, `interior`, `meadow`, `lake`, `volcano`, `islands`, `lunar`, `bamboo`, `waterfall`, `countryside`, `ruins`, `savanna`, `swamp`, `mushrooms`, `crystals`, `great_tree`, `wizard_tower`, `standing_stones`, `lantern_lake`, `lighthouse`, `harbour`, `zen_garden`, `observatory`, `oasis`, `windmills`, `fjord`, `viaduct`, `shrine`, `greenhouse`. |
| `--weather <id>` | Force the weather where the scene allows it, such as `rain`, `fog`, `birds`, `seeds`, `shooting_stars`, or a landscape's own: `spray`, `sandstorm`, `tumbleweed`, `cinders`, `sky_lanterns`, `fireworks`, `spores`, `wisps`, `bats`, `spindrift`, `dragonflies`. An unknown id lists them all. |
| `--look <look>` | `line` or `dither`. |
| `--contrast <c>` | `soft`, `normal`, `high`, or `vary` (one per seed). |
| `--palette <p>` | `classic` (the default), `nord`, `gruvbox`, `catppuccin`, `solarized`, `rose_pine`, `tokyo_night`, `phosphor`, `amber`, `everforest`, `kanagawa` or `dracula`. |
| `--letters <l>` | `text` (the default), `braille`, `blocks`, `lines`, `kana`, `shapes` or `hex`. |
| `--no-buddy` | The same scene without Clawd. |
| `--recipe-out <path>` | Also write the scene recipe as JSON. |
| `--time-ms <t>` | Render a given moment instead of the default frame. |

## GIFs

A looping GIF, 6 or 12 seconds long (`--smoothness battery-saver|balanced|smooth|very-smooth|ultra-smooth`):

```sh
clairlune-cli gif --seed 7 --activity warm_hands --size 960x540 --out campfire.gif
```

## Frozen scene packages

A frozen scene package holds everything a scene needs to play, about 0.5-1 MB at 4K, and it plays the same after the generator changes:

```sh
clairlune-cli package --seed 7 --size 1920x1080 --out scene7       # freeze a scene
clairlune-cli generate --package scene7 --theme light --out still.png
clairlune-cli gif --package scene7 --out scene7.gif
```

## Contact sheets

Many seeds in one grid, to check variety:

```sh
clairlune-cli contact-sheet --count 12 --theme dark --out sheet.png
clairlune-cli contact-sheet --activity stroll --count 8 --out strolls.png
clairlune-cli contact-sheet --archetype shore --count 6 --theme light --out shores.png
```

## Setting the wallpaper (Windows)

Set wallpapers directly:

```sh
clairlune-cli monitors                 # ids, physical sizes, current wallpapers
clairlune-cli set --seed 7             # one scene per monitor, at each monitor's resolution
clairlune-cli set --archetype shore --theme light
clairlune-cli restore                  # put back the wallpaper you had before
```

The main monitor gets the seed you give and the others get seeds derived from it, so the same seed always gives the same set. The first `set` remembers your wallpaper so `restore` can bring it back.

## Live playback (Windows)

Play live wallpapers for a while and see what they cost:

```sh
clairlune-cli live --seconds 30 --fps 12
```

This leaves the still frame as your wallpaper when it ends; `clairlune-cli restore` undoes that.

