//! Projectile effects (docs/activity-system.md section 3): something thrown
//! from an anchor on the character, on its own arc, as a pure function of
//! the time since the effect's state began.
//!
//! - `skipping_stone`: a flat stone from the hand, out over the water past
//!   the pier's end, skipping three times, each hop shorter and lower, with
//!   a splash at every touch; then it sinks.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::clock::Clock;
use crate::frame::PixelRect;
use crate::palette::IndexedPalette;
use crate::tether::{Anchor, fill, glyph};
use clairlune_scene::{Rgb, SceneGraph, Theme};

/// The stone: light on dark scenes, slate on light ones, so it shows.
const STONE_LIGHT: Rgb = Rgb::hex(0xC8C2B0);
const STONE_DARK: Rgb = Rgb::hex(0x3B4450);

/// Draws a projectile preset `elapsed_ms` into its state; unknown presets and
/// scenes without what the preset needs (open water) draw nothing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_projectile(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    preset: &str,
    a: &Anchor,
    elapsed_ms: u64,
    clock: Clock,
) {
    if preset == "skipping_stone" {
        skipping_stone(img, drawn, graph, atlas, palette, a, elapsed_ms, clock);
    }
}

/// One hop: from `x0` to `x1` on the waterline (the first from the hand),
/// `h` pixels high at its top, taking `ms`.
struct Hop {
    x0: f64,
    y0: f64,
    x1: f64,
    h: f64,
    ms: u64,
}

/// The stone's hops: the throw, then three skips, each 55 % as long and
/// 40 % as high as the last, and quicker.
fn hops(graph: &SceneGraph, a: &Anchor, water_x: f64, water_y: f64) -> Vec<Hop> {
    let (w, h) = (f64::from(graph.width), f64::from(graph.height));
    let dir = f64::from(a.dir);
    // The first touch lands well out on the open water.
    let first = (water_x - f64::from(a.x)).abs() + w * 0.05;
    let mut out = vec![Hop {
        x0: f64::from(a.x),
        y0: f64::from(a.y),
        x1: f64::from(a.x) + dir * first,
        h: h * 0.07,
        ms: 900,
    }];
    let (mut len, mut height, mut ms) = (w * 0.07, h * 0.03, 520.0);
    for _ in 0..3 {
        let x0 = out.last().map_or(0.0, |p| p.x1);
        out.push(Hop {
            x0,
            y0: water_y,
            x1: x0 + dir * len,
            h: height,
            ms: ms as u64,
        });
        len *= 0.55;
        height *= 0.4;
        ms *= 0.72;
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn skipping_stone(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    a: &Anchor,
    elapsed_ms: u64,
    clock: Clock,
) {
    let Some((wx, wy)) = graph.water else {
        return;
    };
    let (water_x, water_y) = (f64::from(wx), f64::from(wy));
    let g = &graph.grid;
    let hops = hops(graph, a, water_x, water_y);
    // Splashes where the stone touched, fading over 600 ms.
    const SPLASH_MS: u64 = 600;
    let mut start = 0u64;
    for (i, hop) in hops.iter().enumerate() {
        let end = start + hop.ms;
        if elapsed_ms >= end && elapsed_ms < end + SPLASH_MS {
            // A ring at the touch that shrinks (O, o, .) while ripples
            // spread to the sides; later skips splash less.
            let q = (elapsed_ms - end) as f64 / SPLASH_MS as f64;
            let level = ((1.0 - 0.6 * q) * (6.0 - i as f64)).round().clamp(1.0, 6.0) as u8;
            let y = water_y + f64::from(g.cell_h) * 0.2;
            let ring = match q {
                q if q < 0.33 => b'O',
                q if q < 0.66 => b'o',
                _ => b'.',
            };
            glyph(img, drawn, graph, atlas, hop.x1, y, ring, level, clock);
            if q > 0.25 {
                let reach = f64::from(g.cell_w) * (0.8 + 1.4 * q);
                let faint = level.saturating_sub(1).max(1);
                for side in [-1.0, 1.0] {
                    let x = hop.x1 + side * reach;
                    glyph(img, drawn, graph, atlas, x, y, b'~', faint, clock);
                }
            }
        }
        if (start..end).contains(&elapsed_ms) {
            // The stone on its arc: a straight line between the ends, lifted
            // by a parabola.
            let u = (elapsed_ms - start) as f64 / hop.ms as f64;
            let x = hop.x0 + (hop.x1 - hop.x0) * u;
            let y = hop.y0 + (water_y - hop.y0) * u - 4.0 * hop.h * u * (1.0 - u);
            let sc = a.sc;
            let stone = match graph.theme {
                Theme::Dark => STONE_LIGHT,
                Theme::Light => STONE_DARK,
            };
            let color = palette.sprite_index(stone);
            fill(
                img,
                drawn,
                graph,
                x.round() as i32 - sc,
                y.round() as i32 - sc,
                sc * 2,
                sc,
                color,
            );
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_scene::{Aspect, BuildOptions, ContentContext, Theme, build, compose};

    /// The skipping-stone scene's graph at 1920x1080.
    fn shore() -> SceneGraph {
        let ctx = ContentContext {
            enabled_activities: vec!["skip_stones".to_string()],
            ..ContentContext::default()
        };
        let r = compose(4, Aspect::from_size(1920, 1080), &ctx);
        let theme = if r.tod_for(Theme::Dark).is_some() {
            Theme::Dark
        } else {
            Theme::Light
        };
        let opts = BuildOptions {
            width: 1920,
            height: 1080,
            theme,
            buddy: true,
        };
        build(&r, &opts).unwrap()
    }

    #[test]
    fn the_stone_lands_on_open_water_and_skips_shorter_each_time() {
        let g = shore();
        let (wx, wy) = g.water.expect("the shore has open water");
        let track = g.character.expect("a character");
        let dir = if track.face_left { -1 } else { 1 };
        let a = Anchor {
            x: wx - dir * 200,
            y: g.ground_y - 40,
            sc: 4,
            dir,
            state: "watch",
        };
        let hops = hops(&g, &a, f64::from(wx), f64::from(wy));
        assert_eq!(hops.len(), 4);
        // The first touch is past the pier's end, out on the water.
        assert!(f64::from(dir) * (hops[0].x1 - f64::from(wx)) > 0.0);
        for w in hops.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert_eq!(b.x0, a.x1, "each skip starts where the last touched");
            assert!((b.x1 - b.x0).abs() < (a.x1 - a.x0).abs());
            assert!(b.h < a.h && b.ms < a.ms);
        }
        // All of it on screen, and over before the watch state can end
        // (dwell_s starts at 3.5 s; the last splash fades 600 ms later).
        assert!(hops.iter().all(|h| (0.0..1920.0).contains(&h.x1)));
        let total: u64 = hops.iter().map(|h| h.ms).sum();
        assert!(total + 600 <= 3500, "{total} ms");
    }
}
