//! Property tests over random seeds and sizes (PLAN.md section 12).

use clairlune_render::palette::TRANSPARENT_INDEX;
use clairlune_render::{GlyphAtlas, bake_static, render_still};
use clairlune_scene::{BuildOptions, SceneGraph, SpriteKind, Theme, build};
use clairlune_tests::recipe;
use proptest::prelude::*;

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
    build(
        &recipe(seed, w, h),
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
        let theme = if dark { Theme::Dark } else { Theme::Light };
        let r = recipe(seed, w, h);
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
        for s in &g.sprites {
            prop_assert_eq!(s.y + s.height() as i32, g.ground_y, "{:?} not on the ground", s.kind);
            prop_assert!(s.x >= 0 && s.x + s.width() as i32 <= w as i32, "{:?} off-screen", s.kind);
            prop_assert!(s.y >= 0);
        }
        prop_assert!(g.sprites.iter().any(|s| s.kind == SpriteKind::Character));
    }

    /// Buddy is a render flag: toggling it never changes the landscape.
    #[test]
    fn buddy_toggle_keeps_the_landscape(seed in any::<u64>(), (w, h) in size()) {
        let on = graph(seed, w, h, Theme::Light, true);
        let off = graph(seed, w, h, Theme::Light, false);
        prop_assert_eq!(&on.cells, &off.cells);
        let atlas = GlyphAtlas::for_grid(&on.grid);
        prop_assert_eq!(bake_static(&on, &atlas), bake_static(&off, &atlas));
        prop_assert!(off.sprites.iter().all(|s| s.kind != SpriteKind::Character));
    }

    /// Switching theme re-renders the same scene: same layout, same ground and
    /// sprite placement, and identical terrain below the horizon. (The sky
    /// differs by design: stars at night, dust by day.)
    #[test]
    fn theme_toggle_keeps_the_layout(seed in any::<u64>(), (w, h) in size()) {
        let dark = graph(seed, w, h, Theme::Dark, true);
        let light = graph(seed, w, h, Theme::Light, true);
        prop_assert_eq!(dark.grid, light.grid);
        prop_assert_eq!(dark.ground_y, light.ground_y);
        prop_assert_eq!(&dark.sprites, &light.sprites);
        let ground_row = ((dark.ground_y as u32) - dark.grid.origin_y) / dark.grid.cell_h;
        let from = (ground_row * dark.grid.cols) as usize;
        prop_assert_eq!(&dark.cells[from..], &light.cells[from..]);
        prop_assert_eq!(dark.t_hero_ms, light.t_hero_ms);
    }
}
