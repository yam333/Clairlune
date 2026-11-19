# Clairlune

Procedural, animated ASCII-art desktop wallpapers, written in Rust.

Every wallpaper is generated locally from a seed: a landscape built from text glyphs, a small pixel-art buddy doing something in it, and a few animated details such as embers, snow or stars. Generation is deterministic, so the same seed gives the same picture on every computer. There is no AI and no network access at runtime.

![Nine generated wallpapers, one per landscape, in dark and light themes](docs/img/showcase.png)

> **Unofficial fan project.** Not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character.

## Status

Early development. The generator and a developer CLI work today: nine landscapes (canyon, mountains, forest, desert, snowfield, shore, city rooftops, night sky, cave), seven activities for Clawd, weather, stills and seamless looping GIFs. On Windows the tray app already runs: a new wallpaper on a schedule, one scene per monitor, with an archive of past ones. The settings window, live (animated) wallpapers and the other systems come in later milestones; see [PLAN.md](PLAN.md).

## Try the generator

```sh
cargo run --release -p clairlune-cli -- generate --seed 7 --theme dark --size 3840x2160 --out wallpaper.png
```

Options:

| Flag | Meaning |
|---|---|
| `--seed <u64>` | Scene seed. Omit it to get a random one (printed). |
| `--theme light\|dark` | Palette and time of day. |
| `--size <W>x<H>` | Output size in pixels. |
| `--out <path>` | Output PNG (indexed, at most 255 colours). |
| `--activity <id>` | Force an activity: `warm_hands`, `stroll`, `stargaze` (night only), `wizard_magic`, `make_coffee`, `read_lantern`, `play_guitar`. |
| `--archetype <id>` | Force a landscape: `canyon`, `mountains`, `forest`, `desert`, `snowfield`, `shore`, `city_rooftops`, `night_sky`, `cave`. |
| `--no-buddy` | Render the same scene without the character. |
| `--recipe-out <path>` | Also write the scene recipe as JSON. |
| `--time-ms <t>` | Render a specific animation time instead of the hero frame. |

To share an animated version, export a seamless looping GIF (6 s or 12 s; `--smoothness battery-saver|balanced|smooth|very-smooth|ultra-smooth`):

```sh
clairlune-cli gif --seed 7 --activity warm_hands --size 960x540 --out campfire.gif
```

To check variety, render many seeds into one grid:

```sh
clairlune-cli contact-sheet --count 12 --theme dark --out sheet.png
clairlune-cli contact-sheet --activity stroll --count 8 --out strolls.png
clairlune-cli contact-sheet --archetype shore --count 6 --theme light --out shores.png
```

## The tray app (Windows)

```sh
cargo run --release -p clairlune
```

A moon appears in the notification area. The app sets a new wallpaper at start and then every hour, one scene per monitor, and keeps the last week in an archive. Its menu has New wallpaper, Previous wallpaper, Theme (Follow system / Light / Dark), Pause schedule, Save current as PNG (to `Pictures\Clairlune`), Open archive folder, and Settings, which for now opens `%APPDATA%\clairlune\settings.toml`; edits there apply within seconds. Logs are in `%LOCALAPPDATA%\clairlune\logs`. Before its first change the app remembers your own wallpaper; `clairlune-cli restore` puts it back.

## Set it as your wallpaper from the CLI (Windows)

```sh
clairlune-cli monitors                 # ids, physical sizes, current wallpapers
clairlune-cli set --seed 7             # one scene per monitor, at each monitor's resolution
clairlune-cli set --archetype shore --theme light
clairlune-cli restore                  # put back the wallpaper you had before
```

The primary monitor gets the seed you give; the others get seeds derived from it and their monitor id, so each screen has its own scene and the same seed always gives the same set. The first `set` remembers your current wallpaper (per monitor, and the fit mode) so `restore` can put it back. Stills are kept in `%LOCALAPPDATA%\clairlune\current`, each under a new file name. macOS and Linux follow in this milestone.

## Workspace

| Crate | Purpose |
|---|---|
| `clairlune-scene` (`crates/scene`) | Pure, deterministic: seed → recipe → scene graph. No I/O. |
| `clairlune-sprites` (`crates/sprites`) | Aseprite sheet loader, anchors, the `Character` trait. |
| `clairlune-render` (`crates/render`) | Glyph atlas, palette-index compositor, PNG export. |
| `clairlune-platform` (`crates/platform`) | Monitors and setting the wallpaper (Windows today). |
| `clairlune-cli` (`crates/cli`) | Developer tool. |
| `clairlune` (`crates/app`) | The tray app: schedule, per-monitor scenes, archive, settings file. |

## Generated images

With the buddy turned on, wallpapers contain a drawing of Clawd, which is Anthropic's character. If you publish or sell generated images, that is fan art and the same caution applies. With `--no-buddy`, images contain only this project's own content.

## License

Code: either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

The bundled JetBrains Mono font is under the [SIL Open Font License 1.1](crates/render/assets/fonts/OFL.txt).

Clawd artwork is fan art and is **not** covered by the code licence. See [NOTICE](NOTICE).
