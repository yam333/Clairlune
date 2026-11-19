//! Frame compositing: the baked background plus animated elements at time `t`.
//! Particles snap to the cell grid (exact palette, small GIF diffs); sprites use
//! integer nearest-neighbour scaling.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::clock::{Clock, LoopScript, loop_track};
use crate::palette::{IndexedPalette, LEVELS, ink_level};
use clairlune_scene::activity::builtin_catalog;
use clairlune_scene::anim::state_at;
use clairlune_scene::graph::CHARACTER_BOX;
use clairlune_scene::particles::{Preset, preset_particles};
use clairlune_scene::{SceneGraph, SpriteInstance, SpriteKind};
use clairlune_sprites::{Character, Frame, builtin_character};

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
    let mut img = background.clone();
    for (col, row, p) in graph.particles_at(t_ms) {
        draw_particle(
            &mut img, graph, atlas, col, row, p.glyph, p.level, p.ink, false,
        );
    }
    for sprite in graph.sprites_at(t_ms) {
        if sprite.kind == SpriteKind::Character {
            let sprite = match (clock, graph.character) {
                (Clock::Loop(loop_ms), Some(track)) => loop_track(track, loop_ms).at(t_ms),
                _ => sprite,
            };
            if let Some(pose) = character_pose(graph, &sprite, t_ms, clock) {
                // Effects go on top as an overlay (only the glyph's own pixels),
                // so steam over a mug or sparkles over the hat stay visible.
                draw_pose(&mut img, &pose, palette);
                draw_effects(&mut img, graph, atlas, &pose, t_ms);
            }
            continue;
        }
        draw_sprite(&mut img, &sprite, palette);
    }
    img
}

/// Draws a particle glyph in ember colour, or ink colour when `ink` (faint
/// wisps such as steam). Normally it replaces its whole cell (background band,
/// then the glyph); with `overlay` only the glyph's pixels are written.
#[allow(clippy::too_many_arguments)]
fn draw_particle(
    img: &mut IndexedImage,
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

/// The still-frame time used for stills, thumbnails and the static wallpaper:
/// the first moment at or after the scene's ember-based hero time (50 ms steps,
/// up to 10 s) when the character shows the first frame of its activity's start
/// state, so a still never catches a blink or a mid-gesture. Independent of the
/// Buddy toggle, so toggling it keeps the same moment.
pub fn hero_time(graph: &SceneGraph) -> u64 {
    let base = graph.t_hero_ms;
    let activity = graph
        .activity
        .as_deref()
        .and_then(|id| builtin_catalog().get(id));
    let (Some(a), Some(_)) = (activity, graph.character) else {
        return base;
    };
    let character = builtin_character();
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
struct Pose {
    frame: &'static Frame,
    state: String,
    /// Screen pixel of the origin pixel's top-left corner.
    x0: i32,
    y0: i32,
    sc: i32,
    mirror: bool,
}

impl Pose {
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
fn character_pose(graph: &SceneGraph, s: &SpriteInstance, t_ms: u64, clock: Clock) -> Option<Pose> {
    let character = builtin_character();
    let activity = graph
        .activity
        .as_deref()
        .and_then(|id| builtin_catalog().get(id));
    let (anim, state, elapsed, looping) = match (activity, clock) {
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
        (Some(a), Clock::Live) => {
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
/// current frame. Particles snap to cells like the embers.
fn draw_effects(
    img: &mut IndexedImage,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    pose: &Pose,
    t_ms: u64,
) {
    let Some(a) = graph
        .activity
        .as_deref()
        .and_then(|id| builtin_catalog().get(id))
    else {
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
        for p in preset_particles(preset, seed, t_ms) {
            let x = f64::from(cx) + p.dx * dir * f64::from(pose.sc);
            let y = f64::from(cy) + p.dy * f64::from(pose.sc);
            let col = ((x - f64::from(g.origin_x)) / f64::from(g.cell_w)).floor();
            let row = ((y - f64::from(g.origin_y)) / f64::from(g.cell_h)).floor();
            if col < 0.0 || row < 0.0 || col >= f64::from(g.cols) || row >= f64::from(g.rows) {
                continue;
            }
            draw_particle(
                img,
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

fn draw_pose(img: &mut IndexedImage, pose: &Pose, palette: &IndexedPalette) {
    let f = pose.frame;
    let (ox, oy) = f.origin();
    let (x0, y0, sc, s) = (pose.x0, pose.y0, pose.sc, pose);
    let (w, h) = (img.width as i32, img.height as i32);
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

fn draw_sprite(img: &mut IndexedImage, s: &SpriteInstance, palette: &IndexedPalette) {
    let map = s.kind.map();
    let (w, h) = (img.width as i32, img.height as i32);
    let scale = s.scale as i32;
    for my in 0..map.height {
        for mx in 0..map.width {
            let sx = if s.mirror { map.width - 1 - mx } else { mx };
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
