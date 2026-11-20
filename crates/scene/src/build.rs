//! `build(recipe, size, theme, buddy) -> SceneGraph`.
//!
//! All layout math uses normalised coordinates (0..1) sampled at cell centres, so
//! the same recipe gives the same landscape at any resolution; only the cell
//! sampling changes. Buddy is a render flag: it changes sprites, never cells.

use crate::activity::{Activity, Catalog, builtin_catalog};
use crate::glyphs::{self, EMPTY};
use crate::graph::{Cell, Glow, Lighting, SceneGraph, grid_spec};
use crate::hash::hash_label;
use crate::math::pm;
use crate::palette::{self, PaletteFamily, Theme};
use crate::particles::{
    CALM_PACE, Emitter, EmitterKind, LOOP_MS, WeatherKind, ember_set, hero_time, weather_set,
};
use crate::recipe::{GENERATOR_VERSION, OLDEST_GENERATOR_VERSION, SceneRecipe, TimeOfDay};
use crate::rng::Rng;
use crate::{focal, terrain};
use std::fmt;

/// Smallest supported output size.
pub const MIN_WIDTH: u32 = 160;
pub const MIN_HEIGHT: u32 = 90;

/// Render-time inputs that are not part of the recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    pub width: u32,
    pub height: u32,
    pub theme: Theme,
    /// Show the character (Buddy toggle). Off draws the unattended variant.
    pub buddy: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    UnsupportedVersion(u32),
    UnknownArchetype(String),
    UnknownActivity(String),
    UnknownPalette(String),
    /// The recipe has no time of day for this theme.
    ThemeLocked(Theme),
    TooSmall {
        width: u32,
        height: u32,
    },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::UnsupportedVersion(v) => write!(
                f,
                "recipe generator_version {v} is not supported (this build is version {GENERATOR_VERSION})"
            ),
            BuildError::UnknownArchetype(a) => write!(f, "unknown archetype `{a}`"),
            BuildError::UnknownActivity(a) => write!(f, "unknown activity `{a}`"),
            BuildError::UnknownPalette(p) => write!(f, "unknown palette family `{p}`"),
            BuildError::ThemeLocked(t) => {
                write!(f, "this scene has no {} variant (theme-locked)", t.as_str())
            }
            BuildError::TooSmall { width, height } => write!(
                f,
                "{width}x{height} is too small; the minimum is {MIN_WIDTH}x{MIN_HEIGHT}"
            ),
        }
    }
}

impl std::error::Error for BuildError {}

/// Builds the scene graph for one size, theme and Buddy state.
pub fn build(recipe: &SceneRecipe, opts: &BuildOptions) -> Result<SceneGraph, BuildError> {
    build_with(builtin_catalog(), recipe, opts)
}

/// [`build`] with another activity catalog (the developer preview loads one
/// from disk to show edits without a rebuild).
pub fn build_with(
    catalog: &Catalog,
    recipe: &SceneRecipe,
    opts: &BuildOptions,
) -> Result<SceneGraph, BuildError> {
    if !(OLDEST_GENERATOR_VERSION..=GENERATOR_VERSION).contains(&recipe.generator_version) {
        return Err(BuildError::UnsupportedVersion(recipe.generator_version));
    }
    if opts.width < MIN_WIDTH || opts.height < MIN_HEIGHT {
        return Err(BuildError::TooSmall {
            width: opts.width,
            height: opts.height,
        });
    }
    let tod = recipe
        .tod_for(opts.theme)
        .ok_or(BuildError::ThemeLocked(opts.theme))?;
    let family_id = match opts.theme {
        Theme::Light => &recipe.palette.light,
        Theme::Dark => &recipe.palette.dark,
    };
    let family =
        *palette::family(family_id).ok_or_else(|| BuildError::UnknownPalette(family_id.clone()))?;
    let activity = match &recipe.activity {
        Some(id) => Some(
            catalog
                .get(id)
                .ok_or_else(|| BuildError::UnknownActivity(id.clone()))?,
        ),
        None => None,
    };
    if !terrain::PAINTED.contains(&recipe.archetype.as_str()) {
        return Err(BuildError::UnknownArchetype(recipe.archetype.clone()));
    }
    Ok(build_scene(recipe, activity, opts, tod, family))
}

pub use crate::focal::sprite_scale;

fn tod_glow(tod: TimeOfDay) -> f64 {
    match tod {
        TimeOfDay::Night => 1.0,
        TimeOfDay::DeepDusk => 0.95,
        TimeOfDay::Dusk => 0.85,
        TimeOfDay::GoldenHour => 0.7,
        TimeOfDay::Morning => 0.55,
        TimeOfDay::Noon => 0.5,
    }
}

/// The shared scene pipeline. The archetype only paints its background layers
/// and its platform (`terrain`); everything else is the same for every scene.
fn build_scene(
    recipe: &SceneRecipe,
    activity: Option<&Activity>,
    opts: &BuildOptions,
    tod: TimeOfDay,
    family: PaletteFamily,
) -> SceneGraph {
    // Version 4 brought the calm particle pace; version 3 recipes keep the
    // original one, so they come out exactly as they did.
    let pace = if recipe.generator_version >= 4 {
        CALM_PACE
    } else {
        0
    };
    let (w, h) = (opts.width, opts.height);
    let (wf, hf) = (f64::from(w), f64::from(h));
    let aspect = wf / hf;
    // Object sizes follow the short side; layout fractions follow width/height.
    let unit = wf.min(hf);
    let grid = grid_spec(w, h);
    let comp = &recipe.composition;
    let seed = recipe.seed;
    let (cols, rows) = (grid.cols, grid.rows);
    let mut cells = vec![Cell::default(); (cols * rows) as usize];

    // ---- Focal group from the activity: character track, props, light. ----
    let horizon = pm(comp.horizon_pm);
    let platform_row = ((horizon * f64::from(rows)).round() as u32).clamp(rows / 3, rows - 4);
    let ground_y = (grid.origin_y + platform_row * grid.cell_h) as i32;
    let focal = focal::layout(recipe, activity, w, h, ground_y, grid.cell_w, grid.cell_h);
    let (gx0, gx1, group_h) = (focal.x0, focal.x1, focal.height);
    let group_cx = f64::from(gx0 + gx1) * 0.5;

    let ctx = terrain::Ctx {
        recipe,
        co: terrain::Coords {
            g: grid,
            w: wf,
            h: hf,
        },
        aspect,
        unit,
        cols,
        rows,
        horizon,
        platform_row,
        terrain: hash_label(seed, "terrain"),
        gx0,
        gx1,
        group_cx,
        tod,
    };
    let co = &ctx.co;

    // ---- The archetype's background layers, painted back to front. ----
    let sky_limit = terrain::background(&recipe.archetype, &ctx, &mut cells);

    // ---- Breathing room: a dome over the focal group, cleared then dimmed. ----
    let dome_cx = (group_cx - f64::from(grid.origin_x)) / f64::from(grid.cell_w);
    let dome_a = f64::from(gx1 - gx0) * 0.5 / f64::from(grid.cell_w) + 3.0;
    let dome_b = f64::from(group_h) / f64::from(grid.cell_h) + 2.0;
    let dome = |col: u32, row: u32| -> f64 {
        let u = (f64::from(col) + 0.5 - dome_cx) / dome_a;
        let v = (f64::from(row) + 0.5 - f64::from(platform_row)) / dome_b;
        let (u2, v2) = (u * u, v * v);
        u2 * u2 + v2 * v2
    };
    for row in 0..platform_row {
        for col in 0..cols {
            let s = dome(col, row);
            let cell = &mut cells[(row * cols + col) as usize];
            if s <= 1.0 {
                *cell = Cell::default();
            } else if s <= 2.4 {
                cell.shade = (f64::from(cell.shade) * 0.55).round() as u8;
            }
        }
    }

    // ---- The archetype's platform under the focal group. ----
    terrain::platform(&recipe.archetype, &ctx, &mut cells);
    let water = (recipe.archetype == "shore").then(|| terrain::shore::open_water(&ctx));

    // ---- Sky: stars at night, faint dust by day. Normalised positions, so the
    // same stars appear at every resolution. ----
    let (per_aspect, set, lo, hi): (f64, &[u8], f64, f64) = match tod {
        TimeOfDay::Night => (150.0, &glyphs::STARS, 0.25, 0.75),
        TimeOfDay::DeepDusk => (95.0, &glyphs::STARS, 0.20, 0.60),
        TimeOfDay::Dusk => (50.0, &glyphs::STARS, 0.18, 0.50),
        _ => (35.0, &glyphs::SKY_DUST, 0.12, 0.30),
    };
    let star_weights: &[u32] = if set.len() == glyphs::STARS.len() {
        &[6, 4, 4, 3, 2, 1]
    } else {
        &[4, 3, 2, 2]
    };
    let mut sky = Rng::stream(seed, "sky");
    let count = (per_aspect * aspect).round() as u32;
    for _ in 0..count {
        let x = sky.unit();
        let y = sky.unit() * horizon;
        let gi = sky.weighted_index(star_weights);
        let shade = sky.range_f64(lo, hi);
        let col = co.col_of(x * wf);
        let row = ((y * hf - f64::from(grid.origin_y)) / f64::from(grid.cell_h)).floor() as i64;
        if col < 0 || row < 0 || col >= i64::from(cols) || row >= i64::from(platform_row) {
            continue;
        }
        let (col, row) = (col as u32, row as u32);
        let idx = (row * cols + col) as usize;
        if cells[idx].glyph != EMPTY
            || co.ny(row) > sky_limit[col as usize] - 0.02
            || dome(col, row) <= 2.4
        {
            continue;
        }
        let bright = gi == set.len() - 1;
        cells[idx] = Cell {
            glyph: set[gi],
            shade: terrain::shade_u8(if bright { hi } else { shade }),
        };
    }

    // ---- Light: vignette at the centre, warm glow around a light source. ----
    let lighting = Lighting {
        vignette_cx: wf * 0.5,
        vignette_cy: hf * 0.5,
        vignette_rx: wf * 0.75,
        vignette_ry: hf * 0.75,
        glow: focal.light.map(|l| Glow {
            cx: l.cx,
            cy: l.cy,
            radius: 0.417 * unit * tod_glow(tod) * l.strength,
        }),
    };

    // ---- Embers rising from a flame tip. The hero time comes from the seed's
    // ember set either way, so it doesn't depend on the activity's props. ----
    let embers = ember_set(seed, pace);
    let t_hero_ms = hero_time(&embers);
    let emitters = focal
        .embers_at
        .map(|(x, y)| Emitter {
            kind: EmitterKind::Embers,
            origin_x: x,
            origin_y: y,
            unit,
            intensity: match opts.theme {
                Theme::Dark => 1.0,
                Theme::Light => 0.85,
            },
            embers,
            drops: Vec::new(),
            size: (wf, hf),
            pace,
        })
        .into_iter()
        .collect();
    // ---- Weather across the whole screen (fireflies only after dark). ----
    let mut emitters: Vec<Emitter> = emitters;
    if let Some(kind) = WeatherKind::from_id(&recipe.weather)
        && (kind != WeatherKind::Fireflies
            || matches!(
                tod,
                TimeOfDay::Dusk | TimeOfDay::DeepDusk | TimeOfDay::Night
            ))
    {
        emitters.push(Emitter {
            kind: EmitterKind::Weather(kind),
            origin_x: 0.0,
            origin_y: 0.0,
            unit,
            intensity: 1.0,
            embers: Vec::new(),
            drops: weather_set(kind, seed, aspect, pace),
            size: (wf, hf),
            pace,
        });
    }

    SceneGraph {
        width: w,
        height: h,
        theme: opts.theme,
        tod,
        buddy: opts.buddy,
        seed: recipe.seed,
        family,
        grid,
        cells,
        lighting,
        activity: recipe.activity.clone(),
        props: focal.props,
        unattended: focal.unattended,
        character: focal.character,
        emitters,
        ground_y,
        t_hero_ms,
        loop_ms: LOOP_MS,
        pace,
        generator: recipe.generator_version,
        water,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::compose;
    use crate::graph::SpriteKind;
    use crate::recipe::{Aspect, ContentContext};

    fn graph(seed: u64, w: u32, h: u32, theme: Theme, buddy: bool) -> SceneGraph {
        let r = compose(seed, Aspect::from_size(w, h), &ContentContext::default());
        // A theme-locked scene (night or day only) builds in the theme it has.
        let theme = if r.tod_for(theme).is_some() {
            theme
        } else {
            match theme {
                Theme::Dark => Theme::Light,
                Theme::Light => Theme::Dark,
            }
        };
        build(
            &r,
            &BuildOptions {
                width: w,
                height: h,
                theme,
                buddy,
            },
        )
        .unwrap()
    }

    #[test]
    fn grid_matches_plan_at_4k() {
        let g = grid_spec(3840, 2160);
        assert_eq!((g.cell_w, g.cell_h), (11, 20));
        assert_eq!(g.rows, 108);
        let small = grid_spec(640, 360);
        assert_eq!((small.cell_w, small.cell_h), (6, 11));
    }

    #[test]
    fn buddy_toggle_keeps_landscape() {
        let a = graph(7, 960, 540, Theme::Dark, true);
        let b = graph(7, 960, 540, Theme::Dark, false);
        assert_eq!(a.cells, b.cells);
        assert_eq!(a.lighting, b.lighting);
        assert_ne!(a.sprites_at(0), b.sprites_at(0));
    }

    #[test]
    fn feet_on_platform_top_at_all_times() {
        for seed in 0..30 {
            // Dark: every activity has a dark variant.
            let g = graph(seed, 1280, 720, Theme::Dark, true);
            let archetype = compose(
                seed,
                Aspect::from_size(1280, 720),
                &ContentContext::default(),
            )
            .archetype;
            let row = ((g.ground_y as u32) - g.grid.origin_y) / g.grid.cell_h;
            for t in (0..20_000).step_by(1_250) {
                let c = g
                    .sprites_at(t)
                    .into_iter()
                    .find(|s| s.kind == SpriteKind::Character)
                    .unwrap();
                if g.character.is_some_and(|t| t.flies()) {
                    // Off the ground (in the air, on the water): on screen.
                    assert!(c.y >= 0 && c.y + c.height() as i32 <= g.height as i32);
                    continue;
                }
                assert_eq!(c.y + c.height() as i32, g.ground_y);
                assert!(c.x >= 0 && c.x + c.width() as i32 <= g.width as i32);
                // The platform's top row is under the character's feet.
                let col = ((c.x + c.width() as i32 / 2) as u32 - g.grid.origin_x) / g.grid.cell_w;
                assert_eq!(
                    g.cell(col, row).glyph,
                    terrain::platform_top(&archetype),
                    "seed {seed} t {t}"
                );
            }
        }
    }

    #[test]
    fn theme_locked_scenes_refuse_the_other_theme() {
        let ctx = ContentContext {
            enabled_activities: vec!["stargaze".into()],
            ..ContentContext::default()
        };
        let r = compose(4, Aspect::from_size(16, 9), &ctx);
        let opts = BuildOptions {
            width: 640,
            height: 360,
            theme: Theme::Light,
            buddy: true,
        };
        assert_eq!(
            build(&r, &opts).unwrap_err(),
            BuildError::ThemeLocked(Theme::Light)
        );
        assert!(
            build(
                &r,
                &BuildOptions {
                    theme: Theme::Dark,
                    ..opts
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn props_never_overlap_the_character() {
        for seed in 0..30 {
            let g = graph(seed, 1920, 1080, Theme::Dark, true);
            let track = g.character.unwrap();
            if track.flies() {
                continue; // above the props, not beside them
            }
            let (lo, hi) = track.x_range();
            let cw = track.at(0).width() as i32;
            for p in &g.props {
                let (px0, px1) = (p.x, p.x + p.width() as i32);
                assert!(
                    px1 <= lo || px0 >= hi + cw,
                    "seed {seed}: {:?} overlaps the character's range",
                    p.kind
                );
            }
        }
    }

    #[test]
    fn rejects_tiny_sizes_and_unknown_archetypes() {
        let mut r = compose(1, Aspect::from_size(16, 9), &ContentContext::default());
        // A theme the scene has (day-only and night-only activities exist).
        let theme = if r.tod_for(Theme::Dark).is_some() {
            Theme::Dark
        } else {
            Theme::Light
        };
        let opts = BuildOptions {
            width: 100,
            height: 50,
            theme,
            buddy: true,
        };
        assert!(matches!(build(&r, &opts), Err(BuildError::TooSmall { .. })));
        r.archetype = "volcano".into();
        let opts = BuildOptions {
            width: 640,
            height: 360,
            ..opts
        };
        assert!(matches!(
            build(&r, &opts),
            Err(BuildError::UnknownArchetype(_))
        ));
    }
}
