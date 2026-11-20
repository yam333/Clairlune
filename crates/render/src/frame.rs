//! Frame compositing: the baked background plus animated elements at time `t`.
//! Particles snap to the cell grid (exact palette, small GIF diffs); sprites use
//! integer nearest-neighbour scaling.
//!
//! The activity, character and prop art come through [`SceneArt`], so the same
//! code plays live scenes (built-in art) and saved ones (their own copy).
//!
//! Every draw records the rectangle it writes, so [`IncrementalFrame`] can make
//! the next frame from the last one by restoring and redrawing only those
//! rectangles (the live wallpaper, PLAN.md section 5.3), and report what changed.

use crate::IndexedImage;
use crate::art::{BuiltinArt, SceneArt};
use crate::atlas::GlyphAtlas;
use crate::clock::{Clock, LoopScript, loop_track};
use crate::palette::{IndexedPalette, LEVELS, ink_level};
use clairlune_scene::anim::state_at;
use clairlune_scene::graph::CHARACTER_BOX;
use clairlune_scene::particles::{Preset, preset_particles};
use clairlune_scene::{SceneGraph, SpriteInstance, SpriteKind};
use clairlune_sprites::Frame;

/// Composites one frame at `t_ms` over a baked background (live clock).
pub fn render_frame(
    background: &IndexedImage,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    t_ms: u64,
) -> IndexedImage {
    render_frame_clocked(background, graph, atlas, palette, t_ms, Clock::Live)
}

/// Like [`render_frame`], with an explicit character clock: `Clock::Loop` makes
/// the character follow the activity's loop script for a seamless GIF.
pub fn render_frame_clocked(
    background: &IndexedImage,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    t_ms: u64,
    clock: Clock,
) -> IndexedImage {
    let art = BuiltinArt::for_graph(graph);
    render_frame_with_art(background, graph, atlas, palette, t_ms, clock, &art)
}

/// Composites one frame with the given art (built-in, or a saved scene's).
pub fn render_frame_with_art(
    background: &IndexedImage,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    t_ms: u64,
    clock: Clock,
    art: &dyn SceneArt,
) -> IndexedImage {
    let mut img = background.clone();
    let mut drawn = Vec::new();
    compose(
        &mut img, &mut drawn, graph, atlas, palette, t_ms, clock, art,
    );
    img
}

/// A rectangle of pixels, inside the image it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PixelRect {
    /// `[x0, x1) x [y0, y1)` clipped to a `width x height` image; `None` when
    /// nothing is left.
    fn clipped(x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Option<Self> {
        let (x0, y0) = (x0.max(0), y0.max(0));
        let (x1, y1) = (x1.min(width as i32), y1.min(height as i32));
        (x1 > x0 && y1 > y0).then(|| PixelRect {
            x: x0 as u32,
            y: y0 as u32,
            w: (x1 - x0) as u32,
            h: (y1 - y0) as u32,
        })
    }

    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// A frame kept between calls: each new time redraws only what the last
/// frame and this one draw over the background. The image always equals
/// [`render_frame_with_art`] at the last time given.
#[derive(Clone, Debug)]
pub struct IncrementalFrame {
    image: IndexedImage,
    /// What the frame drew over the background (restored before the next).
    drawn: Vec<PixelRect>,
}

impl IncrementalFrame {
    /// Starts as the bare background (nothing drawn yet).
    pub fn new(background: &IndexedImage) -> Self {
        IncrementalFrame {
            image: background.clone(),
            drawn: Vec::new(),
        }
    }

    pub fn image(&self) -> &IndexedImage {
        &self.image
    }

    /// Moves to `t_ms` and returns the rectangles whose pixels may have
    /// changed (sorted, without repeats): the only ones a copy of the last
    /// frame needs, such as a GPU texture.
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        background: &IndexedImage,
        graph: &SceneGraph,
        atlas: &GlyphAtlas,
        palette: &IndexedPalette,
        t_ms: u64,
        clock: Clock,
        art: &dyn SceneArt,
    ) -> Vec<PixelRect> {
        debug_assert_eq!(
            (background.width, background.height),
            (self.image.width, self.image.height)
        );
        let w = self.image.width as usize;
        for r in &self.drawn {
            for y in r.y..r.y + r.h {
                let a = y as usize * w + r.x as usize;
                let b = a + r.w as usize;
                self.image.pixels[a..b].copy_from_slice(&background.pixels[a..b]);
            }
        }
        let mut drawn = Vec::with_capacity(self.drawn.len());
        compose(
            &mut self.image,
            &mut drawn,
            graph,
            atlas,
            palette,
            t_ms,
            clock,
            art,
        );
        let mut changed: Vec<PixelRect> = self.drawn.iter().chain(&drawn).copied().collect();
        changed.sort_unstable();
        changed.dedup();
        self.drawn = drawn;
        changed
    }
}

/// Draws the animated elements at `t_ms` over `img` (the background, or a
/// frame restored to it) and records each rectangle written in `drawn`.
#[allow(clippy::too_many_arguments)]
fn compose(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    t_ms: u64,
    clock: Clock,
    art: &dyn SceneArt,
) {
    if clock == Clock::Smooth {
        for p in graph.particles_in_grid(t_ms) {
            draw_particle_at(img, drawn, graph, atlas, p.x, p.y, p.glyph, p.level, p.ink);
        }
    } else {
        for (col, row, p) in graph.particles_at(t_ms) {
            draw_particle(
                img, drawn, graph, atlas, col, row, p.glyph, p.level, p.ink, false,
            );
        }
    }
    for sprite in graph.sprites_at(t_ms) {
        if sprite.kind == SpriteKind::Character {
            let sprite = match (clock, graph.character) {
                (Clock::Loop(loop_ms), Some(track)) => loop_track(track, loop_ms).at(t_ms),
                _ => sprite,
            };
            if let Some(pose) = character_pose(graph, art, &sprite, t_ms, clock) {
                // Effects go on top as an overlay (only the glyph's own pixels),
                // so steam over a mug or sparkles over the hat stay visible.
                draw_pose(img, drawn, &pose, palette);
                draw_effects(img, drawn, graph, art, atlas, &pose, t_ms, clock);
            }
            continue;
        }
        draw_sprite(img, drawn, &sprite, art, palette);
    }
}

/// Draws a particle glyph in ember colour, or ink colour when `ink` (faint
/// wisps such as steam). Normally it replaces its whole cell (background band,
/// then the glyph); with `overlay` only the glyph's pixels are written.
#[allow(clippy::too_many_arguments)]
fn draw_particle(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    col: u32,
    row: u32,
    glyph: u8,
    level: u8,
    ink: bool,
    overlay: bool,
) {
    let g = &graph.grid;
    let shade =
        ((u32::from(level.min(LEVELS)) * 255 + u32::from(LEVELS) / 2) / u32::from(LEVELS)) as u8;
    let mask = atlas.mask(glyph);
    let (x0, y0) = (g.origin_x + col * g.cell_w, g.origin_y + row * g.cell_h);
    // Cells lie inside the image (the grid is laid out within it).
    drawn.push(PixelRect {
        x: x0,
        y: y0,
        w: g.cell_w,
        h: g.cell_h,
    });
    let w = img.width as usize;
    for my in 0..g.cell_h {
        let y = y0 + my;
        let rt = graph.lighting.row_terms(y);
        for mx in 0..g.cell_w {
            let x = x0 + mx;
            let band = graph.lighting.band_in_row(&rt, x);
            let lvl = ink_level(mask[(my * g.cell_w + mx) as usize], shade);
            if overlay && lvl == 0 {
                continue; // overlay: leave whatever is under the glyph
            }
            img.pixels[y as usize * w + x as usize] = match (lvl > 0, ink) {
                (false, _) => band,
                (true, false) => IndexedPalette::ember(band, lvl),
                (true, true) => IndexedPalette::ink(band, lvl),
            };
        }
    }
}

/// Draws a particle glyph centred on pixel `(cx, cy)` (`Clock::Smooth`): only
/// the glyph's own pixels, over whatever is there, clipped to the image.
#[allow(clippy::too_many_arguments)]
fn draw_particle_at(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    cx: f64,
    cy: f64,
    glyph: u8,
    level: u8,
    ink: bool,
) {
    let g = &graph.grid;
    let shade =
        ((u32::from(level.min(LEVELS)) * 255 + u32::from(LEVELS) / 2) / u32::from(LEVELS)) as u8;
    let mask = atlas.mask(glyph);
    let x0 = (cx - f64::from(g.cell_w) / 2.0).round() as i32;
    let y0 = (cy - f64::from(g.cell_h) / 2.0).round() as i32;
    let x1 = x0 + g.cell_w as i32;
    let y1 = y0 + g.cell_h as i32;
    let Some(rect) = PixelRect::clipped(x0, y0, x1, y1, img.width, img.height) else {
        return;
    };
    drawn.push(rect);
    let w = img.width as usize;
    for y in rect.y..rect.y + rect.h {
        let my = (y as i32 - y0) as u32;
        let rt = graph.lighting.row_terms(y);
        for x in rect.x..rect.x + rect.w {
            let mx = (x as i32 - x0) as u32;
            let lvl = ink_level(mask[(my * g.cell_w + mx) as usize], shade);
            if lvl == 0 {
                continue;
            }
            let band = graph.lighting.band_in_row(&rt, x);
            img.pixels[y as usize * w + x as usize] = if ink {
                IndexedPalette::ink(band, lvl)
            } else {
                IndexedPalette::ember(band, lvl)
            };
        }
    }
}

/// The still-frame time used for stills, thumbnails and the static wallpaper:
/// the first moment at or after the scene's ember-based hero time (50 ms steps,
/// up to 10 s) when the character shows the first frame of its activity's start
/// state, so a still never catches a blink or a mid-gesture. Independent of the
/// Buddy toggle, so toggling it keeps the same moment.
pub fn hero_time(graph: &SceneGraph) -> u64 {
    hero_time_with_art(graph, &BuiltinArt::for_graph(graph))
}

/// [`hero_time`] with the given art.
pub fn hero_time_with_art(graph: &SceneGraph, art: &dyn SceneArt) -> u64 {
    let base = graph.t_hero_ms;
    let (Some(a), Some(_)) = (art.activity(), graph.character) else {
        return base;
    };
    let character = art.character();
    for k in 0..200u64 {
        let t = base + k * 50;
        let st = state_at(a, graph.seed, t, &|s| character.one_shot_ms(&a.id, s));
        if st.state != a.animation.start {
            continue;
        }
        let looping = a.animation.states.get(&st.state).is_some_and(|d| d.looping);
        if let Some(anim) = character.animation(&a.id, &st.state)
            && anim.frame_index_at(t - st.since_ms, looping) == 0
        {
            return t;
        }
    }
    base
}

/// The character's current frame and where it sits on screen: the art's origin
/// (feet row, body centre) aligned inside the layout box, mirrored when it faces
/// left. All character art comes through `Character`.
struct Pose<'a> {
    frame: &'a Frame,
    state: String,
    /// Screen pixel of the origin pixel's top-left corner.
    x0: i32,
    y0: i32,
    sc: i32,
    mirror: bool,
}

impl Pose<'_> {
    /// Screen position of the centre of frame pixel `(col, row)`.
    fn screen(&self, col: i32, row: i32) -> (i32, i32) {
        let (ox, oy) = self.frame.origin();
        let dx = (col - ox) * if self.mirror { -1 } else { 1 };
        (
            self.x0 + dx * self.sc + self.sc / 2,
            self.y0 + (row - oy) * self.sc + self.sc / 2,
        )
    }
}

/// Picks the frame for the activity state at `t` (state machine when live, the
/// loop script when looping) and places it.
fn character_pose<'a>(
    graph: &SceneGraph,
    art: &'a dyn SceneArt,
    s: &SpriteInstance,
    t_ms: u64,
    clock: Clock,
) -> Option<Pose<'a>> {
    let character = art.character();
    let (anim, state, elapsed, looping) = match (art.activity(), clock) {
        (Some(a), Clock::Loop(loop_ms)) => {
            let script = LoopScript::new(a, character, loop_ms);
            let (state, elapsed) = script.at(t_ms);
            (
                character.animation(&a.id, state),
                state.to_string(),
                elapsed,
                true,
            )
        }
        (Some(a), Clock::Live | Clock::Smooth) => {
            let st = state_at(a, graph.seed, t_ms, &|state| {
                character.one_shot_ms(&a.id, state)
            });
            let looping = a.animation.states.get(&st.state).is_some_and(|d| d.looping);
            (
                character.animation(&a.id, &st.state),
                st.state.clone(),
                t_ms - st.since_ms,
                looping,
            )
        }
        (None, _) => (
            character.animation("", "idle"),
            "idle".to_string(),
            t_ms,
            true,
        ),
    };
    let frame = anim?.frame_at(elapsed, looping);
    let sc = s.scale as i32;
    Some(Pose {
        frame,
        state,
        x0: s.x + (CHARACTER_BOX.0 as i32 / 2) * sc,
        y0: s.y + (CHARACTER_BOX.1 as i32 - 1) * sc,
        sc,
        mirror: s.mirror,
    })
}

/// The activity's `[[effects]]` for the current state, at their anchors on the
/// current frame. Particles snap to cells like the embers, except with
/// `Clock::Smooth`.
#[allow(clippy::too_many_arguments)]
fn draw_effects(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    art: &dyn SceneArt,
    atlas: &GlyphAtlas,
    pose: &Pose,
    t_ms: u64,
    clock: Clock,
) {
    let Some(a) = art.activity() else {
        return;
    };
    let g = &graph.grid;
    let dir = if pose.mirror { -1.0 } else { 1.0 };
    for (i, e) in a.effects.iter().enumerate() {
        if !e.active_in(&pose.state) {
            continue;
        }
        let (Some(&(ax, ay)), Some(preset)) =
            (pose.frame.anchors.get(&e.at), Preset::from_id(&e.preset))
        else {
            continue;
        };
        let (cx, cy) = pose.screen(ax, ay);
        let seed = graph.seed.wrapping_add(i as u64 + 1);
        for p in preset_particles(preset, seed, t_ms, graph.pace) {
            let x = f64::from(cx) + p.dx * dir * f64::from(pose.sc);
            let y = f64::from(cy) + p.dy * f64::from(pose.sc);
            if clock == Clock::Smooth {
                let (gx0, gy0) = (f64::from(g.origin_x), f64::from(g.origin_y));
                let inside = x >= gx0
                    && y >= gy0
                    && x < gx0 + f64::from(g.cols * g.cell_w)
                    && y < gy0 + f64::from(g.rows * g.cell_h);
                if inside {
                    let steam = preset == Preset::Steam;
                    draw_particle_at(img, drawn, graph, atlas, x, y, p.glyph, p.level, steam);
                }
                continue;
            }
            let col = ((x - f64::from(g.origin_x)) / f64::from(g.cell_w)).floor();
            let row = ((y - f64::from(g.origin_y)) / f64::from(g.cell_h)).floor();
            if col < 0.0 || row < 0.0 || col >= f64::from(g.cols) || row >= f64::from(g.rows) {
                continue;
            }
            draw_particle(
                img,
                drawn,
                graph,
                atlas,
                col as u32,
                row as u32,
                p.glyph,
                p.level,
                preset == Preset::Steam,
                true,
            );
        }
    }
}

fn draw_pose(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    pose: &Pose,
    palette: &IndexedPalette,
) {
    let f = pose.frame;
    let (ox, oy) = f.origin();
    let (x0, y0, sc, s) = (pose.x0, pose.y0, pose.sc, pose);
    let (w, h) = (img.width as i32, img.height as i32);
    // The frame's box on screen (mirroring flips it around the origin).
    let (fw, fh) = (f.width as i32, f.height as i32);
    let (left, right) = if s.mirror {
        (x0 - (fw - 1 - ox) * sc, x0 + (ox + 1) * sc)
    } else {
        (x0 - ox * sc, x0 + (fw - ox) * sc)
    };
    if let Some(r) = PixelRect::clipped(
        left,
        y0 - oy * sc,
        right,
        y0 + (fh - oy) * sc,
        img.width,
        img.height,
    ) {
        drawn.push(r);
    }
    for row in 0..f.height {
        for col in 0..f.width {
            let Some(rgb) = f.pixel(col, row) else {
                continue;
            };
            let idx = palette
                .sprite_index(rgb)
                .expect("character colour in palette");
            let dx = (col as i32 - ox) * if s.mirror { -1 } else { 1 };
            let px = x0 + dx * sc;
            let py = y0 + (row as i32 - oy) * sc;
            for y in py.max(0)..(py + sc).min(h) {
                let line = (y * w) as usize;
                for x in px.max(0)..(px + sc).min(w) {
                    img.pixels[line + x as usize] = idx;
                }
            }
        }
    }
}

fn draw_sprite(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    s: &SpriteInstance,
    art: &dyn SceneArt,
    palette: &IndexedPalette,
) {
    let Some(map) = art.prop(s.kind) else {
        return;
    };
    let (w, h) = (img.width as i32, img.height as i32);
    let scale = s.scale as i32;
    let (mw, mh) = (map.width(), map.height());
    if let Some(r) = PixelRect::clipped(
        s.x,
        s.y,
        s.x + mw as i32 * scale,
        s.y + mh as i32 * scale,
        img.width,
        img.height,
    ) {
        drawn.push(r);
    }
    for my in 0..mh {
        for mx in 0..mw {
            let sx = if s.mirror { mw - 1 - mx } else { mx };
            let Some(rgb) = map.pixel(sx, my) else {
                continue;
            };
            let idx = palette.sprite_index(rgb).expect("sprite colour in palette");
            let px = s.x + mx as i32 * scale;
            let py = s.y + my as i32 * scale;
            for y in py.max(0)..(py + scale).min(h) {
                let line = (y * w) as usize;
                for x in px.max(0)..(px + scale).min(w) {
                    img.pixels[line + x as usize] = idx;
                }
            }
        }
    }
}
