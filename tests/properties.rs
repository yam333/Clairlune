//! Property tests over random seeds and sizes (PLAN.md section 12).

use clairlune_render::palette::TRANSPARENT_INDEX;
use clairlune_render::{GlyphAtlas, bake_static, render_still};
use clairlune_scene::{BuildOptions, SceneGraph, SpriteKind, Theme, build};
use clairlune_tests::{available_theme, recipe};
use proptest::prelude::*;
use std::collections::BTreeMap;

/// Sizes that exercise landscape, ultrawide, portrait and the minimum cell size.
fn size() -> impl Strategy<Value = (u32, u32)> {
    prop_oneof![
        Just((320, 180)),
        Just((480, 270)),
        Just((640, 360)),
        Just((840, 360)),  // 21:9
        Just((1280, 360)), // 32:9
        Just((270, 480)),  // 9:16
        Just((400, 300)),  // 4:3
    ]
}

fn graph(seed: u64, w: u32, h: u32, theme: Theme, buddy: bool) -> SceneGraph {
    let r = recipe(seed, w, h);
    let theme = available_theme(&r, theme);
    build(
        &r,
        &BuildOptions {
            width: w,
            height: h,
            theme,
            buddy,
        },
    )
    .unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// At most 255 colours, and every pixel is an entry of the palette.
    #[test]
    fn every_pixel_is_a_palette_entry(seed in any::<u64>(), (w, h) in size(), dark in any::<bool>()) {
        let r = recipe(seed, w, h);
        let theme = available_theme(&r, if dark { Theme::Dark } else { Theme::Light });
        let s = render_still(&r, &BuildOptions { width: w, height: h, theme, buddy: true }, None).unwrap();
        prop_assert!(s.palette.len() <= 255);
        let n = s.palette.len() as u8;
        prop_assert!(s.image.pixels.iter().all(|&p| p < n && p != TRANSPARENT_INDEX));
    }

    /// The character's feet are on the platform's top surface, and the whole focal
    /// group is on screen.
    #[test]
    fn focal_group_is_grounded_and_on_screen(seed in any::<u64>(), (w, h) in size()) {
        let g = graph(seed, w, h, Theme::Dark, true);
        let flies = g.character.is_some_and(|t| t.flies());
        for s in &g.sprites_at(g.t_hero_ms) {
            if flies && s.kind == SpriteKind::Character {
                prop_assert!(s.y >= 0 && s.y + s.height() as i32 <= h as i32, "flying off screen");
                prop_assert!(s.x >= 0 && s.x + s.width() as i32 <= w as i32, "flying off screen");
                continue;
            }
            // On a journey the feet are on the ground going by under them.
            let ground = g
                .journey
                .as_ref()
                .and_then(|j| j.ground_at(g.grid.cell_w, g.grid.origin_x, g.t_hero_ms))
                .unwrap_or(g.ground_y);
            prop_assert_eq!(s.y + s.height() as i32, ground, "{:?} not on the ground", s.kind);
            prop_assert!(s.x >= 0 && s.x + s.width() as i32 <= w as i32, "{:?} off-screen", s.kind);
            prop_assert!(s.y >= 0);
        }
        prop_assert!(g.sprites_at(0).iter().any(|s| s.kind == SpriteKind::Character));
    }

    /// Buddy is a render flag: toggling it never changes the landscape.
    #[test]
    fn buddy_toggle_keeps_the_landscape(seed in any::<u64>(), (w, h) in size()) {
        let on = graph(seed, w, h, Theme::Light, true);
        let off = graph(seed, w, h, Theme::Light, false);
        prop_assert_eq!(&on.cells, &off.cells);
        let atlas = GlyphAtlas::for_grid(&on.grid);
        prop_assert_eq!(bake_static(&on, &atlas), bake_static(&off, &atlas));
        prop_assert!(off.sprites_at(0).iter().all(|s| s.kind != SpriteKind::Character));
        prop_assert_eq!(&on.journey, &off.journey);
        // A journey with Buddy off is the land going by: nothing stays put.
        prop_assert!(
            off.journey.is_some() || !off.sprites_at(0).is_empty(),
            "Buddy off must still have a focal object"
        );
    }

    /// Switching theme re-renders the same scene: same layout, same ground and
    /// sprite placement, and the same terrain shapes below the horizon. (The sky
    /// differs by design: stars at night, dust by day; so do details that follow
    /// the time of day, such as which city windows are lit.)
    #[test]
    fn theme_toggle_keeps_the_layout(seed in any::<u64>(), (w, h) in size()) {
        prop_assume!(!recipe(seed, w, h).is_theme_locked());
        let dark = graph(seed, w, h, Theme::Dark, true);
        let light = graph(seed, w, h, Theme::Light, true);
        prop_assert_eq!(dark.grid, light.grid);
        prop_assert_eq!(dark.ground_y, light.ground_y);
        prop_assert_eq!(dark.sprites_at(0), light.sprites_at(0));
        // Line art: the same shapes. Dithered: ink is light on dark and
        // dark on light, so only the ground line under the group matches.
        let cells = layout_cells(&dark, recipe(seed, w, h).look == "line");
        let shape = |g: &SceneGraph| -> Vec<bool> {
            cells.iter().map(|&i| g.cells[i].glyph != 0).collect()
        };
        prop_assert_eq!(shape(&dark), shape(&light));
        prop_assert_eq!(dark.t_hero_ms, light.t_hero_ms);
    }
}

/// A boat rowing back and forth stays on screen, even on a narrow portrait
/// monitor with no room past the pier.
#[test]
fn the_rowing_boat_stays_on_screen() {
    let ctx = clairlune_scene::ContentContext {
        enabled_activities: vec!["rowing".into()],
        ..Default::default()
    };
    let mut rowed = 0;
    for (w, h) in [(160, 3000), (180, 900), (270, 480)] {
        for seed in 0..40 {
            let r = clairlune_scene::compose(seed, clairlune_scene::Aspect::from_size(w, h), &ctx);
            if r.activity.as_deref() != Some("rowing") {
                continue;
            }
            rowed += 1;
            let theme = available_theme(&r, Theme::Dark);
            let opts = BuildOptions {
                width: w,
                height: h,
                theme,
                buddy: true,
            };
            let g = build(&r, &opts).unwrap();
            for t in (0..60_000).step_by(1_250) {
                for s in g.sprites_at(t) {
                    assert!(
                        s.x >= 0 && s.x + s.width() as i32 <= w as i32,
                        "seed {seed} at {w}x{h}, {t} ms: {:?} at x {} off screen",
                        s.kind,
                        s.x
                    );
                }
            }
        }
    }
    assert!(rowed > 0, "no rowing scenes to check");
}

/// Further back (wide, vast), Clawd is smaller than up close and still on
/// screen with his feet on the ground (or over it, flying).
#[test]
fn zoomed_out_scenes_shrink_clawd_and_keep_him_grounded() {
    let sizes = [(1920, 1080), (2560, 1440), (1280, 720), (1080, 1920)];
    for (w, h) in sizes {
        for seed in 0..24u64 {
            let at = |zoom: &str| {
                let ctx = clairlune_scene::ContentContext {
                    enabled_zooms: vec![zoom.into()],
                    ..Default::default()
                };
                let r =
                    clairlune_scene::compose(seed, clairlune_scene::Aspect::from_size(w, h), &ctx);
                assert_eq!(r.zoom, zoom);
                let theme = available_theme(&r, Theme::Dark);
                let opts = BuildOptions {
                    width: w,
                    height: h,
                    theme,
                    buddy: true,
                };
                build(&r, &opts).unwrap()
            };
            let close = at("close");
            let scale_of = |g: &SceneGraph| g.character.map_or(0, |c| c.scale);
            for zoom in ["wide", "vast"] {
                let g = at(zoom);
                assert!(
                    scale_of(&g) < scale_of(&close) || scale_of(&close) == 1,
                    "seed {seed} {w}x{h} {zoom}: Clawd isn't smaller"
                );
                assert!(
                    g.grid.cell_h <= close.grid.cell_h,
                    "finer text, or the same"
                );
                let flies = g.character.is_some_and(|t| t.flies());
                for s in g.sprites_at(g.t_hero_ms) {
                    assert!(
                        s.x >= 0 && s.x + s.width() as i32 <= w as i32 && s.y >= 0,
                        "seed {seed} {w}x{h} {zoom}: {:?} off screen",
                        s.kind
                    );
                    if flies || s.kind != SpriteKind::Character {
                        continue;
                    }
                    let ground = g
                        .journey
                        .as_ref()
                        .and_then(|j| j.ground_at(g.grid.cell_w, g.grid.origin_x, g.t_hero_ms))
                        .unwrap_or(g.ground_y);
                    assert_eq!(
                        s.y + s.height() as i32,
                        ground,
                        "seed {seed} {w}x{h} {zoom}"
                    );
                }
            }
        }
    }
}

/// The cells whose shape must not change with the theme: line art keeps
/// everything from the ground row down; the dithered look inks light on
/// dark and dark on light, so only the ground under the character.
fn layout_cells(g: &SceneGraph, line: bool) -> Vec<usize> {
    let cols = g.grid.cols as usize;
    let ground_row = ((g.ground_y as u32 - g.grid.origin_y) / g.grid.cell_h) as usize;
    if line {
        return (ground_row * cols..g.cells.len()).collect();
    }
    // A character in the air or on the water stands on nothing.
    if g.character.is_none_or(|t| t.flies()) {
        return Vec::new();
    }
    let Some(c) = g
        .sprites_at(0)
        .into_iter()
        .find(|s| s.kind == SpriteKind::Character)
    else {
        return Vec::new();
    };
    let col = |x: i32| {
        ((x.max(g.grid.origin_x as i32) as u32 - g.grid.origin_x) / g.grid.cell_w) as usize
    };
    let (c0, c1) = (col(c.x), col(c.x + c.width() as i32 - 1).min(cols - 1));
    (c0..=c1).map(|cl| ground_row * cols + cl).collect()
}

/// The theme check above on every landscape: random seeds reach some of them
/// only rarely, and their details follow the time of day.
#[test]
fn theme_toggle_keeps_the_layout_on_every_landscape() {
    use clairlune_scene::archetype::ARCHETYPE_INFO;
    use clairlune_scene::{Aspect, ContentContext, compose};
    for info in ARCHETYPE_INFO.iter().filter(|a| a.implemented) {
        let ctx = ContentContext {
            enabled_archetypes: vec![info.id.to_string()],
            ..ContentContext::default()
        };
        for seed in 0..24u64 {
            for (w, h) in [(640, 360), (1280, 360), (270, 480)] {
                let r = compose(seed, Aspect::from_size(w, h), &ctx);
                if r.is_theme_locked() {
                    continue;
                }
                let g = |theme| {
                    let opts = BuildOptions {
                        width: w,
                        height: h,
                        theme,
                        buddy: true,
                    };
                    build(&r, &opts).unwrap()
                };
                let (dark, light) = (g(Theme::Dark), g(Theme::Light));
                let cells = layout_cells(&dark, r.look == "line");
                let shape = |g: &SceneGraph| -> Vec<bool> {
                    cells.iter().map(|&i| g.cells[i].glyph != 0).collect()
                };
                assert_eq!(
                    shape(&dark),
                    shape(&light),
                    "{} seed {seed} at {w}x{h}",
                    info.id
                );
            }
        }
    }
}

/// Variety (PLAN.md section 12): over 1,000 seeds with default weights, no
/// activity exceeds twice its expected share and every activity appears.
#[test]
fn no_activity_dominates() {
    let n = 1000;
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for seed in 0..n {
        let id = recipe(seed, 1920, 1080)
            .activity
            .expect("an activity is eligible");
        *counts.entry(id).or_default() += 1;
    }
    let catalog = clairlune_scene::activity::builtin_catalog();
    let expected = n as f64 / catalog.activities.len() as f64;
    for a in &catalog.activities {
        let c = f64::from(counts.get(&a.id).copied().unwrap_or(0));
        assert!(c > 0.0, "{} never appears", a.id);
        assert!(
            c <= 2.0 * expected,
            "{} appears {c} times, expected about {expected}",
            a.id
        );
    }
}
