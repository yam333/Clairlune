//! Viaduct: a tall stone railway bridge of semicircular arches spanning a
//! wooded valley, hills faint behind it, and a short train of lit carriages
//! crossing along its deck with a trail of steam. The focal group stands on a
//! grassy bank on this side of the valley.

use super::Ctx;
use super::layers::{self, Layer, put};
use crate::glyphs::{self, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::recipe::TimeOfDay;

const FAR_HILLS: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.22,
    amp: 0.05,
    freq: 1.2,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.52,
    shade: 0.22,
    outline: Some(0.32),
    snow: None,
    label: "viaduct_hills",
};

const VALLEY_SLOPES: Layer = Layer {
    family: &glyphs::RIDGE,
    hatch: None,
    base: 0.01,
    amp: 0.04,
    freq: 1.6,
    ridged: false,
    plateau: None,
    fill: (18.0, 24.0),
    threshold: 0.48,
    shade: 0.36,
    outline: Some(0.46),
    snow: None,
    label: "viaduct_valley",
};

const PINE: &[&str] = &["  ^  ", " /%\\ ", "/%@%\\"];

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
    let seed = hash_label(ctx.terrain, "viaduct");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x71AD));
    let evening = lit(ctx.tod);
    let mut sky = layers::paint(ctx, cells, &[FAR_HILLS]);

    // Deck level and valley floor in rows.
    let deck_ny = ctx.horizon - (0.14 + 0.04 * pick(1));
    let deck_row = layers::row_at(ctx, deck_ny);
    let valley_row = layers::row_at(ctx, ctx.horizon + 0.02);
    let span_h = (valley_row - deck_row).max(6);

    // Arch geometry: piers spaced evenly across the screen, semicircular openings
    // carved between them (cells are roughly twice as tall as wide).
    let span_cols = ctx.scaled(14.0 + 4.0 * pick(2)).max(9);
    let pier_half = ctx.scaled(1.6).max(1);
    let arch_r = ((span_cols - 2 * pier_half) / 2).max(3);
    let arch_rise = (arch_r / 2).max(2).min(span_h - 2);
    let spring_row = deck_row + 2 + arch_rise;
    let offset = (pick(3) * span_cols as f64) as i64;
    let masonry = hash_label(seed, "masonry");

    for col in 0..i64::from(ctx.cols) {
        let local = (col + offset).rem_euclid(span_cols);
        let centre = span_cols / 2;
        let dx = (local - centre) as f64 / arch_r.max(1) as f64;
        // Parapet and cornice along the deck.
        put(ctx, cells, col, deck_row - 1, id('_'), 0.52);
        put(ctx, cells, col, deck_row, id('='), 0.58);
        for row in deck_row + 1..=valley_row {
            let inside_opening = if dx.abs() < 1.0 && row >= spring_row - arch_rise {
                if row >= spring_row {
                    true
                } else {
                    let dy = (spring_row - row) as f64 / arch_rise.max(1) as f64;
                    dx * dx + dy * dy < 0.96
                }
            } else {
                false
            };
            if inside_opening {
                continue;
            }
            // Arch ring outline right at the curve, coursed stone elsewhere.
            let on_ring =
                dx.abs() <= 1.05 && row <= spring_row && row >= spring_row - arch_rise - 1 && {
                    let dy = (spring_row - row) as f64 / arch_rise.max(1) as f64;
                    let r2 = dx * dx + dy * dy;
                    (0.96..=1.38).contains(&r2)
                };
            let on_pier_edge =
                row > spring_row && (local == pier_half || local == span_cols - 1 - pier_half);
            let glyph = if on_ring {
                if dx < -0.35 {
                    id('/')
                } else if dx > 0.35 {
                    id('\\')
                } else {
                    id('_')
                }
            } else if on_pier_edge {
                id('|')
            } else {
                let k = hash3(masonry, col, row);
                if (row + (col / 3)) % 2 == 0 {
                    if k & 3 == 0 { id(':') } else { id('=') }
                } else if k & 1 == 0 {
                    id('#')
                } else {
                    id('+')
                }
            };
            put(
                ctx,
                cells,
                col,
                row,
                glyph,
                if on_ring || on_pier_edge { 0.54 } else { 0.42 },
            );
        }
        let top_y = co.ny((deck_row - 1).max(0) as u32) - 0.5 / rows;
        super::shelter(ctx, &mut sky, col, top_y);
    }

    // A short train crossing the far side of the deck, with puffs of steam.
    let far_nx = if ctx.group_cx < w * 0.5 {
        0.64 + 0.1 * pick(4)
    } else {
        0.26 + 0.1 * pick(4)
    };
    let dir = if pick(5) < 0.5 { 1i64 } else { -1 };
    let head = co.col_of(far_nx * w);
    let train_row = deck_row - 2;
    // Engine.
    for dx in 0..5 {
        put(ctx, cells, head - dir * dx, train_row, id('#'), 0.62);
    }
    put(ctx, cells, head, train_row - 1, id('|'), 0.58);
    // Steam trailing back from the funnel.
    for s in 1..=5 {
        let sx = head - dir * (s * 3);
        let sy = train_row - 2 - s / 2;
        put(
            ctx,
            cells,
            sx,
            sy,
            if s <= 2 { id('o') } else { id('.') },
            0.46 - 0.05 * s as f64,
        );
    }
    // Three carriages with windows glowing warm after golden hour.
    for car in 0..3 {
        let start = head - dir * (7 + car * 8);
        for dx in 0..6 {
            let c = start - dir * dx;
            let win = dx == 1 || dx == 3 || dx == 4;
            put(
                ctx,
                cells,
                c,
                train_row,
                if win {
                    if evening { id('o') } else { id(':') }
                } else {
                    id('=')
                },
                if win && evening { 0.96 } else { 0.55 },
            );
            put(ctx, cells, c, train_row - 1, id('_'), 0.5);
        }
    }
    let train_top = co.ny((train_row - 4).max(0) as u32);
    for c in (head - 34).max(0)..=(head + 34).min(i64::from(ctx.cols) - 1) {
        super::shelter(ctx, &mut sky, c, train_top);
    }

    // Near valley slopes and scattered pines along their crest.
    let slopes = layers::paint(ctx, cells, &[VALLEY_SLOPES]);
    layers::stamp_along(
        ctx,
        cells,
        &VALLEY_SLOPES,
        &[PINE],
        11,
        0.48,
        "viaduct_pines",
    );
    for (s, n) in sky.iter_mut().zip(slopes) {
        *s = s.min(n);
    }
    sky
}
