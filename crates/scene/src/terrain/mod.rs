//! Terrain painters, one module per archetype, plus what they share.
//!
//! The scene pipeline (`build`) is the same for every archetype: lay out the
//! focal group, paint the archetype's background layers, clear breathing room
//! around the focal group, paint the archetype's platform, scatter the sky,
//! then light and particles. An archetype only supplies `background` (which
//! returns the highest silhouette per column, for the sky pass) and `platform`.

pub(crate) mod canyon;
pub(crate) mod cave;
pub(crate) mod city;
pub(crate) mod desert;
pub(crate) mod details;
pub(crate) mod forest;
pub(crate) mod interior;
pub(crate) mod layers;
pub(crate) mod ledge;
pub(crate) mod mountains;
pub(crate) mod night_sky;
pub(crate) mod shore;
pub(crate) mod snowfield;

use crate::graph::{Cell, GridSpec};
use crate::hash::{hash3, unit_from_hash};
use crate::math::clamp01;
use crate::recipe::{SceneRecipe, TimeOfDay};

/// Archetypes this generator version can paint. Must match the `implemented`
/// flags in `archetype::ARCHETYPE_INFO` (checked by a test).
pub const PAINTED: [&str; 10] = [
    "canyon",
    "mountains",
    "desert",
    "snowfield",
    "forest",
    "shore",
    "city_rooftops",
    "night_sky",
    "cave",
    "interior",
];

/// Normalised cell-centre coordinates.
pub(crate) struct Coords {
    pub g: GridSpec,
    pub w: f64,
    pub h: f64,
}

impl Coords {
    pub fn nx(&self, col: u32) -> f64 {
        (f64::from(self.g.origin_x) + (f64::from(col) + 0.5) * f64::from(self.g.cell_w)) / self.w
    }
    pub fn ny(&self, row: u32) -> f64 {
        (f64::from(self.g.origin_y) + (f64::from(row) + 0.5) * f64::from(self.g.cell_h)) / self.h
    }
    /// Column containing pixel x (may be outside the grid).
    pub fn col_of(&self, px: f64) -> i64 {
        ((px - f64::from(self.g.origin_x)) / f64::from(self.g.cell_w)).floor() as i64
    }
}

/// Everything a terrain painter needs to know about the scene.
pub(crate) struct Ctx<'a> {
    pub recipe: &'a SceneRecipe,
    pub co: Coords,
    pub aspect: f64,
    /// The screen's short side in pixels (object sizes follow it).
    pub unit: f64,
    pub cols: u32,
    pub rows: u32,
    /// Platform top (the horizon line), normalised from the top.
    pub horizon: f64,
    pub platform_row: u32,
    /// `hash(seed, "terrain")`: sub-seeds for every layer derive from it.
    pub terrain: u64,
    /// Horizontal extent of the focal group in pixels, and its centre.
    pub gx0: i32,
    pub gx1: i32,
    pub group_cx: f64,
    /// For archetypes that change with the time of day (lit windows, aurora).
    pub tod: TimeOfDay,
}

impl Ctx<'_> {
    #[inline]
    pub fn idx(&self, col: u32, row: u32) -> usize {
        (row * self.cols + col) as usize
    }
}

pub(crate) fn shade_u8(s: f64) -> u8 {
    (clamp01(s) * 255.0).round() as u8
}

/// Picks a glyph from a light-to-heavy family by density, with a seeded jitter of
/// up to ±0.6 of a step so it doesn't band.
pub(crate) fn family_glyph(
    family: &[u8],
    density: f64,
    threshold: f64,
    jitter_seed: u64,
    col: u32,
    row: u32,
) -> u8 {
    let n = family.len() as f64;
    let jitter = (unit_from_hash(hash3(jitter_seed, i64::from(col), i64::from(row))) - 0.5) * 1.2;
    let t = (density - threshold) / (1.0 - threshold) * n + jitter;
    family[t.floor().clamp(0.0, n - 1.0) as usize]
}

/// Paints the archetype's background layers; returns the highest silhouette
/// (normalised y) per column for the sky pass.
pub(crate) fn background(archetype: &str, ctx: &Ctx, cells: &mut [Cell]) -> Vec<f64> {
    match archetype {
        "canyon" => canyon::background(ctx, cells),
        "mountains" => mountains::background(ctx, cells),
        "desert" => desert::background(ctx, cells),
        "snowfield" => snowfield::background(ctx, cells),
        "forest" => forest::background(ctx, cells),
        "shore" => shore::background(ctx, cells),
        "city_rooftops" => city::background(ctx, cells),
        "night_sky" => night_sky::background(ctx, cells),
        "cave" => cave::background(ctx, cells),
        "interior" => interior::background(ctx, cells),
        other => unreachable!("archetype `{other}` is not painted"),
    }
}

/// The archetype's platform style.
fn ledge_style(archetype: &str) -> &'static ledge::LedgeStyle {
    match archetype {
        "canyon" | "mountains" | "cave" => &ledge::ROCK,
        "desert" => &ledge::DUNE,
        "snowfield" => &ledge::ICE,
        "forest" => &ledge::GRASS,
        other => unreachable!("archetype `{other}` is not painted"),
    }
}

/// Paints the archetype's platform under the focal group.
pub(crate) fn platform(archetype: &str, ctx: &Ctx, cells: &mut [Cell]) {
    match archetype {
        "shore" => shore::pier(ctx, cells),
        "city_rooftops" => city::rooftop(ctx, cells),
        "night_sky" => night_sky::island(ctx, cells),
        "interior" => interior::floor(ctx, cells),
        other => ledge::paint(ctx, cells, ledge_style(other)),
    }
}

/// The glyph on the platform's top row, the surface the feet stand on.
#[cfg(test)]
pub(crate) fn platform_top(archetype: &str) -> u8 {
    match archetype {
        "shore" | "city_rooftops" | "interior" => crate::glyphs::id('='),
        "night_sky" => crate::glyphs::id('"'),
        other => ledge_style(other).top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archetype::ARCHETYPE_INFO;

    #[test]
    fn painted_matches_implemented_flags() {
        for a in &ARCHETYPE_INFO {
            assert_eq!(a.implemented, PAINTED.contains(&a.id), "{}", a.id);
        }
    }
}
