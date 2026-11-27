//! Fjord: sheer glacier-carved cliffs rising on both sides of a deep, still
//! inlet, snow-dusted peaks far at its head, a narrow white cascade tumbling
//! down the far cliff into spray at the water's edge, and a small boathouse
//! at the cliff foot whose window lights up after dusk. The focal group stands
//! on a rock ledge on this side.

use super::layers::{self, Layer, Snow, put};
use super::{Ctx, family_glyph, shade_u8};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, smoothstep};
use crate::noise::{fbm1, fbm2};
use crate::recipe::TimeOfDay;

const SNOWCAP: [u8; 3] = [id('.'), id('\''), id(':')];

/// Peaks at the head of the fjord.
const HEAD_PEAKS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: Some((&glyphs::FAR_LEFT, &glyphs::FAR_RIGHT)),
    base: -0.18,
    amp: 0.08,
    freq: 1.7,
    ridged: true,
    plateau: None,
    fill: (24.0, 16.0),
    threshold: 0.5,
    shade: 0.25,
    outline: Some(0.4),
    snow: Some(Snow {
        line: 0.2,
        rows: 2.5,
        glyphs: &SNOWCAP,
        shade: 0.52,
    }),
    label: "fjord_peaks",
};

const WATER: f64 = 0.065;
const ROCK: [u8; 5] = [id('.'), id(':'), id('|'), id('x'), id('#')];
const BOATHOUSE: &[&str] = &["  /^\\  ", " /===\\ ", " |#o#| ", " ===== "];

/// Where the fjord's open water starts (normalised y).
pub(crate) fn sea_top(ctx: &Ctx) -> f64 {
    ctx.horizon - WATER
}

fn lit(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::GoldenHour | TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "fjord");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0xF10D));
    let evening = lit(ctx.tod);
    let mut sky = layers::paint(ctx, cells, &[HEAD_PEAKS]);
    let water_top = sea_top(ctx);
    let water_row = layers::row_at(ctx, water_top);

    // Clear the distant peaks below the waterline so the inlet sits clean.
    for row in water_row.max(0)..i64::from(ctx.rows) {
        for col in 0..ctx.cols {
            cells[ctx.idx(col, row as u32)] = Cell::default();
        }
    }

    // Two steep walls framing the open inlet, the taller wall on the far side
    // from the focal group so the cascade and boathouse sit in clear view.
    let far_right = ctx.group_cx < w * 0.5;
    let (left_span, right_span) = if far_right {
        (0.24 + 0.05 * pick(1), 0.42 + 0.06 * pick(2))
    } else {
        (0.42 + 0.06 * pick(2), 0.24 + 0.05 * pick(1))
    };
    let left_high = water_top - (0.28 + 0.08 * pick(3));
    let right_high = water_top - (0.32 + 0.08 * pick(4));
    let wall_seed = hash_label(seed, "walls");

    let wall_top = |nx: f64| -> Option<(f64, bool)> {
        if nx < left_span {
            let u = 1.0 - nx / left_span;
            let rise = smoothstep(0.0, 0.28, u);
            let jag = (fbm1(wall_seed, nx * 9.0, 3) - 0.5) * 0.035;
            Some((
                water_top - (water_top - left_high) * rise + jag * rise,
                true,
            ))
        } else if nx > 1.0 - right_span {
            let u = (nx - (1.0 - right_span)) / right_span;
            let rise = smoothstep(0.0, 0.28, u);
            let jag = (fbm1(wall_seed ^ 0x99, nx * 9.0, 3) - 0.5) * 0.035;
            Some((
                water_top - (water_top - right_high) * rise + jag * rise,
                false,
            ))
        } else {
            None
        }
    };

    let rock_seed = hash_label(seed, "cliff_rock");
    for col in 0..ctx.cols {
        let nx = co.nx(col);
        let Some((t, is_left)) = wall_top(nx) else {
            continue;
        };
        if t >= water_top {
            continue;
        }
        sky[col as usize] = sky[col as usize].min(t);
        for row in 0..ctx.rows {
            let ny = co.ny(row);
            if ny < t {
                continue;
            }
            if ny > water_top {
                break;
            }
            let below = (ny - t) * rows;
            let idx = ctx.idx(col, row);
            if below < 1.0 {
                cells[idx] = Cell {
                    glyph: if is_left { id('\\') } else { id('/') },
                    shade: shade_u8(0.54),
                };
                continue;
            }
            if below < 2.2 && ny < water_top - 0.22 {
                let k = hash3(rock_seed, i64::from(col), i64::from(row));
                cells[idx] = Cell {
                    glyph: SNOWCAP[(k % 3) as usize],
                    shade: shade_u8(0.56),
                };
                continue;
            }
            let n = fbm2(rock_seed, nx * ctx.aspect * 14.0, ny * 28.0, 3);
            let d = clamp01((n - 0.5) * 1.9 + 0.54);
            cells[idx] = if d < 0.36 {
                Cell::default()
            } else {
                Cell {
                    glyph: family_glyph(&ROCK, d, 0.36, rock_seed, col, row),
                    shade: shade_u8(0.42),
                }
            };
        }
    }

    // A narrow cascade down the far wall, with mist at its foot.
    let fall_nx = if far_right {
        1.0 - right_span * (0.52 + 0.14 * pick(5))
    } else {
        left_span * (0.52 + 0.14 * pick(5))
    };
    let fcol = co.col_of(fall_nx * w);
    if let Some((lip, _)) = wall_top(fall_nx) {
        let top_r = layers::row_at(ctx, lip) + 1;
        for r in top_r..water_row {
            let k = hash3(seed, fcol, r);
            put(
                ctx,
                cells,
                fcol,
                r,
                if k & 1 == 0 { id('|') } else { id(':') },
                0.72,
            );
            if k & 3 == 0 {
                put(
                    ctx,
                    cells,
                    fcol + if far_right { -1 } else { 1 },
                    r,
                    id('.'),
                    0.45,
                );
            }
        }
        for dx in -2..=2 {
            put(ctx, cells, fcol + dx, water_row, id('~'), 0.62);
            if dx.abs() <= 1 {
                put(ctx, cells, fcol + dx, water_row - 1, id('.'), 0.48);
            }
        }
    }

    // A small boathouse tucked near the inner foot of the far cliff.
    let bh_nx = if far_right {
        1.0 - right_span * 0.22
    } else {
        left_span * 0.22
    };
    let bcol = co.col_of(bh_nx * w);
    layers::stamp(ctx, cells, BOATHOUSE, bcol, water_row, 0.5);
    if evening {
        put(ctx, cells, bcol, water_row - 1, id('o'), 0.95);
    }

    // Still dark fjord water mirroring the cliffs and cascade, broken by ripples.
    let above: Vec<Cell> = cells.to_vec();
    let ripples = hash_label(seed, "ripples");
    for c in 0..i64::from(ctx.cols) {
        if cells[ctx.idx(c as u32, water_row.max(0) as u32)].glyph == EMPTY {
            put(ctx, cells, c, water_row, id('-'), 0.34);
        }
    }
    let first = (water_row + 1).max(0);
    for row in first..i64::from(ctx.rows) {
        let depth = row - first;
        let fade = 1.0 - (depth as f64 / (rows - first as f64).max(1.0)).min(1.0);
        for col in 0..ctx.cols {
            let idx = ctx.idx(col, row as u32);
            let n = fbm2(ripples, co.nx(col) * 34.0, row as f64 * 0.75, 2);
            let src_row = water_row - 1 - depth;
            let mirrored = if src_row >= 0 {
                above[ctx.idx(col, src_row as u32)]
            } else {
                Cell::default()
            };
            cells[idx] = if mirrored.glyph != EMPTY && n < 0.68 {
                let bright = f64::from(mirrored.shade) / 255.0 > 0.8;
                Cell {
                    glyph: if bright { id(':') } else { mirrored.glyph },
                    shade: shade_u8(f64::from(mirrored.shade) / 255.0 * (0.3 + 0.32 * fade)),
                }
            } else if n > 0.67 {
                Cell {
                    glyph: id('~'),
                    shade: shade_u8(0.22 + 0.12 * (1.0 - fade)),
                }
            } else {
                Cell::default()
            };
        }
    }
    sky
}
