//! The live compositor (PLAN.md section 5.3): a frame made incrementally from
//! the last one equals the full render at every time, and the rectangles it
//! reports cover every pixel that changed, so copying only those keeps a GPU
//! texture (or any other copy) exact.

use clairlune_render::BuiltinArt;
use clairlune_render::clock::Clock;
use clairlune_render::{
    GlyphAtlas, IncrementalFrame, IndexedImage, IndexedPalette, PixelRect, ScenePackage,
    bake_static, render_frame_with_art,
};
use clairlune_scene::{Aspect, BuildOptions, ContentContext, SceneRecipe, Theme, build, compose};
use clairlune_tests::{available_theme, golden_cases};

const W: u32 = 480;
const H: u32 = 270;

/// Every landscape, plus activities with anchor effects and weather.
fn recipes() -> Vec<SceneRecipe> {
    let mut out: Vec<SceneRecipe> = golden_cases().into_iter().map(|(_, r)| r).collect();
    let aspect = Aspect::from_size(W, H);
    for (seed, activity) in [(5u64, "wizard_magic"), (6, "make_coffee"), (8, "stroll")] {
        let ctx = ContentContext {
            enabled_activities: vec![activity.to_string()],
            ..ContentContext::default()
        };
        out.push(compose(seed, aspect, &ctx));
    }
    out
}

/// Live playback at 12 fps from the hero time, then a jump (a pause ending).
fn times(hero: u64) -> Vec<u64> {
    let mut t: Vec<u64> = (0..36).map(|k| hero + k * 83).collect();
    t.push(hero + 45_000);
    t.push(hero + 45_083);
    t
}

/// Every pixel that differs between two frames lies in one of the rects.
fn covered(before: &IndexedImage, after: &IndexedImage, rects: &[PixelRect]) -> Result<(), String> {
    let w = before.width;
    for (i, (a, b)) in before.pixels.iter().zip(&after.pixels).enumerate() {
        if a != b {
            let (x, y) = (i as u32 % w, i as u32 / w);
            if !rects.iter().any(|r| r.contains(x, y)) {
                return Err(format!("pixel ({x}, {y}) changed outside the rects"));
            }
        }
    }
    Ok(())
}

#[test]
fn a_package_played_incrementally_matches_full_frames() {
    let mut frames = 0;
    for r in recipes() {
        let pkg = ScenePackage::capture(&r, W, H).unwrap();
        for theme in pkg.themes() {
            let hero = pkg.hero_ms(theme).unwrap();
            for buddy in [true, false] {
                for clock in [Clock::Live, Clock::Loop(pkg.graph(theme).unwrap().loop_ms)] {
                    let mut live = pkg.incremental(theme).unwrap();
                    let mut last = live.image().clone();
                    for t in times(hero) {
                        let rects = pkg.advance(&mut live, theme, buddy, t, clock).unwrap();
                        let (full, _) = pkg.frame(theme, buddy, t, clock).unwrap();
                        let at = format!("seed {} {theme:?} buddy {buddy} {clock:?} t {t}", r.seed);
                        assert!(*live.image() == full, "{at}: differs from the full frame");
                        covered(&last, &full, &rects).unwrap_or_else(|e| panic!("{at}: {e}"));
                        assert!(
                            rects.windows(2).all(|p| p[0] < p[1]),
                            "{at}: rects not sorted and unique"
                        );
                        last = full;
                        frames += 1;
                    }
                }
            }
        }
    }
    assert!(frames > 1000, "only {frames} frames checked");
}

#[test]
fn built_in_art_plays_incrementally_too() {
    let r = compose(42, Aspect::from_size(W, H), &ContentContext::default());
    let opts = BuildOptions {
        width: W,
        height: H,
        theme: available_theme(&r, Theme::Dark),
        buddy: true,
    };
    let graph = build(&r, &opts).unwrap();
    let atlas = GlyphAtlas::for_grid(&graph.grid);
    let palette = IndexedPalette::for_graph(&graph);
    let bg = bake_static(&graph, &atlas);
    let art = BuiltinArt::for_graph(&graph);
    let mut live = IncrementalFrame::new(&bg);
    let mut changed_pixels = 0usize;
    for t in times(graph.t_hero_ms) {
        let rects = live.advance(&bg, &graph, &atlas, &palette, t, Clock::Live, &art);
        let full = render_frame_with_art(&bg, &graph, &atlas, &palette, t, Clock::Live, &art);
        assert!(*live.image() == full, "t {t}");
        changed_pixels += rects.iter().map(|r| (r.w * r.h) as usize).sum::<usize>();
    }
    // The point of it: a frame touches a small part of the screen.
    let per_frame = changed_pixels / times(graph.t_hero_ms).len();
    assert!(
        per_frame < (W * H) as usize / 4,
        "{per_frame} of {} pixels per frame",
        W * H
    );
}

/// Smoothness only changes how often a frame is drawn, never the pace: a
/// scene played at 6 fps and at 60 fps shows the same picture whenever both
/// draw at the same moment.
#[test]
fn every_frame_rate_shows_the_same_moment() {
    for r in recipes().into_iter().take(6) {
        let pkg = ScenePackage::capture(&r, W, H).unwrap();
        let theme = pkg.themes()[0];
        let hero = pkg.hero_ms(theme).unwrap();
        let mut slow = pkg.incremental(theme).unwrap();
        let mut fast = pkg.incremental(theme).unwrap();
        // Ten seconds: 6 fps draws every 1000/6 ms, 60 fps every 1000/60 ms;
        // they meet every 500 ms.
        for k in 0..=600u64 {
            let t = hero + k * 1000 / 60;
            pkg.advance(&mut fast, theme, true, t, Clock::Live).unwrap();
            if k % 10 == 0 {
                pkg.advance(&mut slow, theme, true, t, Clock::Live).unwrap();
                if k % 30 == 0 {
                    assert!(
                        slow.image() == fast.image(),
                        "seed {} at {} ms: 6 and 60 fps differ",
                        r.seed,
                        t - hero
                    );
                }
            }
        }
    }
}
