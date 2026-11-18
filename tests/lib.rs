//! Shared helpers for the workspace integration tests.

use clairlune_render::{IndexedImage, Still, render_still};
use clairlune_scene::{Aspect, BuildOptions, ContentContext, SceneRecipe, Theme, compose};
use std::path::PathBuf;

/// Golden test size (PLAN.md section 11, M1).
pub const GOLDEN_W: u32 = 640;
pub const GOLDEN_H: u32 = 360;
/// The ten fixed golden seeds.
pub const GOLDEN_SEEDS: [u64; 10] = [1, 2, 3, 7, 42, 99, 1234, 31337, 271_828, 9_000_000_001];

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
