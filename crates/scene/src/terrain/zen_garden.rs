//! Zen garden: raked gravel in long straight lines that ring the rocks set
//! in it, moss at their feet, a stone lantern lit after dusk, and behind it
//! all a low tiled wall with a maple and pines leaning over it. The focal
//! group stands on a grassy bank at the garden's edge.

use super::layers::{self, Layer, put};
use super::{Ctx, family_glyph};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::clamp01;
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

/// Hills beyond the wall.
const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.22,
    amp: 0.04,
    freq: 1.1,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.52,
    shade: 0.22,
    outline: Some(0.32),
    snow: None,
    label: "zen_hills",
};

/// The wall's foot and its height, above the horizon.
const WALL: f64 = 0.17;

const ROCK_BIG: &[&str] = &[
    "    _.--._    ",
    "  _/%@@#%%\\_  ",
    " /%@##@@##@%\\ ",
    "/%@###@@###@%\\",
];
const ROCK: &[&str] = &["   _.-._   ", " _/%@#@%\\_ ", "/%@##@##@%\\"];
const ROCK_SMALL: &[&str] = &["  _._  ", " /%@%\\ ", "/%@#@%\\"];
const LANTERN: &[&str] = &[
    "  _^_  ", " /===\\ ", "  |O|  ", "  =#=  ", "   #   ", "  _#_  ",
];
const PINE: &[&str] = &["  _&_  ", " &&%&& ", "&&%@%&&", "   |   "];

fn lit(tod: TimeOfDay) -> bool {
    matches!(
        tod,
        TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
    )
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let w = co.w;
    let seed = hash_label(ctx.terrain, "zen_garden");
    let pick = |i: i64, j: i64| unit_from_hash(hash3(seed, i, j));
    let mut sky = layers::paint(ctx, cells, &[HILLS]);
    let wall_foot = layers::row_at(ctx, ctx.horizon - WALL);
    let wall_rows = ctx.scaled(5.0).max(3);
    let wall_top = wall_foot - wall_rows;
    // Nothing of the hills below the wall's top.
    for row in (wall_top - 1).max(0)..i64::from(ctx.rows) {
        for col in 0..ctx.cols {
            cells[ctx.idx(col, row as u32)] = Cell::default();
        }
    }

    // A maple behind the wall on the far side, pines along it.
    let far_side = if ctx.group_cx < w * 0.5 { 0.72 } else { 0.28 };
    let mx = w * (far_side + 0.06 * (pick(0, 1) - 0.5));
    let mcol = co.col_of(mx);
    let crown_w = ctx.scaled(32.0).max(12);
    let crown_h = ctx.scaled(16.0).max(7);
    let crown_row = wall_top - crown_h / 2;
    let leaves = hash_label(seed, "maple");
    for row in crown_row - crown_h..=crown_row + crown_h / 2 {
        for c in mcol - crown_w..=mcol + crown_w {
            let u = (c - mcol) as f64 / crown_w as f64;
            let v = (row - crown_row) as f64 / crown_h as f64;
            let r = u * u + v * v * 1.3;
            if r > 1.0 || c < 0 || c >= i64::from(ctx.cols) || row < 0 {
                continue;
            }
            let n = fbm2(leaves, c as f64 * 0.18, row as f64 * 0.3, 3);
            let d = clamp01((n - 0.5) * 2.0 + 0.72 - 0.35 * r);
            if d > 0.36 {
                let g = family_glyph(&glyphs::FOLIAGE, d, 0.36, leaves, c as u32, row as u32);
                put(ctx, cells, c, row, g, 0.55);
            }
        }
    }
    for k in 0..crown_h {
        put(
            ctx,
            cells,
            mcol + k / 4 - 1,
            crown_row + crown_h / 2 + k,
            id('|'),
            0.45,
        );
    }
    let crown_top = co.ny((crown_row - crown_h).max(0) as u32);
    for c in (mcol - crown_w).max(0)..=(mcol + crown_w).min(i64::from(ctx.cols) - 1) {
        super::shelter(ctx, &mut sky, c, crown_top);
    }
    for k in 0..4 {
        let px = w * (0.08 + 0.84 * pick(k, 2));
        let c = co.col_of(px);
        if (c - mcol).abs() < crown_w + 4 {
            continue;
        }
        layers::stamp(ctx, cells, PINE, c, wall_top - 1, 0.4);
        let y = co.ny((wall_top - 5).max(0) as u32);
        for x in (c - 4).max(0)..=(c + 4).min(i64::from(ctx.cols) - 1) {
            super::shelter(ctx, &mut sky, x, y);
        }
    }

    // The wall: tiles on top, plaster under, posts now and then.
    for c in 0..i64::from(ctx.cols) {
        put(ctx, cells, c, wall_top - 1, id('^'), 0.5);
        put(ctx, cells, c, wall_top, id('='), 0.5);
        for row in wall_top + 1..wall_foot {
            let post = c.rem_euclid(18) == 0;
            put(
                ctx,
                cells,
                c,
                row,
                if post { id('|') } else { id(':') },
                if post { 0.45 } else { 0.22 },
            );
        }
        put(ctx, cells, c, wall_foot, id('_'), 0.4);
    }
    let y = co.ny((wall_top - 1).max(0) as u32);
    for s in sky.iter_mut() {
        *s = s.min(y);
    }

    // The rocks, then the gravel raked around them.
    let mut stones: Vec<(i64, i64, f64)> = Vec::new();
    let n = 3 + (pick(0, 3) * 3.0) as i64;
    let garden_rows = (layers::row_at(ctx, ctx.horizon) - wall_foot).max(4);
    for k in 0..n {
        // Mostly on the far side from the group, spread along it.
        let from = if ctx.group_cx < w * 0.5 { 0.32 } else { 0.06 };
        let c = (f64::from(ctx.cols)
            * (from + 0.62 * (k as f64 + 0.2 + 0.6 * pick(k, 4)) / n as f64))
            as i64;
        if (co.nx(c.clamp(0, i64::from(ctx.cols) - 1) as u32) * w - ctx.group_cx).abs() < w * 0.12 {
            continue;
        }
        let row = wall_foot + 2 + (garden_rows as f64 * (0.3 + 0.6 * pick(k, 5))) as i64;
        let size = pick(k, 6);
        stones.push((
            c,
            row,
            if size > 0.6 {
                1.0
            } else if size > 0.25 {
                0.75
            } else {
                0.55
            },
        ));
    }
    let rake = hash_label(seed, "rake");
    for row in wall_foot + 1..i64::from(ctx.rows) {
        for c in 0..i64::from(ctx.cols) {
            // Rings around the nearest rock, straight lines elsewhere.
            let mut ring = None;
            for &(sc, sr, s) in &stones {
                let dx = (c - sc) as f64 / (18.0 * s);
                let dy = (row - sr) as f64 / (5.0 * s);
                let d = (dx * dx + dy * dy).sqrt();
                if d < 1.6 {
                    ring = Some(ring.map_or(d, |r: f64| r.min(d)));
                }
            }
            let glyph = match ring {
                Some(d) if d < 0.45 => continue,
                Some(d) => {
                    if ((d - 0.45) * 7.0).fract() < 0.28 {
                        if unit_from_hash(hash3(rake, c, row)) < 0.18 {
                            id('.')
                        } else {
                            id('-')
                        }
                    } else {
                        continue;
                    }
                }
                None if row % 2 == 0 => id('-'),
                None => continue,
            };
            put(ctx, cells, c, row, glyph, 0.38);
        }
    }
    for &(c, row, s) in &stones {
        let shape = if s > 0.9 {
            ROCK_BIG
        } else if s > 0.6 {
            ROCK
        } else {
            ROCK_SMALL
        };
        layers::stamp(ctx, cells, shape, c, row, 0.62);
        // Moss at the foot.
        let half = shape[shape.len() - 1].len() as i64 / 2 + 1;
        for x in c - half..=c + half {
            if unit_from_hash(hash3(rake, x, row + 1)) < 0.5 {
                put(ctx, cells, x, row + 1, id(','), 0.4);
            }
        }
    }

    // The stone lantern, near the group.
    let side = if ctx.group_cx < w * 0.5 { 1.0 } else { -1.0 };
    let lx = ctx.group_cx + side * w * (0.16 + 0.05 * pick(0, 7));
    let lcol = co.col_of(lx);
    let lrow = wall_foot + garden_rows / 2;
    layers::stamp(ctx, cells, LANTERN, lcol, lrow, 0.55);
    let glow = if lit(ctx.tod) { 1.0 } else { 0.55 };
    put(ctx, cells, lcol, lrow - 3, id('O'), glow);
    sky
}
