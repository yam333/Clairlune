//! Performance budget: a 4K static bake in under 150 ms, single-threaded, in a
//! release build (PLAN.md section 5.2). Ignored by default because debug builds
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
