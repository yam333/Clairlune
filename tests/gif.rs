//! GIF export: decoded frames equal the renderer's frames, the loop closes with
//! no seam, delays sum to the loop, and sizes stay small (PLAN.md section 6.4).

use clairlune_render::{
    Clock, GlyphAtlas, IndexedPalette, Smoothness, bake_static, encode_gif, render_frame_clocked,
};
use clairlune_scene::{Aspect, BuildOptions, ContentContext, SceneRecipe, Theme, build, compose};

fn recipe_for(activity: &str, seed: u64, w: u32, h: u32) -> SceneRecipe {
    let ctx = ContentContext {
        enabled_activities: vec![activity.to_string()],
        ..ContentContext::default()
    };
    compose(seed, Aspect::from_size(w, h), &ctx)
}

const ACTIVITIES: [&str; 3] = ["warm_hands", "stroll", "stargaze"];

#[test]
fn decoded_frames_equal_rendered_frames() {
    let (w, h, loop_ms) = (320, 180, 6000);
    for (i, activity) in ACTIVITIES.iter().enumerate() {
        let recipe = recipe_for(activity, 11 + i as u64, w, h);
        let opts = BuildOptions {
            width: w,
            height: h,
            theme: Theme::Dark,
            buddy: true,
        };
        let mut bytes = Vec::new();
        let stats = encode_gif(&recipe, &opts, loop_ms, Smoothness::Balanced, &mut bytes).unwrap();

        // Reference renderer.
        let graph = build(&recipe, &opts).unwrap();
        let atlas = GlyphAtlas::for_grid(&graph.grid);
        let palette = IndexedPalette::for_graph(&graph);
        let bg = bake_static(&graph, &atlas);

        let mut dec_opts = gif::DecodeOptions::new();
        dec_opts.set_color_output(gif::ColorOutput::Indexed);
        let mut dec = dec_opts.read_info(std::io::Cursor::new(bytes)).unwrap();
        let mut canvas = vec![0u8; (w * h) as usize];
        let (mut t_cs, mut frames) = (0u64, 0usize);
        while let Some(f) = dec.read_next_frame().unwrap() {
            for y in 0..f.height as usize {
                for x in 0..f.width as usize {
                    let v = f.buffer[y * f.width as usize + x];
                    if Some(v) != f.transparent {
                        canvas[(f.top as usize + y) * w as usize + f.left as usize + x] = v;
                    }
                }
            }
            let expected = render_frame_clocked(
                &bg,
                &graph,
                &atlas,
                &palette,
                t_cs * 10,
                Clock::Loop(loop_ms),
            );
            assert!(
                canvas == expected.pixels,
                "{activity}: frame {frames} at {} ms differs",
                t_cs * 10
            );
            t_cs += u64::from(f.delay);
            frames += 1;
        }
        assert_eq!(frames, stats.frames_written);
        assert_eq!(
            t_cs * 10,
            loop_ms,
            "{activity}: delays must sum to the loop"
        );
    }
}

#[test]
fn loops_have_no_seam() {
    for (i, activity) in ACTIVITIES.iter().enumerate() {
        for loop_ms in [6000, 12000] {
            let recipe = recipe_for(activity, 40 + i as u64, 480, 270);
            let opts = BuildOptions {
                width: 480,
                height: 270,
                theme: Theme::Dark,
                buddy: true,
            };
            let graph = build(&recipe, &opts).unwrap();
            let atlas = GlyphAtlas::for_grid(&graph.grid);
            let palette = IndexedPalette::for_graph(&graph);
            let bg = bake_static(&graph, &atlas);
            let clock = Clock::Loop(loop_ms);
            let first = render_frame_clocked(&bg, &graph, &atlas, &palette, 0, clock);
            let wrapped = render_frame_clocked(&bg, &graph, &atlas, &palette, loop_ms, clock);
            assert_eq!(first, wrapped, "{activity}, {loop_ms} ms loop");
        }
    }
}

/// Records real sizes (run with --nocapture) and guards against regressions.
#[test]
fn gif_sizes_stay_small() {
    for activity in ACTIVITIES {
        let recipe = recipe_for(activity, 7, 960, 540);
        let opts = BuildOptions {
            width: 960,
            height: 540,
            theme: Theme::Dark,
            buddy: true,
        };
        let mut bytes = Vec::new();
        let stats = encode_gif(&recipe, &opts, 6000, Smoothness::Balanced, &mut bytes).unwrap();
        eprintln!(
            "{activity}: 960x540, 6 s, balanced: {} frames, {:.0} KB",
            stats.frames_written,
            bytes.len() as f64 / 1024.0
        );
        assert!(
            bytes.len() < 5 * 1024 * 1024,
            "{activity}: {} bytes",
            bytes.len()
        );
    }
}
