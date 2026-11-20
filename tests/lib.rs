//! Shared helpers for the workspace integration tests.

use clairlune_render::{IndexedImage, Still, render_still};
use clairlune_scene::{
    Aspect, BuildError, BuildOptions, ContentContext, SceneRecipe, Theme, compose,
};
use std::path::PathBuf;

/// Golden test size (PLAN.md section 11, M1).
pub const GOLDEN_W: u32 = 640;
pub const GOLDEN_H: u32 = 360;
/// The ten fixed golden seeds.
pub const GOLDEN_SEEDS: [u64; 10] = [1, 2, 3, 7, 42, 99, 1234, 31337, 271_828, 9_000_000_001];
/// The seed of the per-landscape goldens (one scene per implemented archetype).
pub const LANDSCAPE_SEED: u64 = 11;

/// Golden cases: the fixed seeds, then one scene per implemented landscape so
/// each archetype is pinned across OSes whatever the seeds happen to pick.
pub fn golden_cases() -> Vec<(String, SceneRecipe)> {
    let aspect = Aspect::from_size(GOLDEN_W, GOLDEN_H);
    let mut cases: Vec<(String, SceneRecipe)> = GOLDEN_SEEDS
        .iter()
        .map(|&seed| (format!("seed-{seed}"), recipe(seed, GOLDEN_W, GOLDEN_H)))
        .collect();
    for a in clairlune_scene::archetype::ARCHETYPE_INFO
        .iter()
        .filter(|a| a.implemented)
    {
        // Only activities with a day and a night time, so every landscape
        // has a golden image in both themes (day-only and night-only ones
        // would lock it to one).
        let both = |times: &[clairlune_scene::TimeOfDay]| {
            use clairlune_scene::TimeOfDay;
            times.iter().any(|t| TimeOfDay::LIGHT.contains(t))
                && times.iter().any(|t| TimeOfDay::DARK.contains(t))
        };
        let ctx = ContentContext {
            enabled_archetypes: vec![a.id.to_string()],
            enabled_activities: clairlune_scene::activity::builtin_catalog()
                .activities
                .iter()
                .filter(|act| both(&act.requires.time_of_day))
                .map(|act| act.id.clone())
                .collect(),
            ..ContentContext::default()
        };
        cases.push((
            format!("land-{}", a.id),
            compose(LANDSCAPE_SEED, aspect, &ctx),
        ));
    }
    cases
}

/// `tests/golden/` in the repository.
pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("golden")
}

/// True when the run should rewrite golden files (`CLAIRLUNE_BLESS=1`).
pub fn blessing() -> bool {
    std::env::var("CLAIRLUNE_BLESS").is_ok_and(|v| v == "1")
}

pub fn recipe(seed: u64, w: u32, h: u32) -> SceneRecipe {
    compose(seed, Aspect::from_size(w, h), &ContentContext::default())
}

pub fn still(seed: u64, w: u32, h: u32, theme: Theme, buddy: bool) -> Still {
    let r = recipe(seed, w, h);
    render_still(
        &r,
        &BuildOptions {
            width: w,
            height: h,
            theme,
            buddy,
        },
        None,
    )
    .expect("renders")
}

/// Like [`still`], but `None` when the scene has no variant for `theme`
/// (theme-locked, e.g. a night-only activity in the light theme).
pub fn still_opt(seed: u64, w: u32, h: u32, theme: Theme, buddy: bool) -> Option<Still> {
    still_of(&recipe(seed, w, h), w, h, theme, buddy)
}

/// Renders a recipe's still, or `None` when it is theme-locked away from `theme`.
pub fn still_of(r: &SceneRecipe, w: u32, h: u32, theme: Theme, buddy: bool) -> Option<Still> {
    let opts = BuildOptions {
        width: w,
        height: h,
        theme,
        buddy,
    };
    match render_still(r, &opts, None) {
        Ok(s) => Some(s),
        Err(BuildError::ThemeLocked(_)) => None,
        Err(e) => panic!("seed {}: {e}", r.seed),
    }
}

/// The requested theme if the recipe has it, otherwise the one it has.
pub fn available_theme(r: &SceneRecipe, wanted: Theme) -> Theme {
    if r.tod_for(wanted).is_some() {
        wanted
    } else if wanted == Theme::Light {
        Theme::Dark
    } else {
        Theme::Light
    }
}

/// Stable FNV-1a hash of an image's size, palette and pixels.
pub fn image_hash(img: &IndexedImage, palette_rgb: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    feed(&img.width.to_le_bytes());
    feed(&img.height.to_le_bytes());
    feed(palette_rgb);
    feed(&img.pixels);
    h
}

/// Decodes an indexed PNG into `(image, palette rgb bytes)`.
pub fn decode_indexed_png(bytes: Vec<u8>) -> (IndexedImage, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("png header");
    let palette = reader
        .info()
        .palette
        .as_ref()
        .expect("indexed png")
        .to_vec();
    let mut buf = vec![0u8; reader.output_buffer_size().expect("buffer size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    assert_eq!(info.color_type, png::ColorType::Indexed);
    buf.truncate(info.buffer_size());
    (
        IndexedImage {
            width: info.width,
            height: info.height,
            pixels: buf,
        },
        palette,
    )
}
