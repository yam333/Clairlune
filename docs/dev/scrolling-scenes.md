# Scrolling scenes ("journeys")

Wallpapers where the camera travels: Clawd hikes across mountains while the landscape slides by, then the whole thing loops back seamlessly. Status: seven journeys ship (2026-11-21: walks, flights and a desert trek, with landmarks; CPU composition); GPU layer grids are M7. Builds on `docs/dev/activity-system.md`.

## The idea: a world that wraps around

The terrain is not endless (that would cost memory and CPU without limit). It is a **periodic strip**: a panorama several screens wide whose right edge continues seamlessly into its left edge, like the label on a can. The camera pans along it at a constant speed. After one full period the view is identical to the start, so the loop has no seam, no jump and no fade.

## Seamless parallax

Each layer (sky, far peaks, mid hills, near ground) has its own period `P_i` (its strip width) and moves `n_i` whole periods per loop of length `T`:

```
speed_i = n_i * P_i / T        offset_i(t) = (speed_i * (t mod T)) mod P_i
```

Because every layer completes whole periods in `T`, all of them line up again at `t = T`, and the whole scene loops perfectly. Parallax comes from choosing larger `P_i * n_i` for nearer layers.

A layer's strip is at least a screen wide, so the slowest *moving* layer covers a whole screen per loop; at a walking pace that makes long loops. The way out is physical: what is far enough away doesn't move at all. Sky, stars, sun and moon, and the far peaks are baked into the still background; only the nearer layers scroll.

The pace is set in cells per second (the recipe's `journey_pace_c`, 4.6 to 5.4 for the hike, about the walk's own stride), and the loop follows from it: as long as the ground underfoot takes to come round, in whole 6 s particle loops (126 to 144 s on 16:9 screens). Cells grow with the screen, so a 1080p and a 1440p monitor scroll at the same speed for the eye. (0.11.0 set a loop of 24 to 36 s instead, which scrolled about four times faster and differed per monitor; its recipes keep that.) A GIF holds a whole journey loop only up to 36 s; longer ones export an excerpt of the length asked for. Live drops journeys below 12 fps (Battery saver): new wallpapers skip them and one on screen is replaced. As built for the ridge hike:

| Layer | Period | Turns per loop | Speed |
|---|---|---|---|
| sky, stars, far peaks | (still) | 0 | 0 |
| middle range | `1 W` | 1 | 1 |
| ridge underfoot (Clawd walks on it) | `2 W` | 1 | 2 |

The general scheme, with screen width `W`:

| Layer | Period | Turns per loop | Relative speed |
|---|---|---|---|
| sky / stars | `1 W` | 1 | 1 (slowest) |
| far peaks | `2 W` | 1 | 2 |
| mid hills | `3 W` | 1 | 3 |
| near ground (Clawd walks on it) | `3 W` | 2 | 6 |

Offsets are **integer pixels** computed from `t` with integer arithmetic, so stills, GIF frames and live frames agree exactly.

## Generating periodic terrain

Every noise lookup for a scrolling layer wraps its lattice: lattice cell `ix` becomes `ix mod period`. Terrain sampled at world column `c` uses `c mod P_cols`, so column `P_cols` is exactly column 0 and the strip tiles with no seam. Each fBm octave doubles both frequency and period, so wrapping holds at every octave. The primitives are `noise::value1_periodic`, `fbm1_periodic` and `fbm2_periodic_x`; tests prove exact repetition.

Props placed along a layer (trees, signposts, a cabin, a frozen lake) use world coordinates modulo that layer's period, so they also come around again.

## Clawd on a journey

- Clawd stays near a fixed screen position (a third of the way in, facing the direction of travel) while the ground moves under him. That's the `journey` motion type.
- **Feet follow the terrain**: his y is the near layer's surface height at `camera_x + screen_x`, so he climbs and descends with the hills. That height is periodic too, so the loop holds. A slight tilt on steep slopes comes from the rig's `lean`.
- **No foot sliding**: the near layer's speed equals the walk's stride speed. One walk cycle moves the ground by exactly one stride, and the loop length is a whole number of walk cycles.
- The state machine still plays variations (pausing to look at the view, waving, taking a photo), but the camera keeps moving, so pauses use poses that read as "walking slowly" or "looking around" rather than standing still.

## As built (2026-11-21)

- Scene: `ScrollLayer` strips (cells, `cols` = the period, `turns` per loop) and `Journey { loop_ms, layers, ground, feet_x }` on the scene graph. A strip's `Cell::default()` lets what is behind show; `HOLE` hides it (a gap in a silhouette's fill). `terrain::layers::Strip` paints a layer with wrapped noise (`ridged1_periodic`, `fbm1_periodic`, `fbm2_periodic_x`) and a wrap-around outline.
- Feet: `ground` is the walking layer's surface for every pixel column of its strip, taken as the highest point across the middle of the feet; `SceneGraph::on_ground` moves the character to it at each time.
- Render: `JourneyBake` turns each strip into ink levels once (see-through, background band, or a level), and each frame lays them over the still background at their offsets. Ink takes its colour from the band under it, so vignette and glow stay put while the land moves. 4K: 2.5 ms per frame plus 2.3 ms to 0RGB (Direct3D 11); live is capped at 30 fps for journeys.
- Packages: format 2 adds `journey-<theme>-<i>.png` per layer (glyph as grey, shade as alpha); scenes without a journey are still written as format 1.
- GIF: a journey exports its own loop.

## Landmarks and flights (2026-11-21)

The depth reads best when most of the picture holds still and only the land in front moves. So a journey usually has a **landmark**: one big, far shape behind everything, painted into the still background, its foot hidden by the land in front.

- Kinds: `dome` (a granite dome, steep sides and a broad round top), `peak`, `twin_peaks`, `volcano` (a broad cone with a notched crater), `mesa` (a butte: flat top, sheer cliffs, a talus slope) and `moon_rise` (a moon far bigger than the sky's own, half risen behind the land, on the side away from Clawd).
- Each landscape has its own list: mountains (dome, peak, twin peaks, volcano), snowfield (peak, twin peaks, dome), forest (dome, peak, volcano, rising moon), desert (mesa, volcano, rising moon, dome). Five journeys in six get one.
- The recipe records it (`composition.landmark`, left out when empty), picked from its own random stream, so nothing else about a scene changes and older recipes come out as before.
- Texture: an outline that follows the face (`/` rising, `\` falling, `|` on cliffs, `_` on tops), strokes along the face just under the edge, then a noise fill (`v` and `^` rock, `:` ash, `=` strata) that fades toward the foot; peaks in the mountains and the snow wear snow.

**Flights** are journeys where Clawd doesn't touch the ground: the activity's `altitude` puts the feet that far down the screen, on a gentle swell (two over the near strip, so it loops) kept clear above the land going by. The still background gets room around the flier, the way a still scene clears around its group; the moving layers pass in front of that clearing. `broom_journey` (dusk to night, sparkles from the bristles) and `balloon_journey` (by day, a slow drift) fly over the ridge, the snow trail, the forest path and the desert's `dune_trail`.

**The desert** has a route of its own: the sky (with its big sun or moon when there's no landmark) and the mesas stay put; the far dunes slide by, and the trail with brush, rocks and the odd cactus twice as fast. `desert_trek` walks it in goggles and a scarf.

## Rendering cost

- **Live (GPU)**:
  - each layer is a **cell grid** (glyph index + shade), not a pixel image, rendered as instanced glyph quads from the glyph atlas with wrap-around addressing;
  - a 4K near layer three screens wide is about 113k cells per layer (a few hundred KB);
  - background bands (vignette, glow) stay in screen space and are computed per pixel in the shader;
  - per frame the cost is a few draw calls with UV offsets.
- **Stills and the static fallback**: the hero frame at `t_hero`, composed on the CPU.
- **GIF export**: a full journey loop is long (20-60 s), which makes for a big GIF. Export offers the full loop at a smaller size or a short excerpt (not seamless, labelled as such).
- **Saved scenes**: the frozen package stores each layer's strip (cell grid or indexed PNG) plus periods, speeds and `T`, so a saved journey plays exactly as it did.

## Schema

```toml
# hike_ridge.toml (sketch)
[motion]
type = "journey"
direction = "right"
loop_s = [24, 40]             # seeded within the range, snapped so layers complete whole periods
screen_x = 0.33               # where Clawd walks, as a fraction of the width

[requires]
terrain = ["ridge"]           # archetypes must offer a periodic variant
```

Archetypes declare `journey = true` when they can generate periodic layers. Most can: the change is in how noise is sampled, not in what is drawn.

## Journey ideas

Each one is an activity plus a periodic archetype:

| Journey | Ground / vehicle | Parallax |
|---|---|---|
| Hiking a mountain ridge | ridge path, rocks, signposts | far peaks, clouds |
| Cross-country skiing | snowy rolling hills, ski tracks left behind | pine forests, mountains |
| Desert trek, built | dunes | mesas, a landmark |
| Riding a train | Clawd on the roof or at a window | fields, poles, distant towns |
| Sailing along a coast | waves, boat bob | cliffs, lighthouse, gulls |
| Hot-air balloon or broom flight (wizard), built | the land far below | a landmark, stars |
| Cycling across a city at dusk | rooftops, street | skyline, lit windows |
| Walking a forest trail in autumn | leaves falling | layered trunks |
| Night sky drift | (no ground) | stars, aurora bands |
| Running along a beach | wet sand, footprints washing away | sea, sunset |

Vertical journeys (climbing a tower, a balloon rising) use the same maths on the y axis.

## Plan

- **Wave J (with M4)**, done: periodic noise, periodic layer builder for `mountains`, `journey` motion, near-layer height sampling for the feet, integer-pixel layer offsets, CPU composition for stills, live and GIF; first journey: hiking a ridge.
- **M7**: GPU layer grids with wrap addressing.
- **Then**: more periodic archetypes and journeys from the table above.
