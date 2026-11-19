//! `build(recipe, size, theme, buddy) -> SceneGraph`.
//!
//! All layout math uses normalised coordinates (0..1) sampled at cell centres, so
//! the same recipe gives the same landscape at any resolution; only the cell
//! sampling changes. Buddy is a render flag: it changes sprites, never cells.

use crate::activity::{Activity, builtin_catalog};
use crate::focal;
use crate::glyphs::{self, EMPTY};
use crate::graph::{Cell, Glow, GridSpec, Lighting, SceneGraph, grid_spec};
use crate::hash::{hash_label, hash3, unit_from_hash};
use crate::math::{clamp01, lerp, pm, pow, smoothstep};
use crate::noise::{fbm1, fbm2, ridged1};
use crate::palette::{self, PaletteFamily, Theme};
use crate::particles::{Emitter, EmitterKind, LOOP_MS, ember_set, hero_time};
use crate::recipe::{GENERATOR_VERSION, SceneRecipe, TimeOfDay};
use crate::rng::Rng;
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
    if recipe.generator_version != GENERATOR_VERSION {
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
            builtin_catalog()
                .get(id)
                .ok_or_else(|| BuildError::UnknownActivity(id.clone()))?,
        ),
        None => None,
    };
    match recipe.archetype.as_str() {
        "canyon" => Ok(canyon(recipe, activity, opts, tod, family)),
        other => Err(BuildError::UnknownArchetype(other.to_string())),
    }
}

pub use crate::focal::sprite_scale;

/// Normalised cell-centre coordinates.
struct Coords {
    g: GridSpec,
    w: f64,
    h: f64,
}

impl Coords {
    fn nx(&self, col: u32) -> f64 {
        (f64::from(self.g.origin_x) + (f64::from(col) + 0.5) * f64::from(self.g.cell_w)) / self.w
    }
    fn ny(&self, row: u32) -> f64 {
        (f64::from(self.g.origin_y) + (f64::from(row) + 0.5) * f64::from(self.g.cell_h)) / self.h
    }
    /// Column containing pixel x (may be outside the grid).
    fn col_of(&self, px: f64) -> i64 {
        ((px - f64::from(self.g.origin_x)) / f64::from(self.g.cell_w)).floor() as i64
    }
}

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

fn shade_u8(s: f64) -> u8 {
    (clamp01(s) * 255.0).round() as u8
}

/// Picks a glyph from a light-to-heavy family by density, with a seeded jitter of
/// up to ±0.6 of a step so it doesn't band.
fn family_glyph(
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

/// The canyon archetype: cliffs left and right, an open centre, and a rock ledge
/// in the gap where the focal group stands. Ported from the prototype's description.
fn canyon(
    recipe: &SceneRecipe,
    activity: Option<&Activity>,
    opts: &BuildOptions,
    tod: TimeOfDay,
    family: PaletteFamily,
) -> SceneGraph {
    let (w, h) = (opts.width, opts.height);
    let (wf, hf) = (f64::from(w), f64::from(h));
    let aspect = wf / hf;
    // Object sizes follow the short side; layout fractions follow width/height.
    let unit = wf.min(hf);
    let grid = grid_spec(w, h);
    let co = Coords {
        g: grid,
        w: wf,
        h: hf,
    };
    let comp = &recipe.composition;
    let seed = recipe.seed;
    let terrain = hash_label(seed, "terrain");
    let (cols, rows) = (grid.cols, grid.rows);
    let mut cells = vec![Cell::default(); (cols * rows) as usize];

    // ---- Focal group from the activity: character track, props, light. ----
    let horizon = pm(comp.horizon_pm);
    let platform_row = ((horizon * f64::from(rows)).round() as u32).clamp(rows / 3, rows - 4);
    let ground_y = (grid.origin_y + platform_row * grid.cell_h) as i32;
    let focal = focal::layout(recipe, activity, w, h, ground_y, grid.cell_w);
    let (gx0, gx1, group_h) = (focal.x0, focal.x1, focal.height);
    let group_cx = f64::from(gx0 + gx1) * 0.5;

    // ---- Cliff layers, painted back to front. ----
    let center = 0.5 + (group_cx / wf - 0.5) * 0.6;
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
    let mut sky_limit = vec![f64::INFINITY; cols as usize];
    let jitter_seed = hash_label(terrain, "jitter");
    for layer in &layers {
        for col in 0..cols {
            let nx = co.nx(col);
            let top = layer.top(nx, center, power, aspect);
            sky_limit[col as usize] = sky_limit[col as usize].min(top);
            let family = if nx < center { layer.left } else { layer.right };
            for row in 0..rows {
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
                let rows_below_top = depth * f64::from(rows);
                let mut shade = layer.shade * (0.55 + 0.45 * (rows_below_top / 6.0).min(1.0));
                if row > platform_row {
                    shade *= 0.5; // ledges below the platform line recede
                }
                cells[(row * cols + col) as usize] = Cell {
                    glyph,
                    shade: shade_u8(shade),
                };
            }
        }
    }

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

    // ---- Platform: a rock ledge rising from the bottom, widening downward. ----
    let platform_seed = hash_label(terrain, "platform");
    let margin_px = pm(comp.platform_margin_pm) * unit + 2.0 * f64::from(grid.cell_w);
    let pc0 = co.col_of(f64::from(gx0) - margin_px);
    let pc1 = co.col_of(f64::from(gx1) + margin_px);
    let span_rows = f64::from(rows - platform_row);
    for row in platform_row..rows {
        let k = f64::from(row - platform_row);
        let widen = if k < 3.0 {
            0.6 * k
        } else {
            1.8 + 0.25 * (k - 3.0)
        };
        // Edge noise fades in over the first rows, so the top always spans the group.
        let rough = 9.0 * (k / 3.0).min(1.0);
        let el = (fbm1(hash_label(platform_seed, "left"), k * 0.22, 3) - 0.5) * rough;
        let er = (fbm1(hash_label(platform_seed, "right"), k * 0.22, 3) - 0.5) * rough;
        let c0 = (pc0 as f64 - widen - el).round() as i64;
        let c1 = (pc1 as f64 + widen + er).round() as i64;
        // One empty cell of separation so the ledge reads as its own object.
        for c in [c0 - 1, c1 + 1] {
            if (0..i64::from(cols)).contains(&c) {
                cells[(row * cols + c as u32) as usize] = Cell::default();
            }
        }
        let fade = smoothstep(0.0, 1.0, k / span_rows);
        for c in c0.max(0)..=c1.min(i64::from(cols) - 1) {
            let col = c as u32;
            let idx = (row * cols + col) as usize;
            if k == 0.0 {
                cells[idx] = Cell {
                    glyph: glyphs::PLATFORM_TOP,
                    shade: shade_u8(0.85),
                };
                continue;
            }
            let n = fbm2(
                platform_seed,
                co.nx(col) * aspect * 9.0,
                co.ny(row) * 38.0,
                3,
            );
            // Biased toward the lighter end of the family so the ledge isn't a solid block.
            let d = clamp01((n - 0.5) * 1.8 + 0.42);
            let threshold = 0.20;
            if d < threshold && k > 1.0 {
                cells[idx] = Cell::default();
                continue;
            }
            let glyph = family_glyph(
                &glyphs::PLATFORM,
                d.max(threshold),
                threshold,
                platform_seed,
                col,
                row,
            );
            cells[idx] = Cell {
                glyph,
                shade: shade_u8(lerp(0.80, 0.40, fade)),
            };
        }
    }

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
            shade: shade_u8(if bright { hi } else { shade }),
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
    let embers = ember_set(seed);
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
        })
        .into_iter()
        .collect();

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
            let row = ((g.ground_y as u32) - g.grid.origin_y) / g.grid.cell_h;
            for t in (0..20_000).step_by(1_250) {
                let c = g
                    .sprites_at(t)
                    .into_iter()
                    .find(|s| s.kind == SpriteKind::Character)
                    .unwrap();
                assert_eq!(c.y + c.height() as i32, g.ground_y);
                assert!(c.x >= 0 && c.x + c.width() as i32 <= g.width as i32);
                // The platform's top row is under the character's feet.
                let col = ((c.x + c.width() as i32 / 2) as u32 - g.grid.origin_x) / g.grid.cell_w;
                assert_eq!(
                    g.cell(col, row).glyph,
                    glyphs::PLATFORM_TOP,
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
        let opts = BuildOptions {
            width: 100,
            height: 50,
            theme: Theme::Dark,
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
