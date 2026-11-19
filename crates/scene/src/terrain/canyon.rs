//! The canyon: cliffs left and right, an open centre, and a rock ledge in the gap
//! where the focal group stands. Ported from the prototype's description.

use super::{Ctx, family_glyph, shade_u8};
use crate::glyphs;
use crate::graph::Cell;
use crate::hash::hash_label;
use crate::math::{clamp01, lerp, pm, pow, smoothstep};
use crate::noise::{fbm2, ridged1};

/// One cliff layer of the canyon.
struct CliffLayer {
    left: &'static [u8],
    right: &'static [u8],
    center_top: f64,
    edge_top: f64,
    gap: f64,
    rough: f64,
    rough_freq: f64,
    fx: f64,
    fy: f64,
    threshold: f64,
    shade: f64,
    seed: u64,
}

impl CliffLayer {
    /// Silhouette top (normalised y) at normalised x.
    fn top(&self, x: f64, center: f64, power: f64, aspect: f64) -> f64 {
        let half = if x < center { center } else { 1.0 - center };
        let d = ((x - center).abs() / half).min(1.0);
        let s = clamp01((d - self.gap) / (1.0 - self.gap));
        let shape = pow(smoothstep(0.0, 1.0, s), power);
        let ridge = ridged1(self.seed, x * aspect * self.rough_freq, 4);
        lerp(self.center_top, self.edge_top, shape) - self.rough * (ridge - 0.5)
    }
}

/// Three cliff layers, painted back to front.
pub(crate) fn background(ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    let comp = &ctx.recipe.composition;
    let (co, aspect, horizon, terrain) = (&ctx.co, ctx.aspect, ctx.horizon, ctx.terrain);
    let center = 0.5 + (ctx.group_cx / co.w - 0.5) * 0.6;
    let gap = pm(comp.gap_pm);
    let power = pm(comp.shape_pow_pm);
    let layers = [
        CliffLayer {
            left: &glyphs::FAR_LEFT,
            right: &glyphs::FAR_RIGHT,
            center_top: horizon + 0.05,
            edge_top: (horizon - 0.40).max(0.10),
            gap: gap * 0.40,
            rough: 0.07,
            rough_freq: 9.0,
            fx: 46.0,
            fy: 5.0,
            threshold: 0.30,
            shade: 0.35,
            seed: hash_label(terrain, "far"),
        },
        CliffLayer {
            left: &glyphs::MID_LEFT,
            right: &glyphs::MID_RIGHT,
            center_top: horizon + 0.22,
            edge_top: (horizon - 0.48).max(0.07),
            gap: gap * 0.90,
            rough: 0.10,
            rough_freq: 6.0,
            fx: 34.0,
            fy: 4.0,
            threshold: 0.32,
            shade: 0.60,
            seed: hash_label(terrain, "mid"),
        },
        CliffLayer {
            left: &glyphs::NEAR,
            right: &glyphs::NEAR,
            center_top: 1.20,
            edge_top: 0.03,
            gap: (gap * 1.6).min(0.9),
            rough: 0.12,
            rough_freq: 4.0,
            fx: 22.0,
            fy: 3.0,
            threshold: 0.30,
            shade: 0.90,
            seed: hash_label(terrain, "near"),
        },
    ];
    // Highest silhouette per column, for the sky pass.
    let mut sky_limit = vec![f64::INFINITY; ctx.cols as usize];
    let jitter_seed = hash_label(terrain, "jitter");
    for layer in &layers {
        for col in 0..ctx.cols {
            let nx = co.nx(col);
            let top = layer.top(nx, center, power, aspect);
            sky_limit[col as usize] = sky_limit[col as usize].min(top);
            let family = if nx < center { layer.left } else { layer.right };
            for row in 0..ctx.rows {
                let ny = co.ny(row);
                if ny < top {
                    continue;
                }
                let depth = ny - top;
                let n = fbm2(layer.seed, nx * aspect * layer.fx, ny * layer.fy, 4);
                let mut d = clamp01((n - 0.5) * 1.9 + 0.5);
                d *= 0.80 + 0.20 * smoothstep(0.0, 0.10, depth);
                if d < layer.threshold {
                    continue; // hole: farther layers show through
                }
                let glyph = family_glyph(
                    family,
                    d,
                    layer.threshold,
                    jitter_seed ^ layer.seed,
                    col,
                    row,
                );
                let rows_below_top = depth * f64::from(ctx.rows);
                let mut shade = layer.shade * (0.55 + 0.45 * (rows_below_top / 6.0).min(1.0));
                if row > ctx.platform_row {
                    shade *= 0.5; // ledges below the platform line recede
                }
                cells[ctx.idx(col, row)] = Cell {
                    glyph,
                    shade: shade_u8(shade),
                };
            }
        }
    }
    sky_limit
}
