//! What one live frame costs on the CPU, the part every presenter shares:
//! the player's composition and packing the changed rectangles for upload.
//! Draws nothing on screen.
//!
//! `cargo run -p clairlune-live --example frame_cost --release -- [WxH] [fps]`

use clairlune_live::Player;
use clairlune_render::ScenePackage;
use clairlune_scene::{Aspect, ContentContext, compose};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let (w, h) = args
        .next()
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((3024u32, 1964u32));
    let fps: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(12);
    for activity in ["warm_hands", "stroll", "stargaze", "hike_ridge", "broom_journey"] {
        let ctx = ContentContext {
            enabled_activities: vec![activity.into()],
            ..ContentContext::default()
        };
        let r = compose(40, Aspect::from_size(w, h), &ctx);
        let made = Instant::now();
        let pkg = Arc::new(ScenePackage::capture(&r, w, h).expect("scene"));
        let capture = made.elapsed();
        let theme = pkg.themes()[0];
        let mut player = Player::new(pkg.clone(), theme, true).expect("theme");
        player.set_smooth(true);
        let t0 = pkg.hero_ms(theme).unwrap_or(0);
        player.advance(t0);
        let frames = 10 * fps;
        let (mut advance, mut pack, mut pixels) = (Duration::ZERO, Duration::ZERO, 0u64);
        let mut packed: Vec<u8> = Vec::new();
        for k in 1..=frames {
            let t = t0 + k * 1000 / fps;
            let a = Instant::now();
            let rects = player.advance(t);
            let b = Instant::now();
            packed.clear();
            for r in clairlune_live::gpu::merge_spans(&rects) {
                let pitch = (r.w as usize).div_ceil(256) * 256;
                for y in r.y..r.y + r.h {
                    packed.extend_from_slice(player.index_row(r.x, y, r.w));
                    let len = packed.len();
                    packed.resize(len + pitch - r.w as usize, 0);
                }
                pixels += u64::from(r.w * r.h);
            }
            pack += b.elapsed();
            advance += b - a;
        }
        let per = |d: Duration| d.as_secs_f64() * 1e3 / frames as f64;
        let cpu = (advance + pack).as_secs_f64() / 10.0 * 100.0;
        println!(
            "{activity:<14} {w}x{h} at {fps} fps: advance {:.2} ms, pack {:.2} ms, {:.0} kpx/frame, ~{cpu:.1} % of a core (capture {:.0} ms)",
            per(advance),
            per(pack),
            pixels as f64 / frames as f64 / 1e3,
            capture.as_secs_f64() * 1e3
        );
    }
}
