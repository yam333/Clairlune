//! Lighthouse: a headland across a bay, a striped lighthouse near its cliff
//! edge and the keeper's cottage beside it, the sea between with rocks and
//! foam at the cliff's foot. After dusk the lamp is lit, its beam swings out
//! over the water and a road of light lies under it. The focal group stands
//! on a rock ledge on this side.

use super::layers::{self, Layer, put};
use super::{Ctx, family_glyph, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, smoothstep};
use crate::noise::{fbm1, fbm2};
use crate::recipe::TimeOfDay;

/// The far coast, low along the horizon.
const COAST: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.085,
    amp: 0.015,
    freq: 1.1,
    ridged: false,
    plateau: None,
    fill: (26.0, 18.0),
    threshold: 0.5,
    shade: 0.24,
    outline: Some(0.36),
    snow: None,
    label: "lighthouse_coast",
};

/// Where the sea starts, above the horizon (the platform line).
const SEA: f64 = 0.07;

/// The headland's rock, light to heavy.
const ROCK: [u8; 5] = [id('.'), id(':'), id('x'), id('%'), id('#')];
/// Its turf.
const TURF: [u8; 3] = [id(','), id('"'), id('\'')];

const COTTAGE: &[&str] = &["  _^_  ", " /===\\ ", "/=====\\", " |#o#| ", " |#_#| "];

/// Where the sea starts (normalised y).
pub(crate) fn sea_top(ctx: &Ctx) -> f64 {
    ctx.horizon - SEA
}

fn lit(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "lighthouse");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x11));
    let night = lit(ctx.tod);
    let mut sky = layers::paint(ctx, cells, &[COAST]);
    let sea_top = sea_top(ctx);

    // The sea: swell lines, sparser and fainter far off.
    let waves = hash_label(seed, "waves");
    for row in 0..ctx.rows {
        let ny = co.ny(row);
        if ny < sea_top {
            continue;
        }
        let near = clamp01((ny - sea_top) / (1.0 - sea_top));
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row);
            if (ny - sea_top) * rows < 1.0 {
                cells[idx] = Cell {
                    glyph: id('-'),
                    shade: shade_u8(0.34),
                };
                continue;
            }
            let n = fbm2(waves, co.nx(col) * ctx.aspect * 9.0, ny * 70.0, 3);
            let glyph = if n > 0.7 {
                id('~')
            } else if n > 0.62 {
                id('-')
            } else if n > 0.58 - 0.05 * near {
                id('.')
            } else {
                EMPTY
            };
            cells[idx] = Cell {
                glyph,
                shade: shade_u8(0.22 + 0.2 * near),
            };
        }
    }

    // The headland, on the far side from the group: a cliff face toward the
    // bay, rising to turf, running off the edge of the screen.
    let toward_edge = if ctx.group_cx < w * 0.5 { 1.0 } else { -1.0 };
    let face = if toward_edge > 0.0 {
        w * (0.6 + 0.08 * pick(1))
    } else {
        w * (0.32 + 0.08 * pick(1))
    };
    let span = if toward_edge > 0.0 { w - face } else { face };
    let land_top = sea_top - (0.1 + 0.05 * pick(2));
    let wobble = hash_label(seed, "headland");
    let top_of = |px: f64| -> Option<f64> {
        let u = (px - face) * toward_edge / span;
        if u < 0.0 {
            return None;
        }
        let rise = smoothstep(0.0, 0.05, u);
        let roll = (fbm1(wobble, u * 6.0, 3) - 0.5) * 0.03;
        Some(sea_top - (sea_top - land_top) * rise + roll * rise)
    };
    let rock = hash_label(seed, "rock");
    for col in 0..ctx.cols {
        let px = co.nx(col) * w;
        let Some(t) = top_of(px) else {
            continue;
        };
        if t >= sea_top {
            continue;
        }
        sky[col as usize] = sky[col as usize].min(t);
        let u = (px - face) * toward_edge / span;
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            if ny < t {
                continue;
            }
            if ny > sea_top + 0.01 {
                break;
            }
            let below = (ny - t) * rows;
            let idx = ctx.idx(col, row);
            if below < 1.0 {
                cells[idx] = Cell {
                    glyph: id('_'),
                    shade: shade_u8(0.55),
                };
                continue;
            }
            if below < 3.0 && u > 0.04 {
                let k = hash3(rock, i64::from(col), i64::from(row));
                cells[idx] = Cell {
                    glyph: TURF[(k % TURF.len() as u64) as usize],
                    shade: shade_u8(0.48),
                };
                continue;
            }
            // The face itself: hatched down.
            if u < 0.025 {
                cells[idx] = Cell {
                    glyph: id('|'),
                    shade: shade_u8(0.5),
                };
                continue;
            }
            let n = fbm2(rock, px / co.h * 16.0, ny * 24.0, 3);
            let d = clamp01((n - 0.5) * 1.9 + 0.55);
            cells[idx] = if d < 0.36 {
                Cell::default()
            } else {
                Cell {
                    glyph: family_glyph(&ROCK, d, 0.36, rock, col, row),
                    shade: shade_u8(0.4),
                }
            };
        }
    }
    // Rocks and foam at the cliff's foot.
    let face_col = co.col_of(face);
    for i in 0..5 {
        let k = hash3(seed, i, 0xF0A);
        let c = face_col - (toward_edge as i64) * (2 + (k % 14) as i64);
        let r = layers::row_at(ctx, sea_top) + 1 + ((k >> 8) % 3) as i64;
        put(
            ctx,
            cells,
            c,
            r,
            if k & 1 == 0 { id('@') } else { id('%') },
            0.45,
        );
        put(ctx, cells, c - 1, r, id('~'), 0.5);
        put(ctx, cells, c + 1, r, id('~'), 0.5);
    }

    // The lighthouse, a little in from the cliff edge.
    let lx = face + toward_edge * span * (0.12 + 0.06 * pick(3));
    let lcol = co.col_of(lx);
    let ground = top_of(lx).unwrap_or(land_top);
    let base = layers::row_at(ctx, ground);
    let height = ctx.scaled(20.0 + 8.0 * pick(4)).max(10);
    let band = ctx.scaled(4.0).max(2);
    for i in 0..height {
        let row = base - i;
        let half = if i < height / 2 { 3 } else { 2 };
        let edge_l = if i == height / 2 { id('/') } else { id('|') };
        let edge_r = if i == height / 2 { id('\\') } else { id('|') };
        put(ctx, cells, lcol - half, row, edge_l, 0.6);
        put(ctx, cells, lcol + half, row, edge_r, 0.55);
        let stripe = if (i / band) % 2 == 0 {
            id('#')
        } else {
            id('=')
        };
        for x in lcol - half + 1..lcol + half {
            // A door at the foot, a window halfway up.
            let glyph = if (i < 2 && x == lcol) || (i == height * 2 / 3 && x == lcol) {
                id('|')
            } else {
                stripe
            };
            put(ctx, cells, x, row, glyph, 0.5);
        }
    }
    // The gallery, the lamp room and the roof.
    let gallery = base - height;
    for x in lcol - 4..=lcol + 4 {
        put(ctx, cells, x, gallery, id('='), 0.6);
    }
    let lamp_row = gallery - 1;
    put(ctx, cells, lcol - 2, lamp_row, id('|'), 0.6);
    put(ctx, cells, lcol + 2, lamp_row, id('|'), 0.6);
    put(ctx, cells, lcol - 1, lamp_row, id(':'), 0.6);
    put(ctx, cells, lcol + 1, lamp_row, id(':'), 0.6);
    put(
        ctx,
        cells,
        lcol,
        lamp_row,
        id('*'),
        if night { 1.0 } else { 0.62 },
    );
    put(ctx, cells, lcol - 2, lamp_row - 1, id('/'), 0.6);
    put(ctx, cells, lcol + 2, lamp_row - 1, id('\\'), 0.6);
    for x in lcol - 1..=lcol + 1 {
        put(ctx, cells, x, lamp_row - 1, id('^'), 0.55);
    }
    put(ctx, cells, lcol, lamp_row - 2, id('^'), 0.6);
    let top = co.ny((lamp_row - 2).max(0) as u32) - 0.5 / rows;
    for c in (lcol - 5).max(0)..=(lcol + 5).min(i64::from(ctx.cols) - 1) {
        super::shelter(ctx, &mut sky, c, top);
    }

    // The keeper's cottage, further along the headland.
    let hx = lx + toward_edge * w * 0.06;
    let hcol = co.col_of(hx);
    let hrow = layers::row_at(ctx, top_of(hx).unwrap_or(ground));
    layers::stamp(ctx, cells, COTTAGE, hcol, hrow, 0.5);
    if night {
        put(ctx, cells, hcol, hrow - 1, id('o'), 0.95);
    }
    let roof = co.ny((hrow - COTTAGE.len() as i64).max(0) as u32);
    for c in (hcol - 4).max(0)..=(hcol + 4).min(i64::from(ctx.cols) - 1) {
        super::shelter(ctx, &mut sky, c, roof);
    }

    if night {
        // The beam: a wedge out over the bay, widening and fading.
        let reach = (w * 0.4 / f64::from(co.g.cell_w)) as i64;
        let dir = -(toward_edge as i64);
        let beam = hash_label(seed, "beam");
        let fall = 0.1 + 0.08 * pick(5);
        for d in 2..reach {
            let t = d as f64 / reach as f64;
            let fade = 1.0 - 0.65 * t;
            let (a, b) = (
                (lamp_row as f64 - d as f64 * 0.03).round() as i64,
                (lamp_row as f64 + d as f64 * fall).round() as i64,
            );
            let c = lcol + dir * d;
            // Above the ground: below it the layout is the same by day.
            let b = b.min(i64::from(ctx.platform_row));
            put(
                ctx,
                cells,
                c,
                a,
                if d < 6 { id('=') } else { id('-') },
                fade,
            );
            put(ctx, cells, c, b, id('-'), fade * 0.85);
            for r in a + 1..b {
                if unit_from_hash(hash3(beam, c, r)) < 0.55 * (1.0 - t) {
                    put(ctx, cells, c, r, id('.'), fade * 0.75);
                }
            }
        }
    }
    // The lamp's road of light on the water (a glint by day). Drawn in
    // both themes: below the ground the layout is the same in each.
    let road = hash_label(seed, "road");
    for row in layers::row_at(ctx, sea_top) + 1..i64::from(ctx.rows) {
        let k = hash3(road, row, 1);
        let jitter = (k % 3) as i64 - 1;
        if unit_from_hash(k) < 0.7 {
            put(
                ctx,
                cells,
                lcol + jitter,
                row,
                if k & 4 == 0 { id(':') } else { id('|') },
                if night { 0.75 } else { 0.3 },
            );
        }
    }
    sky
}
