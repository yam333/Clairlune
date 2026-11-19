//! The platform under the focal group: a ledge whose top row spans the group and
//! whose body widens (or tapers) downward with noisy edges. Archetypes pick a
//! style: rock (canyon), and variations for other landscapes.

use super::{Ctx, family_glyph, shade_u8};
use crate::glyphs;
use crate::graph::Cell;
use crate::hash::hash_label;
use crate::math::{clamp01, lerp, pm, smoothstep};
use crate::noise::{fbm1, fbm2};

/// How a ledge looks.
pub(crate) struct LedgeStyle {
    /// Body glyphs, light to heavy.
    pub family: &'static [u8],
    /// Glyph for the top row (the surface the feet stand on).
    pub top: u8,
    pub top_shade: f64,
    /// Body shade at the top and at the bottom of the screen.
    pub shade: (f64, f64),
    /// Density below this leaves a hole (below the first row).
    pub threshold: f64,
    /// Added to the stretched noise: higher = heavier glyphs.
    pub bias: f64,
    /// Widening: cells per row for the first three rows, the width reached
    /// after them (kept as a literal so results stay bit-exact), and cells per
    /// row after that.
    pub widen: (f64, f64, f64),
    /// Amplitude of the edge noise (cells).
    pub roughness: f64,
    /// Noise frequency across and down the body.
    pub freq: (f64, f64),
}

/// The canyon's rock ledge. Changing these values changes canyon output.
pub(crate) const ROCK: LedgeStyle = LedgeStyle {
    family: &glyphs::PLATFORM,
    top: glyphs::PLATFORM_TOP,
    top_shade: 0.85,
    shade: (0.80, 0.40),
    threshold: 0.20,
    bias: 0.42,
    widen: (0.6, 1.8, 0.25),
    roughness: 9.0,
    freq: (9.0, 38.0),
};

/// The snowfield's ice shelf: a smooth slab of `-=+#`, narrow and steep.
pub(crate) const ICE: LedgeStyle = LedgeStyle {
    family: &glyphs::ICE,
    top: glyphs::id('='),
    top_shade: 0.9,
    shade: (0.75, 0.35),
    threshold: 0.15,
    bias: 0.38,
    widen: (0.8, 2.4, 0.4),
    roughness: 4.0,
    freq: (6.0, 20.0),
};

/// The desert's dune crest: soft sand spreading wide, with wind ripples.
pub(crate) const DUNE: LedgeStyle = LedgeStyle {
    family: &glyphs::SAND,
    top: glyphs::id('~'),
    top_shade: 0.85,
    shade: (0.75, 0.40),
    threshold: 0.22,
    bias: 0.40,
    widen: (1.6, 4.8, 1.2),
    roughness: 5.0,
    freq: (3.0, 45.0),
};

/// The forest's grassy bank: earth under a fringe of grass.
pub(crate) const GRASS: LedgeStyle = LedgeStyle {
    family: &glyphs::EARTH,
    top: glyphs::id('"'),
    top_shade: 0.85,
    shade: (0.7, 0.35),
    threshold: 0.25,
    bias: 0.42,
    widen: (0.8, 2.4, 0.5),
    roughness: 6.0,
    freq: (8.0, 30.0),
};

pub(crate) fn paint(ctx: &Ctx, cells: &mut [Cell], style: &LedgeStyle) {
    let co = &ctx.co;
    let comp = &ctx.recipe.composition;
    let platform_seed = hash_label(ctx.terrain, "platform");
    let margin_px = pm(comp.platform_margin_pm) * ctx.unit + 2.0 * f64::from(co.g.cell_w);
    let pc0 = co.col_of(f64::from(ctx.gx0) - margin_px);
    let pc1 = co.col_of(f64::from(ctx.gx1) + margin_px);
    let span_rows = f64::from(ctx.rows - ctx.platform_row);
    let (cols, platform_row) = (ctx.cols, ctx.platform_row);
    for row in platform_row..ctx.rows {
        let k = f64::from(row - platform_row);
        let widen = if k < 3.0 {
            style.widen.0 * k
        } else {
            style.widen.1 + style.widen.2 * (k - 3.0)
        };
        // Edge noise fades in over the first rows, so the top always spans the group.
        let rough = style.roughness * (k / 3.0).min(1.0);
        let el = (fbm1(hash_label(platform_seed, "left"), k * 0.22, 3) - 0.5) * rough;
        let er = (fbm1(hash_label(platform_seed, "right"), k * 0.22, 3) - 0.5) * rough;
        let c0 = (pc0 as f64 - widen - el).round() as i64;
        let c1 = (pc1 as f64 + widen + er).round() as i64;
        // One empty cell of separation so the ledge reads as its own object.
        for c in [c0 - 1, c1 + 1] {
            if (0..i64::from(cols)).contains(&c) {
                cells[ctx.idx(c as u32, row)] = Cell::default();
            }
        }
        let fade = smoothstep(0.0, 1.0, k / span_rows);
        for c in c0.max(0)..=c1.min(i64::from(cols) - 1) {
            let col = c as u32;
            let idx = ctx.idx(col, row);
            if k == 0.0 {
                cells[idx] = Cell {
                    glyph: style.top,
                    shade: shade_u8(style.top_shade),
                };
                continue;
            }
            let n = fbm2(
                platform_seed,
                co.nx(col) * ctx.aspect * style.freq.0,
                co.ny(row) * style.freq.1,
                3,
            );
            let d = clamp01((n - 0.5) * 1.8 + style.bias);
            if d < style.threshold && k > 1.0 {
                cells[idx] = Cell::default();
                continue;
            }
            let glyph = family_glyph(
                style.family,
                d.max(style.threshold),
                style.threshold,
                platform_seed,
                col,
                row,
            );
            cells[idx] = Cell {
                glyph,
                shade: shade_u8(lerp(style.shade.0, style.shade.1, fade)),
            };
        }
    }
}
