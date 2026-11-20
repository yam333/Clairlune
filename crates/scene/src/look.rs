//! Looks and contrast: how a scene is drawn once it is painted.
//!
//! - `line`: the painted glyph line art as it is (the original look).
//! - `dither`: the scene as light and dark masses, every cell a glyph from
//!   [`DITHER`] (a dot, a bigger dot, a bar, a square) by how bright it is
//!   there, ordered-dithered between neighbouring sizes. A bright sky in
//!   bands and streaks over dark land, haze dots under the ridges, glints
//!   on the water. Ink reads as light on the dark theme and as dark on the
//!   light theme, so a bright sky is dense on one and sparse on the other.
//!
//! Contrast scales the ink: soft, normal or high.

use crate::glyphs::{DITHER, EMPTY};
use crate::graph::Cell;
use crate::hash::{hash_label, unit_from_hash};
use crate::math::clamp01;
use crate::noise::fbm2;
use crate::palette::Theme;
use crate::recipe::TimeOfDay;
use crate::terrain::{Ctx, shade_u8};

/// 4x4 ordered-dither thresholds (Bayer), in sixteenths.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// The glyph step for ink density `d` (0..1) at a cell: between two sizes,
/// the Bayer threshold picks which.
pub(crate) fn ramp_step(d: f64, col: u32, row: u32) -> usize {
    let n = (DITHER.len() - 1) as f64;
    let p = clamp01(d) * n;
    let base = p.floor();
    let t = (f64::from(BAYER[(row % 4) as usize][(col % 4) as usize]) + 0.5) / 16.0;
    let step = base as usize + usize::from(p - base > t);
    step.min(DITHER.len() - 1)
}

/// How much brighter or darker than mid grey contrast pushes things.
fn spread(contrast: &str) -> f64 {
    match contrast {
        "soft" => 0.7,
        "high" => 1.4,
        _ => 1.0,
    }
}

/// Line art with the contrast applied: ink strength scaled.
pub(crate) fn line_contrast(cells: &mut [Cell], contrast: &str) {
    let k = match contrast {
        "soft" => 0.72,
        "high" => 1.4,
        _ => return,
    };
    for c in cells.iter_mut().filter(|c| c.glyph != EMPTY) {
        c.shade = (f64::from(c.shade) * k).round().min(255.0) as u8;
    }
}

/// The sky's brightness at normalised height `g` (0 at the top, 1 at the
/// horizon): where the light comes from and how much there is, by time of
/// day.
fn sky_light(tod: TimeOfDay, from_top: bool, g: f64) -> f64 {
    let (base, span) = match tod {
        TimeOfDay::Night => (0.16, 0.62),
        TimeOfDay::DeepDusk => (0.2, 0.62),
        TimeOfDay::Dusk => (0.26, 0.62),
        TimeOfDay::GoldenHour => (0.3, 0.6),
        TimeOfDay::Morning => (0.38, 0.55),
        TimeOfDay::Noon => (0.42, 0.55),
    };
    // After sunset and before sunrise the glow is low in the sky; a high,
    // lit sky (moonlit cloud, a bright overcast) comes from the top.
    let toward = if from_top { 1.0 - g } else { g };
    base + span * toward * toward.sqrt()
}

/// What the dither pass needs besides the painted cells.
pub(crate) struct Dither<'a> {
    /// Highest silhouette per column (normalised y): sky above, land below.
    pub sky_limit: &'a [f64],
    pub theme: Theme,
    pub tod: TimeOfDay,
    pub contrast: &'a str,
    /// The focal group's breathing room: <= 1 inside, up to 2.4 at its rim.
    pub dome: &'a dyn Fn(u32, u32) -> f64,
    /// Where open water starts (normalised y), if there is any: it mirrors
    /// the sky in streaks.
    pub sea_top: Option<f64>,
    /// Cells of the ground the group stands on (a ledge, a pier, a roof):
    /// drawn as a lighter foreground, so the feet are on something.
    pub ground: &'a [bool],
}

/// Columns either side over which the land line is taken at its lowest, so
/// a spike a column or two wide is a mark in the sky rather than a column
/// of land.
const SPIKE: usize = 1;

/// Redraws the painted cells in the dithered look. Works in ink: how much
/// of a cell to cover. On the dark theme ink is light, so a bright sky is
/// dense and the land dark; on the light theme ink is dark, so the sky is
/// sparse and the land a mid-tone mass. Drawn marks stay ink either way.
pub(crate) fn dither(ctx: &Ctx, cells: &mut [Cell], d: &Dither) {
    let seed = hash_label(ctx.terrain, "dither");
    let from_top = unit_from_hash(seed) < 0.5;
    let cloud_seed = hash_label(ctx.terrain, "dither_clouds");
    let wave_seed = hash_label(ctx.terrain, "dither_waves");
    let k = spread(d.contrast);
    let horizon = ctx.horizon.max(0.05);
    let platform = ctx.platform_row;
    let n = d.sky_limit.len();
    let land_line: Vec<f64> = (0..n)
        .map(|c| {
            (c.saturating_sub(SPIKE)..(c + SPIKE + 1).min(n))
                .map(|i| d.sky_limit[i])
                .fold(f64::NEG_INFINITY, f64::max)
        })
        .collect();
    let dark = d.theme == Theme::Dark;
    let g0 = ctx.co.col_of(f64::from(ctx.gx0));
    let g1 = ctx.co.col_of(f64::from(ctx.gx1));
    for row in 0..ctx.rows {
        let ny = ctx.co.ny(row);
        for col in 0..ctx.cols {
            let i = ctx.idx(col, row);
            let cell = cells[i];
            // The ground line stays as drawn, gaps and all under the group:
            // it stands on something that reads as a surface.
            if row == platform && (g0..=g1).contains(&i64::from(col)) {
                continue;
            }
            let nx = ctx.co.nx(col);
            let top = land_line[col as usize];
            let mark = if cell.glyph == EMPTY {
                0.0
            } else {
                f64::from(cell.shade) / 255.0
            };
            let sky_ink = |y: f64| -> f64 {
                let g = (y / horizon).clamp(0.0, 1.0);
                let clouds = fbm2(cloud_seed, nx * ctx.aspect * 1.6, y * 11.0, 4) - 0.5;
                let light = clamp01(sky_light(d.tod, from_top, g) + clouds * 0.5);
                if dark {
                    light
                } else {
                    0.04 + (1.0 - light) * 0.42
                }
            };
            let water = d.sea_top.filter(|&s| ny > s + 0.004);
            if d.ground.get(i).copied().unwrap_or(false) && cell.glyph != EMPTY {
                let ink = if dark {
                    0.2 + mark * 0.45
                } else {
                    0.28 + mark * 0.35
                };
                let ink = clamp01(0.5 + (clamp01(ink) - 0.5) * k);
                let step = ramp_step(ink, col, row).max(1);
                cells[i] = Cell {
                    glyph: DITHER[step],
                    shade: shade_u8((0.3 + 0.7 * ink) * strength(d.contrast)),
                };
                continue;
            }
            let mut ink = if let Some(sea) = water {
                // Open water: the sky mirrored, squashed toward the sea
                // line, broken into long horizontal streaks of glints.
                let mirrored = (sea - (ny - sea) * 1.6).max(0.0);
                let streak = fbm2(wave_seed, nx * ctx.aspect * 3.0, ny * 70.0, 3);
                let glint = crate::math::smoothstep(0.48, 0.72, streak);
                if dark {
                    (sky_ink(mirrored) * glint * 0.9).max(mark * 0.3)
                } else {
                    // A mid-tone sea with the bright sky's streaks in it.
                    (0.3 - 0.24 * glint).max(mark * 0.3)
                }
            } else if ny < top {
                // Sky: the light, in long streaky bands of cloud; stars and
                // whatever was drawn there show through.
                sky_ink(ny).max(mark)
            } else {
                let depth = ny - top;
                if dark {
                    // Dark land: its texture faint, a haze of dots just
                    // under the ridge line.
                    (mark * 0.4).max(0.22 * (1.0 - depth / 0.1).max(0.0))
                } else {
                    // A mid-tone mass, a little darker lower down.
                    0.3 + 0.12 * (depth / 0.2).min(1.0) + mark * 0.25
                }
            };
            // Breathing room around the group: less ink, so it reads.
            let s = (d.dome)(col, row);
            if s <= 1.0 {
                ink *= 0.4;
            } else if s <= 2.4 {
                ink *= 0.7;
            }
            let ink = clamp01(0.5 + (clamp01(ink) - 0.5) * k);
            let step = ramp_step(ink, col, row);
            cells[i] = if step == 0 {
                Cell::default()
            } else {
                Cell {
                    glyph: DITHER[step],
                    shade: shade_u8((0.3 + 0.7 * ink) * strength(d.contrast)),
                }
            };
        }
    }
}

/// Ink strength of the dithered glyphs: soft contrast also inks lighter.
fn strength(contrast: &str) -> f64 {
    if contrast == "soft" { 0.85 } else { 1.0 }
}

/// A journey's moving layer (`cols` wide) in the dithered look: its marks
/// become land texture; see-through and hidden cells stay as they are.
pub(crate) fn dither_strip(cells: &mut [Cell], cols: u32, theme: Theme, contrast: &str) {
    let k = spread(contrast);
    for (i, c) in cells.iter_mut().enumerate() {
        if *c == Cell::default() || *c == crate::graph::HOLE {
            continue;
        }
        let (col, row) = ((i as u32) % cols, (i as u32) / cols);
        let mark = f64::from(c.shade) / 255.0;
        let ink = match theme {
            Theme::Dark => mark * 0.4,
            Theme::Light => 0.34 + mark * 0.25,
        };
        let ink = clamp01(0.5 + (ink - 0.5) * k);
        let step = ramp_step(ink, col, row);
        // Inside the silhouette: an empty step still hides what's behind.
        *c = if step == 0 {
            crate::graph::HOLE
        } else {
            Cell {
                glyph: DITHER[step],
                shade: shade_u8((0.3 + 0.7 * ink) * strength(contrast)),
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ramp_covers_every_size_and_dithers_between_them() {
        assert_eq!(ramp_step(0.0, 0, 0), 0);
        assert_eq!(ramp_step(1.0, 3, 3), DITHER.len() - 1);
        // Halfway between two sizes: half the cells of a 4x4 block each.
        let n = (DITHER.len() - 1) as f64;
        let d = 2.5 / n;
        let steps: Vec<usize> = (0..16).map(|i| ramp_step(d, i % 4, i / 4)).collect();
        assert_eq!(steps.iter().filter(|&&s| s == 2).count(), 8);
        assert_eq!(steps.iter().filter(|&&s| s == 3).count(), 8);
        // More ink never means a smaller glyph at the same cell.
        for i in 0..16 {
            let mut last = 0;
            for k in 0..=100 {
                let s = ramp_step(f64::from(k) / 100.0, i % 4, i / 4);
                assert!(s >= last);
                last = s;
            }
        }
    }

    #[test]
    fn contrast_spreads_ink() {
        let mut cells = vec![
            Cell {
                glyph: 1,
                shade: 100,
            },
            Cell::default(),
        ];
        line_contrast(&mut cells, "high");
        assert_eq!(cells[0].shade, 140);
        assert_eq!(cells[1], Cell::default());
        line_contrast(&mut cells, "normal");
        assert_eq!(cells[0].shade, 140);
        line_contrast(&mut cells, "soft");
        assert_eq!(cells[0].shade, 101);
    }
}
