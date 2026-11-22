//! Terrain painters, one module per archetype, plus what they share.
//!
//! The scene pipeline (`build`) is the same for every archetype: lay out the
//! focal group, paint the archetype's background layers, clear breathing room
//! around the focal group, paint the archetype's platform, scatter the sky,
//! then light and particles. An archetype only supplies `background` (which
//! returns the highest silhouette per column, for the sky pass) and `platform`.

pub(crate) mod bamboo;
pub(crate) mod canyon;
pub(crate) mod cave;
pub(crate) mod city;
pub(crate) mod countryside;
pub(crate) mod crystals;
pub(crate) mod desert;
pub(crate) mod details;
pub(crate) mod forest;
pub(crate) mod great_tree;
pub(crate) mod interior;
pub(crate) mod islands;
pub(crate) mod lake;
pub mod landmark;
pub(crate) mod lantern_lake;
pub(crate) mod layers;
pub(crate) mod ledge;
pub(crate) mod lunar;
pub(crate) mod meadow;
pub(crate) mod mountains;
pub(crate) mod mushrooms;
pub(crate) mod night_sky;
pub(crate) mod ruins;
pub(crate) mod savanna;
pub(crate) mod shore;
pub(crate) mod snowfield;
pub(crate) mod standing_stones;
pub(crate) mod swamp;
pub(crate) mod volcano;
pub(crate) mod waterfall;
pub(crate) mod wizard_tower;

use crate::graph::{Cell, GridSpec};
use crate::hash::{hash3, unit_from_hash};
use crate::math::clamp01;
use crate::recipe::{SceneRecipe, TimeOfDay};

/// Archetypes this generator version can paint. Must match the `implemented`
/// flags in `archetype::ARCHETYPE_INFO` (checked by a test).
pub const PAINTED: [&str; 27] = [
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
    "meadow",
    "lake",
    "volcano",
    "islands",
    "lunar",
    "bamboo",
    "waterfall",
    "countryside",
    "ruins",
    "savanna",
    "swamp",
    "mushrooms",
    "crystals",
    "great_tree",
    "wizard_tower",
    "standing_stones",
    "lantern_lake",
];

/// What an archetype paints for a journey: the still background's sky limit
/// (as [`background`] returns), the layers that scroll past, back to front,
/// and the walking layer's ground (pixel y) at each pixel column of its strip.
pub(crate) struct Journeyed {
    pub sky_limit: Vec<f64>,
    pub layers: Vec<crate::graph::ScrollLayer>,
    pub surface: Vec<f64>,
}

/// Paints a journey's still background into `cells` and returns its moving
/// layers, or `None` when the archetype has no journey.
pub(crate) fn journey(archetype: &str, ctx: &Ctx, cells: &mut [Cell]) -> Option<Journeyed> {
    // The landmark is the farthest thing: the still land goes over its foot.
    let landmark = landmark::paint(ctx, cells);
    let mut j = match archetype {
        "mountains" => mountains::journey(ctx, cells),
        "snowfield" => snowfield::journey(ctx, cells),
        "forest" => forest::journey(ctx, cells),
        "desert" => desert::journey(ctx, cells),
        _ => return None,
    };
    for (s, l) in j.sky_limit.iter_mut().zip(landmark) {
        *s = s.min(l);
    }
    Some(j)
}

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
    /// Per column, the sky limit from before things in the sky (a moon, a
    /// lighthouse, boats) kept the stars off: where the land really starts.
    /// `None` where nothing did.
    pub unsheltered: std::cell::RefCell<Vec<Option<f64>>>,
}

impl Ctx<'_> {
    #[inline]
    pub fn idx(&self, col: u32, row: u32) -> usize {
        (row * self.cols + col) as usize
    }

    /// How tall this grid is against the usual one (about
    /// [`crate::graph::TARGET_ROWS`] rows; small images have fewer).
    pub fn row_scale(&self) -> f64 {
        f64::from(self.rows) / f64::from(crate::graph::TARGET_ROWS)
    }

    /// A height given in rows of the usual grid, in rows of this one.
    pub fn scaled(&self, rows: f64) -> i64 {
        (rows * self.row_scale()).round() as i64
    }

    /// Where the land starts in each column: the sky limit, without the
    /// shelter given to things standing in the sky.
    pub fn land_line(&self, sky_limit: &[f64]) -> Vec<f64> {
        let u = self.unsheltered.borrow();
        sky_limit
            .iter()
            .enumerate()
            .map(|(c, &s)| u.get(c).copied().flatten().map_or(s, |v| v.max(s)))
            .collect()
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
        "meadow" => meadow::background(ctx, cells),
        "lake" => lake::background(ctx, cells),
        "volcano" => volcano::background(ctx, cells),
        "islands" => islands::background(ctx, cells),
        "lunar" => lunar::background(ctx, cells),
        "bamboo" => bamboo::background(ctx, cells),
        "waterfall" => waterfall::background(ctx, cells),
        "countryside" => countryside::background(ctx, cells),
        "ruins" => ruins::background(ctx, cells),
        "savanna" => savanna::background(ctx, cells),
        "swamp" => swamp::background(ctx, cells),
        "mushrooms" => mushrooms::background(ctx, cells),
        "crystals" => crystals::background(ctx, cells),
        "great_tree" => great_tree::background(ctx, cells),
        "wizard_tower" => wizard_tower::background(ctx, cells),
        "standing_stones" => standing_stones::background(ctx, cells),
        "lantern_lake" => lantern_lake::background(ctx, cells),
        other => unreachable!("archetype `{other}` is not painted"),
    }
}

/// The archetype's platform style.
fn ledge_style(archetype: &str) -> &'static ledge::LedgeStyle {
    match archetype {
        "canyon" | "mountains" | "cave" | "volcano" | "lunar" | "waterfall" | "ruins"
        | "crystals" | "wizard_tower" => &ledge::ROCK,
        "desert" | "islands" | "savanna" => &ledge::DUNE,
        "snowfield" => &ledge::ICE,
        "forest" | "meadow" | "lake" | "bamboo" | "countryside" | "swamp" | "mushrooms"
        | "great_tree" | "standing_stones" | "lantern_lake" => &ledge::GRASS,
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
