//! The great tree: a colossal old tree on the far side, its canopy spread
//! across the sky, roots coiling over the ground, a round door and a lit
//! window in its trunk and lanterns hanging from its boughs; the focal
//! group stands on a grassy bank.

use super::layers::{self, Layer, put};
use super::{Ctx, family_glyph};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::clamp01;
use crate::noise::{fbm1, fbm2};

const HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.1,
    amp: 0.05,
    freq: 1.3,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.5,
    shade: 0.26,
    outline: Some(0.4),
    snow: None,
    label: "tree_hills",
};

/// Grass, light to heavy.
const GRASS: [u8; 4] = [id('.'), id(','), id('\''), id('"')];

const NEAR: Layer = Layer {
    family: &GRASS,
    hatch: None,
    base: 0.11,
    amp: 0.03,
    freq: 2.4,
    ridged: false,
    plateau: None,
    fill: (16.0, 22.0),
    threshold: 0.42,
    shade: 0.6,
    outline: Some(0.7),
    snow: None,
    label: "tree_near",
};

/// Bark, light to heavy: grooves running up the trunk.
const BARK: [u8; 4] = [id(':'), id('|'), id('!'), id('#')];

const DOOR: [&str; 5] = [" __ ", "/  \\", "|  |", "| o|", "|__|"];

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let co = &ctx.co;
    let (w, h) = (co.w, co.h);
    let rows = f64::from(ctx.rows);
    let seed = hash_label(ctx.terrain, "great_tree");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x7EE));
    let mut sky = layers::paint(ctx, cells, &[HILLS]);
    let lit = !matches!(
        ctx.tod,
        crate::recipe::TimeOfDay::Morning | crate::recipe::TimeOfDay::Noon
    );

    // The trunk, on the far side, standing on the ground line.
    let group_left = ctx.group_cx < w * 0.5;
    let tx = if group_left {
        w * (0.72 + 0.12 * pick(1))
    } else {
        w * (0.16 + 0.12 * pick(1))
    };
    let half = w * (0.035 + 0.015 * pick(2));
    let ground = ctx.horizon + 0.01;
    let crown_y = 0.22 + 0.06 * pick(3);
    let bark = hash_label(seed, "bark");
    for row in 0..ctx.rows {
        let ny = co.ny(row);
        if ny < crown_y - 0.06 || ny > ground + 0.02 {
            continue;
        }
        // The trunk flares toward its foot, and a little up into the crown.
        let foot = clamp01((ny - (ground - 0.12)) / 0.12);
        let top = clamp01((crown_y + 0.04 - ny) / 0.1);
        let wobble = (fbm1(bark, ny * 9.0, 2) - 0.5) * half * 0.3;
        let hw = half * (1.0 + 1.6 * foot * foot + 0.8 * top);
        for col in 0..ctx.cols {
            let px = co.nx(col) * w - tx - wobble;
            if px.abs() > hw {
                continue;
            }
            let u = px / hw;
            let glyph = if u.abs() > 0.9 {
                id('|')
            } else {
                let n = fbm2(bark, (px / h) * 60.0, ny * 6.0, 3);
                family_glyph(&BARK, clamp01((n - 0.5) * 1.8 + 0.5), 0.0, bark, col, row)
            };
            put(
                ctx,
                cells,
                i64::from(col),
                i64::from(row),
                glyph,
                0.4 + 0.1 * u.abs(),
            );
        }
    }
    // Roots coiling out over the ground, both ways.
    for side in [-1.0, 1.0] {
        for r in 0..3 {
            let reach = w * (0.05 + 0.05 * pick(10 + r)) * if r == 0 { 1.6 } else { 1.0 };
            let start = tx + side * half * (1.2 + 0.4 * f64::from(r as i32));
            let steps = (reach / f64::from(co.g.cell_w)).round() as i64;
            for s in 0..steps {
                let x = start + side * f64::from(s as i32) * f64::from(co.g.cell_w);
                let t = f64::from(s as i32) / steps.max(1) as f64;
                let y = ground - 0.02 + 0.03 * t + 0.008 * f64::from(r as i32);
                let col = co.col_of(x);
                let row = layers::row_at(ctx, y);
                let glyph = if t < 0.15 {
                    if side < 0.0 { id('/') } else { id('\\') }
                } else if s % 3 == 0 {
                    id('~')
                } else {
                    id('_')
                };
                put(ctx, cells, col, row, glyph, 0.44);
            }
        }
    }
    // The door at its foot, a lit window above.
    let door_col = co.col_of(tx);
    let door_row = layers::row_at(ctx, ground) - 1;
    layers::stamp(ctx, cells, &DOOR, door_col, door_row, 0.62);
    let win_row = door_row - ctx.scaled(9.0 + 4.0 * pick(4)).max(6);
    let win_col = door_col + if pick(5) < 0.5 { -2 } else { 2 };
    put(
        ctx,
        cells,
        win_col,
        win_row,
        id('O'),
        if lit { 0.95 } else { 0.55 },
    );

    // The crown: a great canopy, ragged at the edges, reaching off the top.
    let leaves = hash_label(seed, "leaves");
    let span = w * (0.3 + 0.08 * pick(6));
    let depth = crown_y + 0.04;
    for row in 0..ctx.rows {
        let ny = co.ny(row);
        if ny > depth + 0.03 {
            break;
        }
        for col in 0..ctx.cols {
            let px = co.nx(col) * w - tx;
            let u = px / span;
            if u.abs() > 1.2 {
                continue;
            }
            // Thicker over the trunk and toward the top; the lower edge
            // hangs in clumps.
            let hang = depth - 0.12 * u * u + 0.05 * (fbm1(leaves, co.nx(col) * 14.0, 2) - 0.5);
            if ny > hang {
                continue;
            }
            let n = fbm2(leaves, co.nx(col) * ctx.aspect * 18.0, ny * 18.0, 3);
            let d = clamp01((n - 0.5) * 2.0 + 0.5) - 0.25 * (u.abs() - 0.7).max(0.0) * 2.0;
            if d < 0.42 {
                continue;
            }
            let glyph = family_glyph(&glyphs::FOLIAGE, d, 0.42, leaves, col, row);
            put(
                ctx,
                cells,
                i64::from(col),
                i64::from(row),
                glyph,
                0.34 + 0.12 * d,
            );
            let s = &mut sky[col as usize];
            *s = s.min(ny - 0.5 / rows);
        }
    }
    // Boughs: a few heavy limbs from the trunk out into the crown.
    for b in 0..4 {
        let side = if b % 2 == 0 { -1.0 } else { 1.0 };
        let y0 = crown_y + 0.02 - 0.03 * f64::from(b as i32 / 2);
        let len = span * (0.45 + 0.3 * pick(20 + b));
        let steps = (len / f64::from(co.g.cell_w)).round() as i64;
        for s in 0..steps {
            let x = tx + side * (half + f64::from(s as i32) * f64::from(co.g.cell_w));
            let y = y0 - 0.06 * f64::from(s as i32) / steps.max(1) as f64;
            put(
                ctx,
                cells,
                co.col_of(x),
                layers::row_at(ctx, y),
                if side < 0.0 { id('\\') } else { id('/') },
                0.42,
            );
        }
    }
    // Lanterns hanging from the crown's lower edge on threads.
    let lanterns = hash_label(seed, "lanterns");
    let mut col = co.col_of(tx - span);
    let end = co.col_of(tx + span);
    let g0 = co.col_of(f64::from(ctx.gx0)) - 2;
    let g1 = co.col_of(f64::from(ctx.gx1)) + 2;
    while col < end {
        let k = hash3(lanterns, col, 5);
        let x = co.nx(col.max(0) as u32) * w;
        if col > 0
            && col < i64::from(ctx.cols) - 1
            && (x - tx).abs() > half * 1.6
            && (col < g0 || col > g1)
            && unit_from_hash(k) < 0.45
        {
            // Just under the lowest leaves in this column.
            let mut row = 0i64;
            for r in 0..i64::from(ctx.platform_row) {
                if co.ny(r as u32) > depth + 0.03 {
                    break;
                }
                if cells[ctx.idx(col as u32, r as u32)].glyph != EMPTY {
                    row = r + 1;
                }
            }
            let thread = 2 + (k >> 40) as i64 % 4;
            if row > 2 && row + thread < i64::from(ctx.platform_row) - 2 {
                for r in row..row + thread {
                    put(ctx, cells, col, r, id('|'), 0.3);
                }
                put(
                    ctx,
                    cells,
                    col,
                    row + thread,
                    id('o'),
                    if lit { 0.95 } else { 0.6 },
                );
                if lit {
                    put(ctx, cells, col + 1, row + thread, id('\''), 0.6);
                }
            }
        }
        col += 3 + (k >> 50) as i64 % 4;
    }

    let near = layers::paint(ctx, cells, &[NEAR]);
    for (s, n) in sky.iter_mut().zip(near) {
        *s = s.min(n);
    }
    sky
}
