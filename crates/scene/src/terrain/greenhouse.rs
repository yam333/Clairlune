//! Glasshouse: a vaulted iron-and-glass conservatory with a central dome and
//! arched wings across a stone terrace, palms and ferns showing through the
//! panes, and warm hanging lamps glowing inside after dusk. The focal group
//! stands on a grassy bank in the garden before it.

use super::Ctx;
use super::layers::{self, Layer, put};
use crate::glyphs::{self, EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::noise::fbm2;
use crate::recipe::TimeOfDay;

const GARDEN_TREES: Layer = Layer {
    family: &glyphs::DISTANT,
    hatch: None,
    base: -0.2,
    amp: 0.04,
    freq: 1.5,
    ridged: false,
    plateau: None,
    fill: (24.0, 18.0),
    threshold: 0.5,
    shade: 0.22,
    outline: Some(0.32),
    snow: None,
    label: "greenhouse_trees",
};

const PALM: &[&str] = &[" \\|/", "--*--", "  |  ", "  |  "];
const FERN: &[&str] = &[" \\|/ ", "\\\\|//"];

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
    let seed = hash_label(ctx.terrain, "greenhouse");
    let pick = |i: i64| unit_from_hash(hash3(seed, i, 0x61A5));
    let evening = lit(ctx.tod);
    let mut sky = layers::paint(ctx, cells, &[GARDEN_TREES]);

    // Position the conservatory off-centre on the far side from the focal group.
    let far = if ctx.group_cx < w * 0.5 { 0.66 } else { 0.34 };
    let cx = co.col_of(w * (far + 0.05 * (pick(1) - 0.5)));
    let foot_row = layers::row_at(ctx, ctx.horizon - 0.08);
    let wing_half = ctx.scaled(26.0 + 5.0 * pick(2)).max(14);
    let nave_half = ctx.scaled(11.0).max(6);
    let wall_h = ctx.scaled(8.0).max(4);
    let dome_h = ctx.scaled(9.0).max(5);
    let eave_row = foot_row - wall_h;

    // Clear distant trees inside and below the terrace line.
    for r in foot_row.max(0)..i64::from(ctx.rows) {
        for c in 0..ctx.cols {
            cells[ctx.idx(c, r as u32)] = Cell::default();
        }
    }

    // Roof height per column across the wings and central dome.
    let roof_at = |col: i64| -> Option<i64> {
        let dx = (col - cx).abs();
        if dx > wing_half {
            return None;
        }
        if dx <= nave_half {
            let u = dx as f64 / nave_half.max(1) as f64;
            let rise = ((1.0 - u * u).max(0.0).sqrt() * dome_h as f64).round() as i64;
            Some(eave_row - 2 - rise)
        } else {
            let u = (dx - nave_half) as f64 / (wing_half - nave_half).max(1) as f64;
            let vault = ((1.0 - u * u).max(0.0).sqrt() * 2.5).round() as i64;
            Some(eave_row - vault)
        }
    };

    // Foliage inside the glasshouse, drawn softly before the iron glazing bars.
    let leaves = hash_label(seed, "foliage");
    for offset in [
        -wing_half * 2 / 3,
        -nave_half / 2,
        nave_half / 2,
        wing_half * 2 / 3,
    ] {
        layers::stamp(ctx, cells, PALM, cx + offset, foot_row - 1, 0.36);
    }
    for k in -3..=3 {
        let fc = cx + k * (wing_half / 4).max(3);
        layers::stamp(ctx, cells, FERN, fc, foot_row, 0.34);
    }
    for col in cx - wing_half + 1..cx + wing_half {
        let Some(top) = roof_at(col) else { continue };
        for row in top + 2..foot_row {
            if col < 0 || col >= i64::from(ctx.cols) || row < 0 || row >= i64::from(ctx.rows) {
                continue;
            }
            let n = fbm2(leaves, col as f64 * 0.22, row as f64 * 0.3, 2);
            if n > 0.62 && cells[ctx.idx(col as u32, row as u32)].glyph == EMPTY {
                put(
                    ctx,
                    cells,
                    col,
                    row,
                    if n > 0.7 { id('&') } else { id('.') },
                    if evening { 0.44 } else { 0.26 },
                );
            }
        }
    }

    // Iron ribs, glass mullions, arched doorway and warm hanging lamps.
    let bay = ctx.scaled(5.0).max(3);
    for col in cx - wing_half..=cx + wing_half {
        let Some(top) = roof_at(col) else { continue };
        let dx = col - cx;
        // Roof contour and finial on the central dome.
        let roof_glyph = if dx == 0 {
            put(ctx, cells, col, top - 1, id('^'), 0.62);
            id('_')
        } else if roof_at(col + 1).is_some_and(|n| n < top) {
            id('/')
        } else if roof_at(col - 1).is_some_and(|p| p < top) {
            id('\\')
        } else {
            id('_')
        };
        put(ctx, cells, col, top, roof_glyph, 0.6);

        let edge = dx.abs() == wing_half || dx.abs() == nave_half;
        let mullion = dx.rem_euclid(bay) == 0;
        for row in top + 1..foot_row {
            // Central arched double doorway.
            if dx.abs() <= 2 && row >= foot_row - 4 {
                if dx.abs() == 2 {
                    put(ctx, cells, col, row, id('|'), 0.58);
                } else if row == foot_row - 4 {
                    put(ctx, cells, col, row, id('_'), 0.58);
                } else if evening && dx == 0 && row == foot_row - 2 {
                    put(ctx, cells, col, row, id(':'), 0.85);
                }
                continue;
            }
            if row == eave_row {
                put(ctx, cells, col, row, id('='), 0.55);
            } else if edge {
                put(ctx, cells, col, row, id('|'), 0.56);
            } else if mullion {
                put(ctx, cells, col, row, id('|'), 0.36);
            } else if (row - foot_row).rem_euclid(3) == 0 {
                put(ctx, cells, col, row, id('-'), 0.24);
            }
        }
        // Low brick plinth along the foot.
        put(ctx, cells, col, foot_row, id('='), 0.5);
        let sky_y = co.ny((top - 1).max(0) as u32) - 0.5 / rows;
        super::shelter(ctx, &mut sky, col, sky_y);
    }

    // Hanging lamps under the dome and each wing.
    for lamp_x in [cx - wing_half / 2, cx, cx + wing_half / 2] {
        if let Some(top) = roof_at(lamp_x) {
            let lr = (top + eave_row) / 2 + 1;
            put(ctx, cells, lamp_x, lr - 1, id('|'), 0.42);
            put(
                ctx,
                cells,
                lamp_x,
                lr,
                id('o'),
                if evening { 0.98 } else { 0.48 },
            );
        }
    }

    // Garden gravel path and boxwood parterre lines in front of the conservatory.
    let path_seed = hash_label(seed, "terrace");
    for row in foot_row + 1..layers::row_at(ctx, ctx.horizon) {
        for col in 0..i64::from(ctx.cols) {
            let k = hash3(path_seed, col, row);
            if (col - cx).abs() <= 3 {
                if unit_from_hash(k) < 0.45 {
                    put(ctx, cells, col, row, id('.'), 0.34);
                }
            } else if row == foot_row + 2 && (col - cx).abs() < wing_half && col % 2 == 0 {
                put(ctx, cells, col, row, id(','), 0.3);
            }
        }
    }

    sky
}
