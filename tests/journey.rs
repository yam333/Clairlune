//! Journeys (docs/scrolling-scenes.md): the land scrolls past in layers and
//! comes round exactly after the loop; the feet stay on the ground going by;
//! live playback, saved packages and GIFs show the same frames.

use clairlune_render::{Clock, ScenePackage, Smoothness, encode_gif};
use clairlune_scene::{Aspect, BuildOptions, ContentContext, SceneRecipe, Theme, build, compose};
use std::path::PathBuf;

const W: u32 = 640;
const H: u32 = 360;

fn hike(seed: u64) -> SceneRecipe {
    let ctx = ContentContext {
        enabled_activities: vec!["hike_ridge".into()],
        enabled_archetypes: vec!["mountains".into()],
        ..ContentContext::default()
    };
    let r = compose(seed, Aspect::from_size(W, H), &ctx);
    assert_eq!(r.activity.as_deref(), Some("hike_ridge"));
    r
}

fn loop_of(pkg: &ScenePackage, theme: Theme) -> u64 {
    pkg.graph(theme).unwrap().journey.as_ref().unwrap().loop_ms
}

#[test]
fn a_journey_comes_round_exactly() {
    for seed in [1, 2, 3] {
        let r = hike(seed);
        let ms = u64::from(r.composition.journey_ms);
        assert!(ms >= 12_000 && ms.is_multiple_of(6_000), "loop {ms}");
        let pkg = ScenePackage::capture(&r, W, H).unwrap();
        for theme in pkg.themes() {
            assert_eq!(loop_of(&pkg, theme), ms);
            let clock = Clock::Loop(ms);
            for t in [0, 1_234, ms / 2, ms - 40] {
                let a = pkg.frame(theme, true, t, clock).unwrap().0;
                let b = pkg.frame(theme, true, t + ms, clock).unwrap().0;
                assert!(
                    a == b,
                    "seed {seed} {theme:?}: t {t} and a loop later differ"
                );
            }
        }
    }
}

#[test]
fn the_land_moves_and_the_far_peaks_stay() {
    let r = hike(4);
    let pkg = ScenePackage::capture(&r, W, H).unwrap();
    let g = pkg.graph(Theme::Dark).unwrap();
    let j = g.journey.as_ref().unwrap();
    // Nearer layers go faster; nothing has moved at the start of the loop.
    let speed = |i: usize| u64::from(j.layers[i].cols * j.layers[i].turns);
    assert!(speed(j.layers.len() - 1) > speed(0));
    for i in 0..j.layers.len() {
        assert_eq!(j.offset_px(i, g.grid.cell_w, 0), 0);
        assert_eq!(j.offset_px(i, g.grid.cell_w, j.loop_ms), 0);
    }
    let a = pkg.frame(Theme::Dark, false, 0, Clock::Live).unwrap().0;
    let b = pkg.frame(Theme::Dark, false, 2_000, Clock::Live).unwrap().0;
    let w = W as usize;
    let changed_rows = (0..H as usize)
        .filter(|&y| a.pixels[y * w..(y + 1) * w] != b.pixels[y * w..(y + 1) * w])
        .count();
    assert!(
        changed_rows > H as usize / 4,
        "only {changed_rows} rows moved"
    );
    // The top rows are sky over the still far peaks: they don't move.
    assert_eq!(a.pixels[..w * 10], b.pixels[..w * 10]);
}

#[test]
fn the_ground_has_no_jumps_and_no_seam() {
    for seed in [1, 5, 9] {
        let r = hike(seed);
        for (w, h) in [(W, H), (1920, 1080), (1080, 1920)] {
            let g = build(
                &r,
                &BuildOptions {
                    width: w,
                    height: h,
                    theme: Theme::Dark,
                    buddy: true,
                },
            )
            .unwrap();
            let j = g.journey.as_ref().unwrap();
            let n = j.ground.len();
            let walk = j.layers.last().unwrap();
            assert_eq!(n, (walk.cols * g.grid.cell_w) as usize);
            // One pixel along never moves the feet more than a few pixels,
            // across the strip's seam too.
            let steepest = (0..n)
                .map(|i| (j.ground[(i + 1) % n] - j.ground[i]).abs())
                .max()
                .unwrap();
            assert!(steepest <= 4, "{w}x{h}: a {steepest} px step");
            assert!(j.ground.iter().all(|&y| y > 0 && y < h as i32));
        }
    }
}

#[test]
fn live_playback_matches_the_full_frame() {
    let r = hike(6);
    let pkg = ScenePackage::capture(&r, W, H).unwrap();
    for theme in pkg.themes() {
        let mut live = pkg.incremental(theme).unwrap();
        for t in [0, 40, 1_000, 1_040, 17_000, 3_000] {
            let rects = pkg.advance(&mut live, theme, true, t, Clock::Live).unwrap();
            assert_eq!(rects.len(), 1, "a journey redraws the whole frame");
            let full = pkg.frame(theme, true, t, Clock::Live).unwrap().0;
            assert!(live.image() == &full, "{theme:?} t {t}");
        }
    }
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("clairlune-journey-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn a_saved_journey_plays_as_it_was() {
    let r = hike(7);
    let pkg = ScenePackage::capture(&r, W, H).unwrap();
    let dir = temp("save");
    pkg.save(&dir).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
    assert_eq!(json["format"], 2, "a journey needs format 2");
    let loaded = ScenePackage::load(&dir).unwrap();
    for theme in pkg.themes() {
        let ms = loop_of(&pkg, theme);
        for t in [0, 2_500, ms / 3] {
            for buddy in [true, false] {
                let a = pkg.frame(theme, buddy, t, Clock::Loop(ms)).unwrap().0;
                let b = loaded.frame(theme, buddy, t, Clock::Loop(ms)).unwrap().0;
                assert!(a == b, "{theme:?} t {t} buddy {buddy}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);

    // A scene that stays put is still written as format 1, which older
    // versions read.
    let still = compose(7, Aspect::from_size(W, H), &ContentContext::default());
    let dir = temp("still");
    let pkg = ScenePackage::capture(&still, W, H).unwrap();
    assert!(
        pkg.themes()
            .iter()
            .all(|&t| pkg.graph(t).unwrap().journey.is_none())
    );
    pkg.save(&dir).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
    assert_eq!(json["format"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_journey_gif_loops_with_the_land() {
    let r = hike(8);
    let opts = BuildOptions {
        width: 320,
        height: 180,
        theme: Theme::Dark,
        buddy: true,
    };
    let mut bytes = Vec::new();
    let stats = encode_gif(&r, &opts, 6_000, Smoothness::BatterySaver, &mut bytes).unwrap();
    assert_eq!(stats.loop_ms, u64::from(r.composition.journey_ms));
}
