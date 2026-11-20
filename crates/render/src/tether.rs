//! Tether effects (docs/activity-system.md section 3): a line from an anchor
//! on the character to an end that moves on its own. Everything is periodic
//! in the scene's loop length, like the other effects, so GIF loops stay
//! seamless.
//!
//! - `fishing_line`: down to a float on the water past the pier's end. The
//!   float bobs, dips while something nibbles and lifts as the line is
//!   reeled; ripples spread from it.
//! - `kite_string`: up to a kite drifting high on the open side, its tail
//!   swaying.
//!
//! Lines are drawn in the scene's ink, so they read in both themes; the
//! float and the kite use the sprite colours.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::clock::Clock;
use crate::frame::{PixelRect, draw_particle, draw_particle_at};
use crate::palette::{IndexedPalette, LEVELS};
use clairlune_scene::{Rgb, SceneGraph};
use std::f64::consts::TAU;

const RED: Rgb = Rgb::hex(0xC8452E);
const WHITE: Rgb = Rgb::hex(0xEEEAE0);
const RED_DARK: Rgb = Rgb::hex(0x933022);

/// Where the tether starts and how the character stands.
pub(crate) struct Anchor<'a> {
    pub x: i32,
    pub y: i32,
    /// Sprite scale: one art pixel is `sc` screen pixels.
    pub sc: i32,
    /// 1 when the character faces right, -1 when left.
    pub dir: i32,
    pub state: &'a str,
}

/// A sine of period `period_ms` (which divides the loop length).
fn wave(t_ms: u64, period_ms: u64, phase: f64) -> f64 {
    (TAU * (t_ms % period_ms) as f64 / period_ms as f64 + phase).sin()
}

/// Draws a tether preset; unknown presets and scenes without what the preset
/// needs (open water) draw nothing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_tether(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    preset: &str,
    a: &Anchor,
    t_ms: u64,
    clock: Clock,
) {
    match preset {
        "fishing_line" => fishing_line(img, drawn, graph, atlas, palette, a, t_ms, clock),
        "kite_string" => kite_string(img, drawn, graph, palette, a, t_ms),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn fishing_line(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    a: &Anchor,
    t_ms: u64,
    clock: Clock,
) {
    let Some((fx, fy)) = graph.water else {
        return;
    };
    // Out past the rod tip, so the line hangs forward.
    let beyond = a.x + a.dir * (graph.grid.cell_w as i32 * 3 / 2);
    let fx = if a.dir > 0 {
        fx.max(beyond)
    } else {
        fx.min(beyond)
    };
    let sc = f64::from(a.sc);
    // How far below the waterline the float's bottom sits, and how slack
    // the line hangs.
    let (dy, sag, ripple_ms) = match a.state {
        "nibble" => (sc * (0.8 + 0.8 * wave(t_ms, 500, 0.0)), 0.03, 500),
        "reel" => (-1.5 * sc, 0.0, 0),
        _ => (0.5 * sc * wave(t_ms, 2000, 0.0), 0.12, 2000),
    };
    let float_bottom = f64::from(fy) + dy;
    // The float: two art pixels wide, a white cap over a red body; the part
    // under the waterline is hidden.
    let (fw, fh) = (a.sc * 2, a.sc * 3);
    let left = fx - a.sc;
    let top = (float_bottom - f64::from(fh)).round() as i32;
    let red = palette.sprite_index(RED);
    let white = palette.sprite_index(WHITE);
    for row in 0..3 {
        let y0 = top + row * a.sc;
        let y1 = (y0 + a.sc).min(fy);
        if y1 <= y0 {
            continue;
        }
        let color = if row == 0 { white } else { red };
        fill(img, drawn, graph, left, y0, fw, y1 - y0, color);
    }
    line(
        img,
        drawn,
        graph,
        (f64::from(a.x), f64::from(a.y)),
        (f64::from(fx), f64::from(top)),
        sag,
        (a.sc / 2).max(1),
        4,
    );
    // Ripples spreading from the float.
    if ripple_ms > 0 {
        let g = &graph.grid;
        let q = (t_ms % ripple_ms) as f64 / ripple_ms as f64;
        let level = ((1.0 - q) * 4.0).round().max(1.0) as u8;
        let reach = f64::from(g.cell_w) * (0.8 + 1.6 * q);
        let y = f64::from(fy) + f64::from(g.cell_h) * 0.2;
        for side in [-1.0, 1.0] {
            let x = f64::from(fx) + side * reach;
            glyph(img, drawn, graph, atlas, x, y, b'~', level, clock);
        }
    }
}

fn kite_string(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    palette: &IndexedPalette,
    a: &Anchor,
    t_ms: u64,
) {
    let (w, h) = (f64::from(graph.width), f64::from(graph.height));
    let dir = f64::from(a.dir);
    let kx = f64::from(a.x) + dir * w * (0.16 + 0.025 * wave(t_ms, 6000, 0.0));
    let ky = f64::from(a.y) - h * (0.26 + 0.02 * wave(t_ms, 3000, 1.0));
    let kx = kx.clamp(w * 0.04, w * 0.96).round() as i32;
    let ky = ky.max(h * 0.06).round() as i32;
    let sc = a.sc;
    let red = palette.sprite_index(RED);
    let white = palette.sprite_index(WHITE);
    let red_dark = palette.sprite_index(RED_DARK).or(red);
    // A diamond seven art pixels across, with a white cross-spar.
    const KITE: [&str; 7] = [
        "...R...", "..RWR..", ".RRWRR.", "WWWWWWW", ".RRWRR.", "..RWR..", "...R...",
    ];
    for (r, row) in KITE.iter().enumerate() {
        for (c, ch) in row.bytes().enumerate() {
            let color = match ch {
                b'R' => red,
                b'W' => white,
                _ => continue,
            };
            let x = kx + (c as i32 - 3) * sc;
            let y = ky + (r as i32 - 3) * sc;
            fill(img, drawn, graph, x, y, sc, sc, color);
        }
    }
    // The tail: bows hanging below, swaying (both reds, so they show on
    // either theme).
    for i in 1..=5 {
        let sway = (f64::from(sc) * 1.2 * wave(t_ms, 1000, f64::from(i) * 0.8)).round() as i32;
        let x = kx + sway;
        let y = ky + (3 + i * 2) * sc;
        let color = if i % 2 == 0 { red_dark } else { red };
        fill(img, drawn, graph, x, y, sc, sc, color);
    }
    line(
        img,
        drawn,
        graph,
        (f64::from(a.x), f64::from(a.y)),
        (f64::from(kx), f64::from(ky + 3 * sc)),
        0.1,
        (sc / 2).max(1),
        3,
    );
}

/// A rectangle in a sprite colour, or in strong ink when the palette lacks
/// the colour.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fill(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: Option<u8>,
) {
    let Some(r) = PixelRect::clipped(x, y, x + w, y + h, img.width, img.height) else {
        return;
    };
    drawn.push(r);
    let width = img.width as usize;
    for py in r.y..r.y + r.h {
        let rt = graph.lighting.row_terms(py);
        for px in r.x..r.x + r.w {
            img.pixels[py as usize * width + px as usize] = match color {
                Some(c) => c,
                None => IndexedPalette::ink(graph.lighting.band_in_row(&rt, px), LEVELS),
            };
        }
    }
}

/// A line of `th`-pixel dots in ink at `level`, from `a` to `b`, sagging by
/// `sag` of its length in the middle (a quadratic curve).
#[allow(clippy::too_many_arguments)]
fn line(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    a: (f64, f64),
    b: (f64, f64),
    sag: f64,
    th: i32,
    level: u8,
) {
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    let ctrl = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0 + sag * len);
    let steps = (len / f64::from(th)).ceil().max(1.0) as usize;
    let width = img.width as usize;
    let (iw, ih) = (img.width, img.height);
    // One rectangle per run of dots, so a long line isn't hundreds of rects.
    let mut run: Option<(i32, i32, i32, i32)> = None;
    let flush = |run: &mut Option<(i32, i32, i32, i32)>, drawn: &mut Vec<PixelRect>| {
        if let Some((x0, y0, x1, y1)) = run.take()
            && let Some(r) = PixelRect::clipped(x0, y0, x1, y1, iw, ih)
        {
            drawn.push(r);
        }
    };
    for i in 0..=steps {
        let s = i as f64 / steps as f64;
        let u = 1.0 - s;
        let x = u * u * a.0 + 2.0 * u * s * ctrl.0 + s * s * b.0;
        let y = u * u * a.1 + 2.0 * u * s * ctrl.1 + s * s * b.1;
        let (x0, y0) = (
            (x - f64::from(th) / 2.0).round() as i32,
            (y - f64::from(th) / 2.0).round() as i32,
        );
        for py in y0.max(0)..(y0 + th).min(img.height as i32) {
            let rt = graph.lighting.row_terms(py as u32);
            for px in x0.max(0)..(x0 + th).min(img.width as i32) {
                let band = graph.lighting.band_in_row(&rt, px as u32);
                img.pixels[py as usize * width + px as usize] = IndexedPalette::ink(band, level);
            }
        }
        run = Some(match run {
            Some((rx0, ry0, rx1, ry1)) => {
                (rx0.min(x0), ry0.min(y0), rx1.max(x0 + th), ry1.max(y0 + th))
            }
            None => (x0, y0, x0 + th, y0 + th),
        });
        if i % 8 == 7 {
            flush(&mut run, drawn);
        }
    }
    flush(&mut run, drawn);
}

/// A glyph at pixel `(x, y)`: at that position with `Clock::Smooth`, else in
/// the cell holding it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn glyph(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    x: f64,
    y: f64,
    ch: u8,
    level: u8,
    clock: Clock,
) {
    let g = &graph.grid;
    let id = clairlune_scene::glyphs::id(ch as char);
    let (gx0, gy0) = (f64::from(g.origin_x), f64::from(g.origin_y));
    let (gx1, gy1) = (
        gx0 + f64::from(g.cols * g.cell_w),
        gy0 + f64::from(g.rows * g.cell_h),
    );
    if x < gx0 || y < gy0 || x >= gx1 || y >= gy1 {
        return;
    }
    if clock == Clock::Smooth {
        draw_particle_at(img, drawn, graph, atlas, x, y, id, level, true);
    } else {
        let col = ((x - gx0) / f64::from(g.cell_w)) as u32;
        let row = ((y - gy0) / f64::from(g.cell_h)) as u32;
        draw_particle(img, drawn, graph, atlas, col, row, id, level, true, true);
    }
}
