//! Determinism: `(seed, aspect, ctx)` gives an identical recipe, and
//! `(recipe, size, theme, buddy, t)` gives identical pixels. Cross-OS
//! determinism is checked by the golden hash manifest, which CI runs on Windows,
//! macOS and Linux.

use clairlune_render::{GlyphAtlas, IndexedPalette, bake_static, render_frame, render_still};
use clairlune_scene::{BuildOptions, SceneRecipe, Theme, build};
use clairlune_tests::{GOLDEN_SEEDS, blessing, golden_dir, recipe, still};

#[test]
fn recipes_are_identical_across_runs_and_round_trip() {
    for &seed in &GOLDEN_SEEDS {
        let a = recipe(seed, 3840, 2160);
        let b = recipe(seed, 3840, 2160);
        assert_eq!(a.to_json(), b.to_json(), "seed {seed}");
        assert_eq!(SceneRecipe::from_json(&a.to_json()).unwrap(), a);
    }
}

/// Recipe JSON is pinned byte for byte, so a change in composition logic (or in
/// float formatting on some OS) can't slip through.
#[test]
fn recipes_match_golden_json() {
    let dir = golden_dir().join("recipes");
    let mut failures = Vec::new();
    for &seed in &GOLDEN_SEEDS {
        let json = recipe(seed, 16, 9).to_json() + "\n";
        let path = dir.join(format!("seed-{seed}.json"));
        if blessing() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &json).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing {}; run with CLAIRLUNE_BLESS=1", path.display()));
        if expected.replace("\r\n", "\n") != json {
            failures.push(seed);
        }
    }
    assert!(
        failures.is_empty(),
        "recipe JSON changed for seeds {failures:?}"
    );
}

#[test]
fn pixels_are_identical_across_runs() {
    for &seed in &GOLDEN_SEEDS[..4] {
        for theme in [Theme::Dark, Theme::Light] {
            let a = still(seed, 480, 270, theme, true);
            let b = still(seed, 480, 270, theme, true);
            assert_eq!(a.image, b.image, "seed {seed} {theme:?}");
            assert_eq!(a.palette, b.palette);
        }
    }
}

#[test]
fn frames_loop_seamlessly() {
    let r = recipe(7, 640, 360);
    let opts = BuildOptions {
        width: 640,
        height: 360,
        theme: Theme::Dark,
        buddy: true,
    };
    let graph = build(&r, &opts).unwrap();
    let atlas = GlyphAtlas::for_grid(&graph.grid);
    let palette = IndexedPalette::new(&graph.family);
    let bg = bake_static(&graph, &atlas);
    for t in [0, 750, 2500, 5999] {
        let a = render_frame(&bg, &graph, &atlas, &palette, t);
        let b = render_frame(&bg, &graph, &atlas, &palette, t + graph.loop_ms);
        assert_eq!(a, b, "t = {t}");
    }
}

#[test]
fn hero_frame_is_the_default_still() {
    let r = recipe(42, 640, 360);
    let opts = BuildOptions {
        width: 640,
        height: 360,
        theme: Theme::Light,
        buddy: true,
    };
    let default = render_still(&r, &opts, None).unwrap();
    let explicit = render_still(&r, &opts, Some(default.graph.t_hero_ms)).unwrap();
    assert_eq!(default.image, explicit.image);
}
