//! Performance budget: a 4K static bake in under 150 ms, single-threaded, in a
//! release build. Ignored by default because debug builds
//! and shared CI runners are not representative. Run it with:
//!
//! ```sh
//! cargo test --release -p clairlune-tests --test perf -- --ignored --nocapture
//! ```

use clairlune_render::{GlyphAtlas, IndexedPalette, bake_static, render_frame};
use clairlune_scene::{BuildOptions, Theme, build};
use clairlune_tests::recipe;
use std::time::{Duration, Instant};

const BUDGET: Duration = Duration::from_millis(150);

fn best_of<T>(runs: usize, mut f: impl FnMut() -> T) -> Duration {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(f());
            t.elapsed()
        })
        .min()
        .unwrap()
}

#[test]
#[ignore = "release-only timing; run with --release -- --ignored"]
fn bake_4k_within_budget() {
    let (w, h) = (3840, 2160);
    let r = recipe(7, w, h);
    let opts = BuildOptions {
        width: w,
        height: h,
        theme: Theme::Dark,
        buddy: true,
    };

    let t_build = best_of(5, || build(&r, &opts).unwrap());
    let graph = build(&r, &opts).unwrap();
    let t_atlas = best_of(5, || GlyphAtlas::for_grid(&graph.grid));
    let atlas = GlyphAtlas::for_grid(&graph.grid);
    let t_bake = best_of(5, || bake_static(&graph, &atlas));
    let bg = bake_static(&graph, &atlas);
    let palette = IndexedPalette::for_graph(&graph);
    let t_frame = best_of(5, || {
        render_frame(&bg, &graph, &atlas, &palette, graph.t_hero_ms)
    });

    let total = t_build + t_atlas + t_bake;
    eprintln!(
        "4K: build {:?}, atlas {:?}, bake {:?} (scene total {:?}); frame composite {:?}",
        t_build, t_atlas, t_bake, total, t_frame
    );
    assert!(total < BUDGET, "4K bake took {total:?}, budget {BUDGET:?}");
}

/// A journey redraws the whole frame each tick (the land moves). At 4K that
/// must stay well inside a 30 fps frame on one core, the CPU fallback's cost
/// before the GPU layer grids (docs/dev/scrolling-scenes.md, M7).
#[test]
#[ignore = "release-only timing; run with --release -- --ignored"]
fn journey_frame_4k_within_budget() {
    use clairlune_render::{Clock, ScenePackage};
    use clairlune_scene::{Aspect, ContentContext, compose};
    let (w, h) = (3840, 2160);
    let ctx = ContentContext {
        enabled_activities: vec!["hike_ridge".into()],
        enabled_archetypes: vec!["mountains".into()],
        ..ContentContext::default()
    };
    let r = compose(3, Aspect::from_size(w, h), &ctx);
    let t_capture = best_of(2, || ScenePackage::capture(&r, w, h).unwrap());
    let pkg = ScenePackage::capture(&r, w, h).unwrap();
    let mut frame = pkg.incremental(Theme::Dark).unwrap();
    // The first frame bakes the strips.
    let first = Instant::now();
    pkg.advance(&mut frame, Theme::Dark, true, 0, Clock::Smooth);
    let t_first = first.elapsed();
    let mut t = 0;
    let t_frame = best_of(20, || {
        t += 33;
        pkg.advance(&mut frame, Theme::Dark, true, t, Clock::Smooth)
    });
    // What the Direct3D 11 presenter adds: the whole frame as 0RGB, one
    // palette lookup per pixel (as `Player::rect_pixels`).
    let colors: Vec<u32> = pkg
        .palette(Theme::Dark)
        .unwrap()
        .colors()
        .iter()
        .map(|c| (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b))
        .collect();
    let mut scratch: Vec<u32> = Vec::new();
    let t_rgb = best_of(10, || {
        scratch.clear();
        scratch.extend(frame.image().pixels.iter().map(|&i| colors[usize::from(i)]));
        scratch.len()
    });
    eprintln!(
        "4K journey: capture {t_capture:?}, first frame {t_first:?}, frame {t_frame:?}, to 0RGB {t_rgb:?}"
    );
    assert!(
        t_frame + t_rgb < Duration::from_millis(20),
        "4K journey frame took {t_frame:?} + {t_rgb:?}"
    );
}
