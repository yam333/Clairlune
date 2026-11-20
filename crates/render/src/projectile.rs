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

/// A snowball: snow-white on dark scenes, slate on light ones.
const SNOW_LIGHT: Rgb = Rgb::hex(0xEEEAE0);
const SNOW_DARK: Rgb = Rgb::hex(0x5E6B7A);

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
    match preset {
        "skipping_stone" => skipping_stone(img, drawn, graph, atlas, palette, a, elapsed_ms, clock),
        "snowball" => snowball(img, drawn, graph, atlas, palette, a, elapsed_ms, clock),
        "tennis_ball" => tennis_ball(img, drawn, graph, palette, a, elapsed_ms),
        _ => {}
    }
}

/// One rally stroke in `RALLY_MS`: the ball leaves the racket, bounces off
/// the practice wall and comes back as the swing comes round again (the
/// rally animation lasts as long).
const RALLY_MS: u64 = 1200;
const TENNIS_BALL: Rgb = Rgb::hex(0xD8E04A);

fn tennis_ball(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    palette: &IndexedPalette,
    a: &Anchor,
    elapsed_ms: u64,
) {
    use clairlune_scene::SpriteKind;
    let Some(wall) = graph
        .props
        .iter()
        .find(|s| s.kind == SpriteKind::PracticeWall)
    else {
        return;
    };
    let (x0, y0) = (f64::from(a.x), f64::from(a.y));
    // The wall's face toward the player, a little above its middle.
    let wx = if f64::from(wall.x) > x0 {
        f64::from(wall.x)
    } else {
        f64::from(wall.x) + f64::from(wall.width())
    };
    let wy = f64::from(wall.y) + f64::from(wall.height()) * 0.25;
    let t = (elapsed_ms % RALLY_MS) as f64 / RALLY_MS as f64;
    // Out on the first half, back on the second; a low arc each way.
    let (u, from, to) = if t < 0.5 {
        (t * 2.0, (x0, y0), (wx, wy))
    } else {
        ((t - 0.5) * 2.0, (wx, wy), (x0, y0))
    };
    let h = (wx - x0).abs() * 0.2;
    let x = from.0 + (to.0 - from.0) * u;
    let y = from.1 + (to.1 - from.1) * u - 4.0 * h * u * (1.0 - u);
    let sc = a.sc;
    fill(
        img,
        drawn,
        graph,
        x.round() as i32 - sc,
        y.round() as i32 - sc,
        sc * 2,
        sc * 2,
        palette.sprite_index(TENNIS_BALL),
    );
}

/// A snowball from the hand to the snowman's middle on a short arc, then a
/// puff of snow where it hits.
#[allow(clippy::too_many_arguments)]
fn snowball(
    img: &mut IndexedImage,
    drawn: &mut Vec<PixelRect>,
    graph: &SceneGraph,
    atlas: &GlyphAtlas,
    palette: &IndexedPalette,
    a: &Anchor,
    elapsed_ms: u64,
    clock: Clock,
) {
    use clairlune_scene::SpriteKind;
    let Some(target) = graph.props.iter().find(|s| s.kind == SpriteKind::Snowman) else {
        return;
    };
    let tx = f64::from(target.x) + f64::from(target.width()) / 2.0;
    let ty = f64::from(target.y) + f64::from(target.height()) * 0.55;
    let (x0, y0) = (f64::from(a.x), f64::from(a.y));
    const FLIGHT_MS: u64 = 650;
    const PUFF_MS: u64 = 500;
    let g = &graph.grid;
    if elapsed_ms < FLIGHT_MS {
        let u = elapsed_ms as f64 / FLIGHT_MS as f64;
        let h = ((tx - x0).abs() * 0.35).max(f64::from(g.cell_h) * 2.0);
        let x = x0 + (tx - x0) * u;
        let y = y0 + (ty - y0) * u - 4.0 * h * u * (1.0 - u);
        let ball = match graph.theme {
            Theme::Dark => SNOW_LIGHT,
            Theme::Light => SNOW_DARK,
        };
        let sc = a.sc;
        fill(
            img,
            drawn,
            graph,
            x.round() as i32 - sc,
            y.round() as i32 - sc,
            sc * 2,
            sc * 2,
            palette.sprite_index(ball),
        );
    } else if elapsed_ms < FLIGHT_MS + PUFF_MS {
        // The puff: flecks flying out from the hit, fading.
        let q = (elapsed_ms - FLIGHT_MS) as f64 / PUFF_MS as f64;
        let level = ((1.0 - q) * 5.0).round().max(1.0) as u8;
        let r = f64::from(g.cell_w) * (0.5 + 1.5 * q);
        for (dx, dy, ch) in [
            (-1.0, -0.6, b'*'),
            (1.0, -0.8, b'\''),
            (0.0, -1.2, b'*'),
            (-0.6, 0.4, b'.'),
            (0.8, 0.3, b'.'),
        ] {
            glyph(
                img,
                drawn,
                graph,
                atlas,
                tx + dx * r,
                ty + dy * r * 0.8,
                ch,
                level,
                clock,
            );
        }
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
