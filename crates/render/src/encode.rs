//! PNG export. Scenes have at most 255 colours, so stills are written as indexed
//! (paletted) PNGs, which are much smaller than RGBA.

use crate::IndexedImage;
use crate::palette::IndexedPalette;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Writes an indexed PNG.
pub fn encode_png<W: Write>(
    img: &IndexedImage,
    palette: &IndexedPalette,
    out: W,
) -> Result<(), png::EncodingError> {
    let mut enc = png::Encoder::new(out, img.width, img.height);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(palette.rgb_bytes());
    enc.set_compression(png::Compression::Balanced);
    enc.set_filter(png::Filter::Adaptive);
    let mut writer = enc.write_header()?;
    writer.write_image_data(&img.pixels)?;
    writer.finish()
}

/// Writes an indexed PNG to a file.
pub fn save_png(path: &Path, img: &IndexedImage, palette: &IndexedPalette) -> io::Result<()> {
    let file = BufWriter::new(File::create(path)?);
    encode_png(img, palette, file).map_err(io::Error::other)
}

/// Expands palette indices to RGBA8 (for tests, previews and GPU upload).
pub fn to_rgba(img: &IndexedImage, palette: &IndexedPalette) -> Vec<u8> {
    let colors = palette.colors();
    let mut out = Vec::with_capacity(img.pixels.len() * 4);
    for &i in &img.pixels {
        let c = colors[i as usize];
        out.extend_from_slice(&[c.r, c.g, c.b, 255]);
    }
    out
}
