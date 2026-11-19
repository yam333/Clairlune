# Scrolling scenes ("journeys")

Wallpapers where the camera travels: Clawd hikes across mountains while the landscape slides by, then the whole thing loops back seamlessly. Status: design (2026-11-19). Builds on PLAN.md sections 4 and 5 and `docs/activity-system.md`.

## The idea: a world that wraps around

The terrain is not endless (that would cost memory and CPU without limit). It is a **periodic strip**: a panorama several screens wide whose right edge continues seamlessly into its left edge, like the label on a can. The camera pans along it at a constant speed. After one full period the view is identical to the start, so the loop has no seam, no jump and no fade.

## Seamless parallax

Each layer (sky, far peaks, mid hills, near ground) has its own period `P_i` (its strip width) and moves `n_i` whole periods per loop of length `T`:

```
speed_i = n_i * P_i / T        offset_i(t) = (speed_i * (t mod T)) mod P_i
```

Because every layer completes whole periods in `T`, all of them line up again at `t = T`, and the whole scene loops perfectly. Parallax comes from choosing larger `P_i * n_i` for nearer layers. For example, with screen width `W`:

| Layer | Period | Turns per loop | Relative speed |
|---|---|---|---|
| sky / stars | `1 W` | 1 | 1 (slowest) |
| far peaks | `2 W` | 1 | 2 |
| mid hills | `3 W` | 1 | 3 |
| near ground (Clawd walks on it) | `3 W` | 2 | 6 |

Offsets are **integer pixels** computed from `t` with integer arithmetic, so stills, GIF frames and live frames agree exactly (PLAN.md section 12, determinism).

## Generating periodic terrain

Every noise lookup for a scrolling layer wraps its lattice: lattice cell `ix` becomes `ix mod period`. Terrain sampled at world column `c` uses `c mod P_cols`, so column `P_cols` is exactly column 0 and the strip tiles with no seam. Each fBm octave doubles both frequency and period, so wrapping holds at every octave. The primitives are `noise::value1_periodic`, `fbm1_periodic` and `fbm2_periodic_x`; tests prove exact repetition.

Props placed along a layer (trees, signposts, a cabin, a frozen lake) use world coordinates modulo that layer's period, so they also come around again.

## Clawd on a journey

- Clawd stays near a fixed screen position (a third of the way in, facing the direction of travel) while the ground moves under him. That's the `journey` motion type.
- **Feet follow the terrain**: his y is the near layer's surface height at `camera_x + screen_x`, so he climbs and descends with the hills. That height is periodic too, so the loop holds. A slight tilt on steep slopes comes from the rig's `lean`.
- **No foot sliding**: the near layer's speed equals the walk's stride speed. One walk cycle moves the ground by exactly one stride, and the loop length is a whole number of walk cycles.
- The state machine still plays variations (pausing to look at the view, waving, taking a photo), but the camera keeps moving, so pauses use poses that read as "walking slowly" or "looking around" rather than standing still.

## Rendering cost

- **Live (GPU)**:
  - each layer is a **cell grid** (glyph index + shade), not a pixel image, rendered as instanced glyph quads from the glyph atlas with wrap-around addressing;
  - a 4K near layer three screens wide is about 113k cells per layer (a few hundred KB);
  - background bands (vignette, glow) stay in screen space and are computed per pixel in the shader;
  - per frame the cost is a few draw calls with UV offsets.
- **Stills and the static fallback**: the hero frame at `t_hero`, composed on the CPU.
- **GIF export**: a full journey loop is long (20-60 s), which makes for a big GIF. Export offers the full loop at a smaller size or a short excerpt (not seamless, labelled as such).
- **Saved scenes** (PLAN.md section 9c): the frozen package stores each layer's strip (cell grid or indexed PNG) plus periods, speeds and `T`, so a saved journey plays exactly as it did.

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
| Desert caravan | dunes | mesas, heat shimmer |
| Riding a train | Clawd on the roof or at a window | fields, poles, distant towns |
| Sailing along a coast | waves, boat bob | cliffs, lighthouse, gulls |
| Hot-air balloon or broom flight (wizard) | clouds below | mountains far below, stars |
| Cycling across a city at dusk | rooftops, street | skyline, lit windows |
| Walking a forest trail in autumn | leaves falling | layered trunks |
| Night sky drift | (no ground) | stars, aurora bands |
| Running along a beach | wet sand, footprints washing away | sea, sunset |

Vertical journeys (climbing a tower, a balloon rising) use the same maths on the y axis.

## Plan

- **Wave J (with M4)**: periodic noise (done), periodic layer builder for `mountains`, `journey` motion, near-layer height sampling for the feet, integer-pixel layer offsets, CPU composition for stills and GIF; first journey: hiking a ridge.
- **M7**: GPU layer grids with wrap addressing.
- **Then**: more periodic archetypes and journeys from the table above.
