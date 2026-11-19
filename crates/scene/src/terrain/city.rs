//! City rooftops: two layers of buildings (a hazy far skyline, then taller
//! blocks with walls, roof lines and window grids, lit after dark), and the
//! roof of the focal group's own building as the platform.

use super::Ctx;
use crate::glyphs::{EMPTY, id};
use crate::graph::Cell;
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::pm;
use crate::recipe::TimeOfDay;

use super::layers::put;

/// One skyline layer.
struct Skyline {
    label: &'static str,
    /// Roof height above the horizon (normalised): minimum and random extra.
    height: (f64, f64),
    /// Building width as a fraction of the screen width: minimum, extra.
    width: (f64, f64),
    shade: f64,
    /// Draw walls and roof lines (near) or only windows on a dark block (far).
    outlined: bool,
    /// Chance of a roof antenna.
    antennas: f64,
}

const LAYERS: [Skyline; 2] = [
    Skyline {
        label: "city_far",
        height: (0.06, 0.20),
        width: (0.03, 0.05),
        shade: 0.32,
        outlined: false,
        antennas: 0.0,
    },
    Skyline {
        label: "city_near",
        height: (0.02, 0.26),
        width: (0.05, 0.08),
        shade: 0.55,
        outlined: true,
        antennas: 0.3,
    },
];

/// Share of windows lit: more after dark.
fn lit_share(tod: TimeOfDay) -> f64 {
    match tod {
        TimeOfDay::Night | TimeOfDay::DeepDusk => 0.45,
        TimeOfDay::Dusk => 0.3,
        TimeOfDay::GoldenHour => 0.12,
        TimeOfDay::Morning | TimeOfDay::Noon => 0.05,
    }
}

/// The row whose centre is at or below normalised y.
fn row_of(ctx: &Ctx, y: f64) -> i64 {
    let g = &ctx.co.g;
    ((y * ctx.co.h - f64::from(g.origin_y)) / f64::from(g.cell_h) - 0.5).ceil() as i64
}

pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let mut sky = vec![f64::INFINITY; ctx.cols as usize];
    let cols = i64::from(ctx.cols);
    for layer in &LAYERS {
        let seed = hash_label(ctx.terrain, layer.label);
        let mut c0 = -((hash3(seed, -1, 0) % 6) as i64);
        let mut i = 0i64;
        while c0 < cols {
            let k = |salt: i64| unit_from_hash(hash3(seed, i, salt));
            let w = ((layer.width.0 + layer.width.1 * k(1)) * f64::from(ctx.cols)).round() as i64;
            let w = w.max(4);
            let top_y = ctx.horizon - (layer.height.0 + layer.height.1 * k(2));
            let top = row_of(ctx, top_y).max(1);
            let b = Block {
                c0,
                c1: c0 + w - 1,
                top,
                seed: hash3(seed, i, 3),
            };
            b.draw(ctx, cells, layer);
            if layer.antennas > 0.0 && k(4) < layer.antennas {
                let ac = c0 + 1 + (hash3(seed, i, 5) % (w as u64 - 2)) as i64;
                put(ctx, cells, ac, top - 2, id('|'), layer.shade);
                put(ctx, cells, ac, top - 3, id('|'), layer.shade);
                put(ctx, cells, ac, top - 4, id('\''), layer.shade * 1.2);
            }
            for c in c0.max(0)..(c0 + w).min(cols) {
                let y = ctx.co.ny(top.max(0) as u32) - 1.5 / f64::from(ctx.rows);
                sky[c as usize] = sky[c as usize].min(y);
            }
            // Narrow gaps between some blocks.
            c0 += w + if k(6) < 0.4 {
                1 + (k(7) * 3.0) as i64
            } else {
                0
            };
            i += 1;
        }
    }
    sky
}

/// A building from column `c0` to `c1` (inclusive), roof on row `top`,
/// running down past the bottom of the screen.
struct Block {
    c0: i64,
    c1: i64,
    top: i64,
    seed: u64,
}

impl Block {
    fn draw(&self, ctx: &Ctx, cells: &mut [Cell], layer: &Skyline) {
        let lit = lit_share(ctx.tod);
        let rows = i64::from(ctx.rows);
        for r in self.top..rows {
            for c in self.c0..=self.c1 {
                let edge = c == self.c0 || c == self.c1;
                let (glyph, shade) = if layer.outlined && edge {
                    (id('|'), layer.shade * 0.8)
                } else {
                    self.window(c, r, lit, layer)
                };
                put(ctx, cells, c, r, glyph, shade);
            }
        }
        if layer.outlined {
            // Roof line: `_` on the row above, so it sits on the roof's top edge.
            for c in self.c0..=self.c1 {
                put(ctx, cells, c, self.top - 1, id('_'), layer.shade);
            }
        }
    }

    /// Windows on a grid of every other column and row inside the walls:
    /// lit ones bright, dark ones faint, some with no window at all.
    fn window(&self, c: i64, r: i64, lit: f64, layer: &Skyline) -> (u8, f64) {
        let (dc, dr) = (c - self.c0, r - self.top);
        if dc % 2 == 0 || dr % 2 == 0 || dr == 0 {
            return (EMPTY, 0.0);
        }
        let u = unit_from_hash(hash3(self.seed, c, r));
        if u < lit {
            let glyph = if layer.outlined { id('#') } else { id(':') };
            (glyph, (layer.shade * 1.5).min(0.95))
        } else if u < 0.75 {
            (id('.'), layer.shade * 0.6)
        } else {
            (EMPTY, 0.0)
        }
    }
}

/// The focal group's rooftop: a parapet on the platform row spanning the group
/// with some room either side, walls down to the bottom, windows below.
pub(crate) fn rooftop(ctx: &Ctx, cells: &mut [Cell]) {
    let co = &ctx.co;
    let comp = &ctx.recipe.composition;
    let margin_px = pm(comp.platform_margin_pm) * ctx.unit + 6.0 * f64::from(co.g.cell_w);
    let c0 = co.col_of(f64::from(ctx.gx0) - margin_px);
    let c1 = co.col_of(f64::from(ctx.gx1) + margin_px);
    let roof = i64::from(ctx.platform_row);
    // Clear a gap of one column around the building so it stands apart.
    for r in roof - 1..i64::from(ctx.rows) {
        put(ctx, cells, c0 - 1, r, EMPTY, 0.0);
        put(ctx, cells, c1 + 1, r, EMPTY, 0.0);
    }
    let block = Block {
        c0,
        c1,
        top: roof + 1,
        seed: hash_label(ctx.terrain, "rooftop"),
    };
    let layer = Skyline {
        label: "rooftop",
        height: (0.0, 0.0),
        width: (0.0, 0.0),
        shade: 0.7,
        outlined: true,
        antennas: 0.0,
    };
    block.draw(ctx, cells, &layer);
    for c in c0..=c1 {
        put(ctx, cells, c, roof, id('='), 0.85);
    }
    // Parapet corners.
    put(ctx, cells, c0, roof, id('+'), 0.85);
    put(ctx, cells, c1, roof, id('+'), 0.85);
}
