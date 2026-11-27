//! Image-to-ASCII conversion and custom ASCII wallpaper packaging.
//!
//! Converts an arbitrary RGBA image into:
//! - plain multi-line ASCII/Unicode text (`.txt` / clipboard),
//! - an anti-aliased RGBA image (including transparent background when asked),
//! - a genuine [`ScenePackage`] (with both light and dark themes when using a
//!   Clairlune palette or photo colours, plus optional live weather particles),
//!   playable natively in Still, Live, Library, Archive, Timeline and GIF export.

use crate::IndexedImage;
use crate::atlas::GlyphAtlas;
use crate::package::{CustomTheme, ScenePackage};
use crate::palette::{IndexedPalette, SPRITE_BASE, TRANSPARENT_INDEX, ink_level};
use crate::sign::SIGNATURE;
use clairlune_scene::graph::{ADVANCE_RATIO, FONT_TO_CELL, GridSpec, Lighting, SceneGraph};
use clairlune_scene::particles::{
    CALM_PACE, Emitter, EmitterKind, LOOP_MS, WeatherKind, ember_set, hero_time, weather_set,
};
use clairlune_scene::recipe::{Composition, PalettePick};
use clairlune_scene::{
    Aspect, GENERATOR_VERSION, PaletteFamily, Rgb, SceneRecipe, Theme, TimeOfDay,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Archetype marker stored on custom image-to-ASCII recipes.
pub const PHOTO_ARCHETYPE: &str = "photo";

/// Decoded RGBA8 source image, row-major (`width * height * 4` bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        let want = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| "image dimensions overflow".to_string())?;
        if width == 0 || height == 0 || pixels.len() != want {
            return Err(format!(
                "invalid RGBA image buffer ({} bytes for {width}x{height})",
                pixels.len()
            ));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Downscales the image if its longest side exceeds `max_side`, keeping
    /// saved archive/library copies compact while preserving full ASCII detail.
    pub fn capped(&self, max_side: u32) -> Self {
        let longest = self.width.max(self.height);
        if longest <= max_side || max_side == 0 {
            return self.clone();
        }
        let nw = ((u64::from(self.width) * u64::from(max_side)) / u64::from(longest)).max(1) as u32;
        let nh =
            ((u64::from(self.height) * u64::from(max_side)) / u64::from(longest)).max(1) as u32;
        let mut out = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            let y0 = f64::from(y) * f64::from(self.height) / f64::from(nh);
            let y1 = f64::from(y + 1) * f64::from(self.height) / f64::from(nh);
            for x in 0..nw {
                let x0 = f64::from(x) * f64::from(self.width) / f64::from(nw);
                let x1 = f64::from(x + 1) * f64::from(self.width) / f64::from(nw);
                let s = sample_box(self, x0, y0, x1, y1);
                let idx = ((y * nw + x) * 4) as usize;
                out[idx..idx + 4].copy_from_slice(&[s.r, s.g, s.b, s.a]);
            }
        }
        Self {
            width: nw,
            height: nh,
            pixels: out,
        }
    }
}

/// Line-height / character aspect ratio for standalone `.txt` and PNG output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowDensity {
    /// Matches Clairlune's JetBrains Mono cell geometry (`0.54`).
    #[default]
    Auto,
    /// Tighter terminal spacing (`0.45`).
    Compact,
    /// Classic 1:2 terminal cell (`0.50`).
    Standard,
    /// Looser editor/prose line spacing (`0.62`).
    Tall,
}

impl RowDensity {
    /// Width-to-height ratio of one character cell.
    pub fn cell_aspect(self) -> f64 {
        match self {
            RowDensity::Auto => ADVANCE_RATIO * FONT_TO_CELL,
            RowDensity::Compact => 0.45,
            RowDensity::Standard => 0.50,
            RowDensity::Tall => 0.62,
        }
    }
}

/// Character ramp ordered from heaviest ink to lightest (`' '` last in the UI).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RampPreset {
    #[default]
    Standard,
    Detailed,
    Dither,
    Braille,
    Blocks,
    Lines,
    Kana,
    Shapes,
    Hex,
    Custom,
}

impl RampPreset {
    pub const ALL: [RampPreset; 10] = [
        RampPreset::Standard,
        RampPreset::Detailed,
        RampPreset::Dither,
        RampPreset::Braille,
        RampPreset::Blocks,
        RampPreset::Lines,
        RampPreset::Kana,
        RampPreset::Shapes,
        RampPreset::Hex,
        RampPreset::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RampPreset::Standard => "Standard, 10 levels",
            RampPreset::Detailed => "Detailed, 70 levels",
            RampPreset::Dither => "Dithered, 7 levels",
            RampPreset::Braille => "Braille, 2×4 dots",
            RampPreset::Blocks => "Blocks, 5 levels",
            RampPreset::Lines => "Box lines, 8 levels",
            RampPreset::Kana => "Katakana, 10 levels",
            RampPreset::Shapes => "Geometric shapes, 9 levels",
            RampPreset::Hex => "Hex digits, 9 levels",
            RampPreset::Custom => "Custom ramp",
        }
    }

    /// Characters ordered heaviest to lightest (as shown in the UI).
    pub fn heavy_to_light(self, custom: &str) -> Vec<char> {
        let s = match self {
            RampPreset::Standard => "@%#*+=-:. ",
            RampPreset::Detailed => {
                "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/|()1{}[]?-_+~<>i!lI;:,\"^`'. "
            }
            RampPreset::Dither => "■●━▪•· ",
            RampPreset::Braille => "⣿⣷⣯⣟⡿⢿⣻⣽⣾⣶⣦⣤⣄⣀⡀ ",
            RampPreset::Blocks => "█▓▒░ ",
            RampPreset::Lines => "╬╋╪┼═─· ",
            RampPreset::Kana => "ヰモムヌメミニー・ ",
            RampPreset::Shapes => "■●◆▪▫○◦· ",
            RampPreset::Hex => "8BECF731 ",
            RampPreset::Custom => {
                let mut chars: Vec<char> = Vec::new();
                for ch in custom.chars() {
                    if ch != ' '
                        && !ch.is_control()
                        && GlyphAtlas::can_render_char(ch)
                        && !chars.contains(&ch)
                    {
                        chars.push(ch);
                        if chars.len() >= 250 {
                            break;
                        }
                    }
                }
                if chars.is_empty() {
                    chars = "@%#*+=-:.".chars().collect();
                }
                chars.push(' ');
                return chars;
            }
        };
        s.chars().collect()
    }

    /// Characters ordered lightest (`' '`) to heaviest (for density indexing).
    pub fn light_to_heavy(self, custom: &str) -> Vec<char> {
        let mut v = self.heavy_to_light(custom);
        v.reverse();
        v
    }

    /// Matching Clairlune letter set for ambient weather particles.
    pub fn weather_letters(self) -> &'static str {
        match self {
            RampPreset::Braille => "braille",
            RampPreset::Blocks => "blocks",
            RampPreset::Lines => "lines",
            RampPreset::Kana => "kana",
            RampPreset::Shapes => "shapes",
            RampPreset::Hex => "hex",
            _ => "text",
        }
    }
}

/// How the converted ASCII art is coloured and backed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "palette", rename_all = "snake_case")]
pub enum AsciiColorMode {
    /// One of Clairlune's 12 palettes (both light and dark themes).
    Palette(String),
    /// Pure black background (`#000000`) with white characters.
    Black,
    /// Pure white background (`#FFFFFF`) with black characters.
    White,
    /// Transparent background on PNG export; dark polarity by default.
    None,
    /// Each character takes its colour from the source image.
    Photo,
}

impl Default for AsciiColorMode {
    fn default() -> Self {
        AsciiColorMode::Palette("classic".into())
    }
}

/// How an image fits a monitor whose aspect ratio differs from the image's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    Cover,
    #[default]
    Contain,
}

/// Full user-configurable settings for an image-to-ASCII conversion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsciiConfig {
    /// Number of columns across (`20..=320`).
    pub columns: u32,
    #[serde(default)]
    pub row_density: RowDensity,
    #[serde(default)]
    pub ramp: RampPreset,
    #[serde(default = "default_custom_ramp")]
    pub custom_ramp: String,
    /// Contrast percentage (`50..=250`; `100` is neutral, `120` is crisp).
    pub contrast: i32,
    /// Brightness shift (`-100..=100`; `0` is neutral).
    pub brightness: i32,
    /// Invert light/dark polarity.
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub color_mode: AsciiColorMode,
    /// Optional ambient weather id (`"none"`, `"snow"`, `"fireflies"`, ...).
    #[serde(default = "none_weather")]
    pub weather: String,
    #[serde(default)]
    pub fit: FitMode,
}

fn default_custom_ramp() -> String {
    "@%#*+=-:. ".into()
}

fn none_weather() -> String {
    "none".into()
}

impl Default for AsciiConfig {
    fn default() -> Self {
        Self {
            columns: 110,
            row_density: RowDensity::Auto,
            ramp: RampPreset::Standard,
            custom_ramp: default_custom_ramp(),
            contrast: 120,
            brightness: 0,
            invert: false,
            color_mode: AsciiColorMode::Palette("classic".into()),
            weather: none_weather(),
            fit: FitMode::Contain,
        }
    }
}

impl AsciiConfig {
    /// Whether `theme` uses dark-background polarity (characters represent light)
    /// before applying `self.invert`.
    pub fn dark_canvas(&self, theme: Theme) -> bool {
        match &self.color_mode {
            AsciiColorMode::Black => true,
            AsciiColorMode::White => false,
            AsciiColorMode::Palette(_) | AsciiColorMode::Photo | AsciiColorMode::None => {
                theme == Theme::Dark
            }
        }
    }

    /// Whether `self.weather` resolves to an animated particle emitter.
    pub fn has_live_weather(&self) -> bool {
        WeatherKind::from_id(&self.weather).is_some()
    }
}

/// Converted character grid with per-cell colour, opacity and plain text.
#[derive(Clone, Debug)]
pub struct AsciiArt {
    pub cols: u32,
    pub rows: u32,
    pub chars: Vec<char>,
    pub colors: Vec<[u8; 3]>,
    /// Sampled source alpha (`0..=255`) per cell.
    pub alphas: Vec<u8>,
    /// Multi-line text (`\n`-separated rows, trailing spaces trimmed on empty right margins
    /// kept uniform as exact `cols` characters per row so alignment never shifts).
    pub text: String,
}

#[derive(Clone, Copy, Default)]
struct Sample {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

/// Exact area-weighted box average over `[x0, x1] x [y0, y1]` in source pixel space.
fn sample_box(src: &RgbaImage, x0: f64, y0: f64, x1: f64, y1: f64) -> Sample {
    let w = src.width as i32;
    let h = src.height as i32;
    let x0 = x0.clamp(0.0, f64::from(src.width));
    let y0 = y0.clamp(0.0, f64::from(src.height));
    let x1 = x1.clamp(x0, f64::from(src.width));
    let y1 = y1.clamp(y0, f64::from(src.height));
    let ix0 = (x0.floor() as i32).clamp(0, w - 1);
    let iy0 = (y0.floor() as i32).clamp(0, h - 1);
    let ix1 = (x1.ceil() as i32).clamp(ix0 + 1, w);
    let iy1 = (y1.ceil() as i32).clamp(iy0 + 1, h);

    let mut wr = 0.0;
    let mut wg = 0.0;
    let mut wb = 0.0;
    let mut wa = 0.0;
    let mut w_rgb = 0.0;
    let mut w_tot = 0.0;

    for py in iy0..iy1 {
        let oy = (f64::from(py + 1).min(y1) - f64::from(py).max(y0)).max(0.0);
        if oy <= 0.0 {
            continue;
        }
        for px in ix0..ix1 {
            let ox = (f64::from(px + 1).min(x1) - f64::from(px).max(x0)).max(0.0);
            let area = ox * oy;
            if area <= 0.0 {
                continue;
            }
            let idx = ((py as u32 * src.width + px as u32) * 4) as usize;
            let r = f64::from(src.pixels[idx]);
            let g = f64::from(src.pixels[idx + 1]);
            let b = f64::from(src.pixels[idx + 2]);
            let a = f64::from(src.pixels[idx + 3]);
            let alpha_w = area * (a / 255.0);
            wr += r * alpha_w;
            wg += g * alpha_w;
            wb += b * alpha_w;
            w_rgb += alpha_w;
            wa += a * area;
            w_tot += area;
        }
    }
    if w_tot <= 0.0 {
        return Sample::default();
    }
    let a = (wa / w_tot).round().clamp(0.0, 255.0) as u8;
    if w_rgb <= 1e-6 {
        return Sample {
            r: 0,
            g: 0,
            b: 0,
            a,
        };
    }
    Sample {
        r: (wr / w_rgb).round().clamp(0.0, 255.0) as u8,
        g: (wg / w_rgb).round().clamp(0.0, 255.0) as u8,
        b: (wb / w_rgb).round().clamp(0.0, 255.0) as u8,
        a,
    }
}

/// Applies brightness (`-100..=100`), contrast (`50..=250`), alpha compositing
/// over the canvas polarity, and optional inversion to produce ink density in `0..=1`.
fn ink_density(s: Sample, config: &AsciiConfig, dark_canvas: bool) -> f64 {
    if s.a < 16 {
        return 0.0;
    }
    let alpha = f64::from(s.a) / 255.0;
    let bg_luma = if dark_canvas { 0.0 } else { 1.0 };
    let raw_luma =
        (0.2126 * f64::from(s.r) + 0.7152 * f64::from(s.g) + 0.0722 * f64::from(s.b)) / 255.0;
    let luma = raw_luma * alpha + bg_luma * (1.0 - alpha);
    let bright = luma + f64::from(config.brightness.clamp(-100, 100)) / 180.0;
    let contrast = f64::from(config.contrast.clamp(40, 300)) / 100.0;
    let adjusted = ((bright - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
    let polarity = if dark_canvas {
        adjusted
    } else {
        1.0 - adjusted
    };
    if config.invert {
        1.0 - polarity
    } else {
        polarity
    }
}

/// Converts `src` using its natural aspect ratio (for the Studio preview, `.txt`
/// export and standalone PNG export).
pub fn convert_standalone(src: &RgbaImage, config: &AsciiConfig, theme: Theme) -> AsciiArt {
    let cols = config.columns.clamp(20, 320);
    let img_aspect = f64::from(src.height) / f64::from(src.width.max(1));
    let rows = ((f64::from(cols) * img_aspect * config.row_density.cell_aspect()).round() as u32)
        .clamp(8, 320);
    convert_grid(src, config, theme, cols, rows, None)
}

/// Converts `src` onto a fixed `(cols, rows)` wallpaper grid, cropping (`Cover`)
/// or centring (`Contain`) according to `config.fit` and cell pixel aspect `cell_w / cell_h`.
pub fn convert_for_grid(
    src: &RgbaImage,
    config: &AsciiConfig,
    theme: Theme,
    cols: u32,
    rows: u32,
    cell_w: u32,
    cell_h: u32,
) -> AsciiArt {
    let grid_px_w = f64::from(cols * cell_w.max(1));
    let grid_px_h = f64::from(rows * cell_h.max(1));
    convert_grid(src, config, theme, cols, rows, Some((grid_px_w, grid_px_h)))
}

fn convert_grid(
    src: &RgbaImage,
    config: &AsciiConfig,
    theme: Theme,
    cols: u32,
    rows: u32,
    screen_px: Option<(f64, f64)>,
) -> AsciiArt {
    let dark_canvas = config.dark_canvas(theme);
    let ramp = config.ramp.light_to_heavy(&config.custom_ramp);
    let n = (cols * rows) as usize;
    let mut chars = vec![' '; n];
    let mut colors = vec![[0u8; 3]; n];
    let mut alphas = vec![0u8; n];

    let sw = f64::from(src.width);
    let sh = f64::from(src.height);

    // Map cell coordinates [0..cols] x [0..rows] into source pixel space.
    // Either full source (standalone), Cover crop, or Contain letterbox.
    let (crop_x, crop_y, crop_w, crop_h, dst_x0, dst_y0, dst_w, dst_h) = match screen_px {
        None => (0.0, 0.0, sw, sh, 0.0, 0.0, f64::from(cols), f64::from(rows)),
        Some((gw, gh)) => {
            let src_ratio = sw / sh.max(1.0);
            let dst_ratio = gw / gh.max(1.0);
            match config.fit {
                FitMode::Cover => {
                    if src_ratio > dst_ratio {
                        let cw = sh * dst_ratio;
                        (
                            (sw - cw) * 0.5,
                            0.0,
                            cw,
                            sh,
                            0.0,
                            0.0,
                            f64::from(cols),
                            f64::from(rows),
                        )
                    } else {
                        let ch = sw / dst_ratio;
                        (
                            0.0,
                            (sh - ch) * 0.5,
                            sw,
                            ch,
                            0.0,
                            0.0,
                            f64::from(cols),
                            f64::from(rows),
                        )
                    }
                }
                FitMode::Contain => {
                    if src_ratio > dst_ratio {
                        let dh = (f64::from(rows) * (dst_ratio / src_ratio)).max(1.0);
                        (
                            0.0,
                            0.0,
                            sw,
                            sh,
                            0.0,
                            (f64::from(rows) - dh).max(0.0) * 0.5,
                            f64::from(cols),
                            dh,
                        )
                    } else {
                        let dw = (f64::from(cols) * (src_ratio / dst_ratio)).max(1.0);
                        (
                            0.0,
                            0.0,
                            sw,
                            sh,
                            (f64::from(cols) - dw).max(0.0) * 0.5,
                            0.0,
                            dw,
                            f64::from(rows),
                        )
                    }
                }
            }
        }
    };

    let canvas_bg = resolve_family(&config.color_mode, theme).bg_center;
    const BAYER_2X4: [[f64; 2]; 4] = [[-0.07, 0.02], [0.05, -0.04], [-0.02, 0.07], [0.04, -0.05]];
    const BRAILLE_BITS: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

    for row in 0..rows {
        let ry0 = f64::from(row);
        let ry1 = f64::from(row + 1);
        if ry1 <= dst_y0 || ry0 >= dst_y0 + dst_h {
            continue;
        }
        let u_y0 = ((ry0 - dst_y0) / dst_h).clamp(0.0, 1.0);
        let u_y1 = ((ry1 - dst_y0) / dst_h).clamp(0.0, 1.0);
        let sy0 = crop_y + u_y0 * crop_h;
        let sy1 = crop_y + u_y1 * crop_h;

        for col in 0..cols {
            let cx0 = f64::from(col);
            let cx1 = f64::from(col + 1);
            if cx1 <= dst_x0 || cx0 >= dst_x0 + dst_w {
                continue;
            }
            let u_x0 = ((cx0 - dst_x0) / dst_w).clamp(0.0, 1.0);
            let u_x1 = ((cx1 - dst_x0) / dst_w).clamp(0.0, 1.0);
            let sx0 = crop_x + u_x0 * crop_w;
            let sx1 = crop_x + u_x1 * crop_w;

            let idx = (row * cols + col) as usize;
            let cell_sample = sample_box(src, sx0, sy0, sx1, sy1);
            if cell_sample.a == 0 {
                continue;
            }
            let composited = if cell_sample.a < 255 {
                let t = f64::from(cell_sample.a) / 255.0;
                let mixed = canvas_bg.mix(
                    Rgb {
                        r: cell_sample.r,
                        g: cell_sample.g,
                        b: cell_sample.b,
                    },
                    t,
                );
                Sample {
                    r: mixed.r,
                    g: mixed.g,
                    b: mixed.b,
                    a: cell_sample.a,
                }
            } else {
                cell_sample
            };
            colors[idx] = readable_photo_rgb(composited, dark_canvas);

            if config.ramp == RampPreset::Braille {
                // 2x4 sub-cell braille sampling for sharp edges and stippled gradients.
                let mut mask = 0u8;
                let mut sum = 0.0;
                let mut max_sub_a = cell_sample.a;
                for (dy, bit_row) in BRAILLE_BITS.iter().enumerate() {
                    let by0 = sy0 + (sy1 - sy0) * (dy as f64 / 4.0);
                    let by1 = sy0 + (sy1 - sy0) * ((dy + 1) as f64 / 4.0);
                    for (dx, &bit) in bit_row.iter().enumerate() {
                        let bx0 = sx0 + (sx1 - sx0) * (dx as f64 / 2.0);
                        let bx1 = sx0 + (sx1 - sx0) * ((dx + 1) as f64 / 2.0);
                        let sub = sample_box(src, bx0, by0, bx1, by1);
                        max_sub_a = max_sub_a.max(sub.a);
                        let d = ink_density(sub, config, dark_canvas);
                        sum += d;
                        if d + BAYER_2X4[dy][dx] >= 0.34 {
                            mask |= 1 << bit;
                        }
                    }
                }
                if max_sub_a < 16 {
                    continue;
                }
                alphas[idx] = if mask != 0 { max_sub_a } else { cell_sample.a };
                if mask == 0 && sum / 8.0 > 0.14 {
                    mask = 1 << 6;
                    alphas[idx] = max_sub_a;
                }
                chars[idx] = if mask == 0 {
                    ' '
                } else {
                    char::from_u32(0x2800 + u32::from(mask)).unwrap_or(' ')
                };
            } else {
                if cell_sample.a < 16 {
                    continue;
                }
                alphas[idx] = cell_sample.a;
                let d = ink_density(cell_sample, config, dark_canvas);
                let max_i = ramp.len().saturating_sub(1);
                let ri = (d * max_i as f64).round().clamp(0.0, max_i as f64) as usize;
                chars[idx] = ramp[ri];
            }
        }
    }

    let mut text = String::with_capacity(((cols + 1) * rows) as usize);
    for r in 0..rows {
        if r > 0 {
            text.push('\n');
        }
        let start = (r * cols) as usize;
        let line: String = chars[start..start + cols as usize].iter().collect();
        text.push_str(line.trim_end_matches(' '));
    }

    AsciiArt {
        cols,
        rows,
        chars,
        colors,
        alphas,
        text,
    }
}

/// Ensures photo-coloured glyphs keep legible contrast against the chosen canvas.
fn readable_photo_rgb(s: Sample, dark_canvas: bool) -> [u8; 3] {
    let luma = (u32::from(s.r) * 2126 + u32::from(s.g) * 7152 + u32::from(s.b) * 722) / 10_000;
    if dark_canvas && luma < 48 {
        let lift = (48 - luma) as u8;
        [
            s.r.saturating_add(lift),
            s.g.saturating_add(lift),
            s.b.saturating_add(lift),
        ]
    } else if !dark_canvas && luma > 212 {
        let drop = (luma - 212) as u8;
        [
            s.r.saturating_sub(drop),
            s.g.saturating_sub(drop),
            s.b.saturating_sub(drop),
        ]
    } else {
        [s.r, s.g, s.b]
    }
}

/// Resolves the `PaletteFamily` for a `(color_mode, theme)` pair.
pub fn resolve_family(mode: &AsciiColorMode, theme: Theme) -> PaletteFamily {
    match mode {
        AsciiColorMode::Palette(set) => clairlune_scene::palette::families_in(set, theme)
            .next()
            .copied()
            .or_else(|| {
                clairlune_scene::palette::families_for(theme)
                    .next()
                    .copied()
            })
            .expect("classic palette exists"),
        AsciiColorMode::Black => PaletteFamily {
            id: "ascii_black",
            set: "classic",
            theme: Theme::Dark,
            bg_edge: Rgb::hex(0x000000),
            bg_center: Rgb::hex(0x000000),
            ink: Rgb::hex(0xF2F2F2),
            accent: Rgb::hex(0xFF9646),
            ember: Rgb::hex(0xFFB066),
            glow_strength: 0.14,
            tint_strength: 0.50,
        },
        AsciiColorMode::White => PaletteFamily {
            id: "ascii_white",
            set: "classic",
            theme: Theme::Light,
            bg_edge: Rgb::hex(0xFFFFFF),
            bg_center: Rgb::hex(0xFFFFFF),
            ink: Rgb::hex(0x141414),
            accent: Rgb::hex(0xD9652B),
            ember: Rgb::hex(0xCF5320),
            glow_strength: 0.10,
            tint_strength: 0.45,
        },
        AsciiColorMode::None => match theme {
            Theme::Dark => PaletteFamily {
                id: "ascii_black",
                set: "classic",
                theme: Theme::Dark,
                bg_edge: Rgb::hex(0x000000),
                bg_center: Rgb::hex(0x000000),
                ink: Rgb::hex(0xF2F2F2),
                accent: Rgb::hex(0xFF9646),
                ember: Rgb::hex(0xFFB066),
                glow_strength: 0.14,
                tint_strength: 0.50,
            },
            Theme::Light => PaletteFamily {
                id: "ascii_white",
                set: "classic",
                theme: Theme::Light,
                bg_edge: Rgb::hex(0xFFFFFF),
                bg_center: Rgb::hex(0xFFFFFF),
                ink: Rgb::hex(0x141414),
                accent: Rgb::hex(0xD9652B),
                ember: Rgb::hex(0xCF5320),
                glow_strength: 0.10,
                tint_strength: 0.45,
            },
        },
        AsciiColorMode::Photo => match theme {
            Theme::Dark => PaletteFamily {
                id: "photo_dark",
                set: "classic",
                theme: Theme::Dark,
                bg_edge: Rgb::hex(0x0B0C10),
                bg_center: Rgb::hex(0x13151C),
                ink: Rgb::hex(0xE9E5D7),
                accent: Rgb::hex(0xFF9646),
                ember: Rgb::hex(0xFFA65C),
                glow_strength: 0.16,
                tint_strength: 0.55,
            },
            Theme::Light => PaletteFamily {
                id: "photo_light",
                set: "classic",
                theme: Theme::Light,
                bg_edge: Rgb::hex(0xEAE6DC),
                bg_center: Rgb::hex(0xF6F3EC),
                ink: Rgb::hex(0x1C1A17),
                accent: Rgb::hex(0xD9652B),
                ember: Rgb::hex(0xCF5320),
                glow_strength: 0.10,
                tint_strength: 0.50,
            },
        },
    }
}

/// Rasterizes every unique character in `chars` in chunks of `<= 200` glyphs,
/// returning `(per_cell_glyph_id, contiguous_masks)` with `0` reserved for `' '`.
fn rasterize_unique_chars(
    chars: &[char],
    cell_w: u32,
    cell_h: u32,
    font_px: f64,
) -> (Vec<u16>, Vec<u8>) {
    let mut unique: Vec<char> = vec![' '];
    let mut map: HashMap<char, u16> = HashMap::new();
    map.insert(' ', 0);
    let mut ids = Vec::with_capacity(chars.len());
    for &c in chars {
        let id = if let Some(&id) = map.get(&c) {
            id
        } else {
            let id = unique.len() as u16;
            unique.push(c);
            map.insert(c, id);
            id
        };
        ids.push(id);
    }
    let cell_len = (cell_w * cell_h) as usize;
    let mut all_masks = vec![0u8; unique.len() * cell_len];
    for (chunk_idx, chunk) in unique.chunks(200).enumerate() {
        let atlas = GlyphAtlas::for_char_set(chunk, cell_w, cell_h, font_px);
        for (local_i, _) in chunk.iter().enumerate() {
            let global_i = chunk_idx * 200 + local_i;
            all_masks[global_i * cell_len..(global_i + 1) * cell_len]
                .copy_from_slice(atlas.mask(local_i as u8));
        }
    }
    (ids, all_masks)
}

/// Renders `art` to an RGBA8 image (`width, height, pixels`), supporting
/// transparent backgrounds (`AsciiColorMode::None`), photo colours, palettes,
/// and optional corner signature on opaque exports.
pub fn render_rgba(
    art: &AsciiArt,
    config: &AsciiConfig,
    theme: Theme,
    cell_h: u32,
    sign: bool,
) -> (u32, u32, Vec<u8>) {
    let cell_h = cell_h.max(8);
    let cell_w = ((f64::from(cell_h) * FONT_TO_CELL * ADVANCE_RATIO).round() as u32).max(4);
    let transparent = config.color_mode == AsciiColorMode::None;
    let pad_x = cell_w * 2;
    let pad_y = cell_h;
    let sig_pad_bottom = if sign && !transparent {
        cell_h.max(16) + 8
    } else {
        0
    };
    let width = art.cols * cell_w + pad_x * 2;
    let height = art.rows * cell_h + pad_y * 2 + sig_pad_bottom;

    let family = resolve_family(&config.color_mode, theme);
    let lighting = Lighting {
        vignette_cx: f64::from(width) * 0.5,
        vignette_cy: f64::from(height) * 0.5,
        vignette_rx: f64::from(width) * 0.78,
        vignette_ry: f64::from(height) * 0.78,
        glow: None,
    };

    let mut rgba = vec![0u8; (width * height * 4) as usize];
    if !transparent {
        let flat = matches!(
            config.color_mode,
            AsciiColorMode::Black | AsciiColorMode::White
        );
        for y in 0..height {
            let rt = lighting.row_terms(y);
            for x in 0..width {
                let bg = if flat {
                    family.bg_center
                } else {
                    let b = lighting.band_in_row(&rt, x).min(3);
                    family.bg_edge.mix(family.bg_center, f64::from(b) / 3.0)
                };
                let o = ((y * width + x) * 4) as usize;
                rgba[o] = bg.r;
                rgba[o + 1] = bg.g;
                rgba[o + 2] = bg.b;
                rgba[o + 3] = 255;
            }
        }
    }

    let font_px = f64::from(cell_h) * FONT_TO_CELL;
    let (glyph_ids, all_masks) = rasterize_unique_chars(&art.chars, cell_w, cell_h, font_px);
    let cell_len = (cell_w * cell_h) as usize;

    for row in 0..art.rows {
        for col in 0..art.cols {
            let ci = (row * art.cols + col) as usize;
            let gid = glyph_ids[ci] as usize;
            if gid == 0 {
                continue;
            }
            let mask = &all_masks[gid * cell_len..(gid + 1) * cell_len];
            let ink = if config.color_mode == AsciiColorMode::Photo {
                let [r, g, b] = art.colors[ci];
                Rgb { r, g, b }
            } else {
                family.ink
            };
            let x0 = pad_x + col * cell_w;
            let y0 = pad_y + row * cell_h;
            for my in 0..cell_h {
                for mx in 0..cell_w {
                    let a = mask[(my * cell_w + mx) as usize];
                    if a == 0 {
                        continue;
                    }
                    let o = (((y0 + my) * width + (x0 + mx)) * 4) as usize;
                    if transparent {
                        let scaled_a =
                            ((u16::from(a) * u16::from(art.alphas[ci]) + 127) / 255) as u8;
                        if scaled_a == 0 {
                            continue;
                        }
                        rgba[o] = ink.r;
                        rgba[o + 1] = ink.g;
                        rgba[o + 2] = ink.b;
                        rgba[o + 3] = scaled_a;
                    } else {
                        let t = f64::from(a) / 255.0;
                        let cur = Rgb {
                            r: rgba[o],
                            g: rgba[o + 1],
                            b: rgba[o + 2],
                        };
                        let m = cur.mix(ink, t);
                        rgba[o] = m.r;
                        rgba[o + 1] = m.g;
                        rgba[o + 2] = m.b;
                        rgba[o + 3] = 255;
                    }
                }
            }
        }
    }

    if sign && !transparent {
        stamp_rgba_signature(&mut rgba, width, height, family.ink);
    }

    (width, height, rgba)
}

fn stamp_rgba_signature(rgba: &mut [u8], width: u32, height: u32, ink: Rgb) {
    let spec = clairlune_scene::grid_spec(width, height);
    let chars: Vec<char> = SIGNATURE.chars().collect();
    let n = chars.len() as u32;
    if spec.cols < 2 * (n + 3) || spec.rows < 6 {
        return;
    }
    let Ok(atlas) = GlyphAtlas::with_chars(
        crate::atlas::FONT_DATA,
        &chars,
        spec.cell_w,
        spec.cell_h,
        spec.font_px,
    ) else {
        return;
    };
    let x0 = spec.origin_x + (spec.cols - n - 2) * spec.cell_w;
    let y0 = spec.origin_y + (spec.rows - 2) * spec.cell_h;
    for c in 0..n {
        let mask = atlas.mask(c as u8);
        for my in 0..spec.cell_h {
            for mx in 0..spec.cell_w {
                let a = mask[(my * spec.cell_w + mx) as usize];
                if a == 0 {
                    continue;
                }
                let px = x0 + c * spec.cell_w + mx;
                let py = y0 + my;
                if px >= width || py >= height {
                    continue;
                }
                let o = ((py * width + px) * 4) as usize;
                let cur = Rgb {
                    r: rgba[o],
                    g: rgba[o + 1],
                    b: rgba[o + 2],
                };
                let m = cur.mix(ink, (f64::from(a) / 255.0) * 0.65);
                rgba[o] = m.r;
                rgba[o + 1] = m.g;
                rgba[o + 2] = m.b;
            }
        }
    }
}

/// Encodes an RGBA8 image buffer as a PNG.
pub fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Builds a frozen [`ScenePackage`] at `(width, height)` for a user image,
/// complete with both themes (unless fixed to Black or White) and optional
/// live weather particles.
pub fn build_photo_package(
    src: &RgbaImage,
    config: &AsciiConfig,
    width: u32,
    height: u32,
    seed: u64,
) -> Result<ScenePackage, String> {
    if width < 160 || height < 90 {
        return Err(format!("{width}x{height} is too small (minimum 160x90)"));
    }

    let wanted_cols = config.columns.clamp(24, 320);
    let max_cell_w = (width / wanted_cols).max(3);
    let cell_h_from_cols =
        ((f64::from(max_cell_w) / (ADVANCE_RATIO * FONT_TO_CELL)).round() as u32).max(6);
    let font_px = f64::from(cell_h_from_cols) * FONT_TO_CELL;
    let cell_w = ((font_px * ADVANCE_RATIO).round() as u32)
        .clamp(3, width)
        .min(max_cell_w);
    let cell_h = cell_h_from_cols.min(height);
    let cols = (width / cell_w).max(1);
    let rows = (height / cell_h).max(1);
    let ascii_grid = GridSpec {
        cols,
        rows,
        cell_w,
        cell_h,
        origin_x: (width - cols * cell_w) / 2,
        origin_y: (height - rows * cell_h) / 2,
        font_px,
    };
    // Weather particles use the screen's standard scene grid so snowflakes/rain
    // stay crisp regardless of how wide or narrow `config.columns` is set.
    let scene_grid = clairlune_scene::grid_spec(width, height);

    let themes_to_build: &[Theme] = match config.color_mode {
        AsciiColorMode::Black => &[Theme::Dark],
        AsciiColorMode::White => &[Theme::Light],
        AsciiColorMode::Palette(_) | AsciiColorMode::None | AsciiColorMode::Photo => {
            &[Theme::Dark, Theme::Light]
        }
    };

    let letters = config.ramp.weather_letters();
    let weather_atlas = GlyphAtlas::for_letters(&scene_grid, letters);
    let embers = ember_set(seed, CALM_PACE);
    let t_hero_ms = hero_time(&embers);
    let aspect = f64::from(width) / f64::from(height);
    let unit = f64::from(width.min(height));

    let mut emitters = Vec::new();
    if let Some(kind) = WeatherKind::from_id(&config.weather) {
        emitters.push(Emitter {
            kind: EmitterKind::Weather(kind),
            origin_x: 0.0,
            origin_y: 0.0,
            unit,
            intensity: 1.0,
            embers: Vec::new(),
            drops: weather_set(kind, seed, aspect, CALM_PACE),
            size: (f64::from(width), f64::from(height)),
            pace: CALM_PACE,
        });
    }

    let mut custom_themes = Vec::with_capacity(themes_to_build.len());
    for &theme in themes_to_build {
        let tod = match theme {
            Theme::Dark => TimeOfDay::Night,
            Theme::Light => TimeOfDay::Morning,
        };
        let family = resolve_family(&config.color_mode, theme);
        let art = convert_for_grid(src, config, theme, cols, rows, cell_w, cell_h);
        let lighting = Lighting {
            vignette_cx: f64::from(width) * 0.5,
            vignette_cy: f64::from(height) * 0.5,
            vignette_rx: f64::from(width) * 0.78,
            vignette_ry: f64::from(height) * 0.78,
            glow: None,
        };

        let graph = SceneGraph {
            width,
            height,
            theme,
            tod,
            buddy: false,
            seed,
            family,
            grid: scene_grid,
            cells: Vec::new(),
            lighting,
            activity: None,
            props: Vec::new(),
            unattended: Vec::new(),
            character: None,
            emitters: emitters.clone(),
            ground_y: height as i32,
            t_hero_ms,
            loop_ms: LOOP_MS,
            pace: CALM_PACE,
            generator: GENERATOR_VERSION,
            letters: if letters == "text" {
                String::new()
            } else {
                letters.to_string()
            },
            water: None,
            journey: None,
            zoom: 1.0,
        };

        let (background, palette) = bake_ascii_theme(&art, config, &graph, &ascii_grid);
        custom_themes.push(CustomTheme {
            theme,
            graph,
            palette,
            atlas: weather_atlas.clone(),
            background,
            hero_ms: t_hero_ms,
        });
    }

    let (light_family, dark_family) = match &config.color_mode {
        AsciiColorMode::Palette(set) => {
            let l = clairlune_scene::palette::families_in(set, Theme::Light)
                .next()
                .map_or("paper_day", |f| f.id);
            let d = clairlune_scene::palette::families_in(set, Theme::Dark)
                .next()
                .map_or("ember_night", |f| f.id);
            (l.to_string(), d.to_string())
        }
        _ => ("paper_day".to_string(), "ember_night".to_string()),
    };

    let recipe = SceneRecipe {
        generator_version: GENERATOR_VERSION,
        seed,
        aspect: Aspect::from_size(width, height),
        content_ctx_hash: format!("{seed:016x}"),
        activity: None,
        archetype: PHOTO_ARCHETYPE.into(),
        light_tod: themes_to_build
            .contains(&Theme::Light)
            .then_some(TimeOfDay::Morning),
        dark_tod: themes_to_build
            .contains(&Theme::Dark)
            .then_some(TimeOfDay::Night),
        platform: "none".into(),
        props: Vec::new(),
        weather: config.weather.clone(),
        composition: Composition {
            horizon_pm: 800,
            focus_x_pm: 500,
            gap_pm: 500,
            shape_pow_pm: 1000,
            platform_margin_pm: 0,
            cliff_layers: 0,
            prop_left: false,
            motion_speed_cc: 0,
            pace_half_pm: 0,
            fly_lift_pm: 0,
            journey_ms: 0,
            journey_pace_c: 0,
            landmark: String::new(),
        },
        palette: PalettePick {
            light: light_family,
            dark: dark_family,
        },
        look: "line".into(),
        contrast: "normal".into(),
        zoom: "close".into(),
        letters: letters.into(),
    };

    ScenePackage::from_custom(recipe, width, height, custom_themes).map_err(|e| e.to_string())
}

fn bake_ascii_theme(
    art: &AsciiArt,
    config: &AsciiConfig,
    graph: &SceneGraph,
    g: &GridSpec,
) -> (IndexedImage, IndexedPalette) {
    let mut img = IndexedImage::new(graph.width, graph.height);
    let w = graph.width as usize;
    let flat_bg = matches!(
        config.color_mode,
        AsciiColorMode::Black | AsciiColorMode::White | AsciiColorMode::None
    );
    if flat_bg {
        img.pixels.fill(3); // uniform bg_center band
    } else {
        for (y, row) in img.pixels.chunks_exact_mut(w).enumerate() {
            graph.lighting.fill_row(y as u32, row);
        }
    }

    let (cw, ch) = (g.cell_w as usize, g.cell_h as usize);
    let cell_len = cw * ch;
    let (glyph_ids, all_masks) = rasterize_unique_chars(&art.chars, g.cell_w, g.cell_h, g.font_px);

    if config.color_mode == AsciiColorMode::Photo {
        let (palette, cell_slots) = build_photo_palette(graph, art);
        for row in 0..g.rows {
            for col in 0..g.cols {
                let ci = (row * g.cols + col) as usize;
                let gid = glyph_ids[ci] as usize;
                if gid == 0 {
                    continue;
                }
                let base_slot = cell_slots[ci];
                let mask = &all_masks[gid * cell_len..(gid + 1) * cell_len];
                let x0 = g.origin_x as usize + col as usize * cw;
                let y0 = g.origin_y as usize + row as usize * ch;
                for my in 0..ch {
                    let line = (y0 + my) * w + x0;
                    for mx in 0..cw {
                        let a = mask[my * cw + mx];
                        // 3 anti-aliasing levels per quantized colour: faint, medium, full.
                        let step = if a < 48 {
                            continue;
                        } else if a < 128 {
                            0
                        } else if a < 205 {
                            1
                        } else {
                            2
                        };
                        img.pixels[line + mx] = base_slot + step;
                    }
                }
            }
        }
        return (img, palette);
    }

    let palette = IndexedPalette::for_graph(graph);
    for row in 0..g.rows {
        for col in 0..g.cols {
            let ci = (row * g.cols + col) as usize;
            let gid = glyph_ids[ci] as usize;
            if gid == 0 {
                continue;
            }
            let mask = &all_masks[gid * cell_len..(gid + 1) * cell_len];
            let x0 = g.origin_x as usize + col as usize * cw;
            let y0 = g.origin_y as usize + row as usize * ch;
            for my in 0..ch {
                let line = (y0 + my) * w + x0;
                for mx in 0..cw {
                    let a = mask[my * cw + mx];
                    if a == 0 {
                        continue;
                    }
                    let level = ink_level(a, 255);
                    if level > 0 {
                        let band = img.pixels[line + mx];
                        img.pixels[line + mx] = IndexedPalette::ink(band, level);
                    }
                }
            }
        }
    }
    (img, palette)
}

/// Builds an `IndexedPalette` whose `0..104` entries keep standard background,
/// ink and ember ramps (for vignette, live weather and signatures) and whose
/// `104..248` entries hold up to 48 median-cut photo colours × 3 AA blend steps.
fn build_photo_palette(graph: &SceneGraph, art: &AsciiArt) -> (IndexedPalette, Vec<u8>) {
    let base = IndexedPalette::for_graph(graph);
    let mut colors = base.colors()[..SPRITE_BASE as usize].to_vec();

    let active: Vec<[u8; 3]> = art
        .chars
        .iter()
        .zip(&art.colors)
        .filter(|(ch, _)| **ch != ' ')
        .map(|(_, &rgb)| rgb)
        .collect();

    let centroids = median_cut(&active, 48);
    let bg = graph.family.bg_center;
    for &[r, g, b] in &centroids {
        let target = Rgb { r, g, b };
        for t in [0.38, 0.70, 1.0] {
            colors.push(bg.mix(target, t));
        }
    }
    debug_assert!(colors.len() < TRANSPARENT_INDEX as usize);

    let cell_slots = art
        .colors
        .iter()
        .map(|rgb| {
            let best = centroids
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| color_dist2(*rgb, **c))
                .map_or(0, |(i, _)| i);
            SPRITE_BASE + (best as u8) * 3
        })
        .collect();

    (IndexedPalette::from_colors(colors), cell_slots)
}

fn color_dist2(a: [u8; 3], b: [u8; 3]) -> u32 {
    let dr = i32::from(a[0]) - i32::from(b[0]);
    let dg = i32::from(a[1]) - i32::from(b[1]);
    let db = i32::from(a[2]) - i32::from(b[2]);
    (dr * dr * 2 + dg * dg * 4 + db * db) as u32
}

/// Deterministic median-cut colour quantizer producing `1..=max_colors` RGB centroids.
fn median_cut(samples: &[[u8; 3]], max_colors: usize) -> Vec<[u8; 3]> {
    if samples.is_empty() {
        return vec![[220, 220, 220]];
    }
    let mut buckets: Vec<Vec<[u8; 3]>> = vec![samples.to_vec()];
    while buckets.len() < max_colors {
        let Some((split_idx, axis)) = buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() >= 2)
            .filter_map(|(i, b)| {
                let (mut rmin, mut rmax) = (255u8, 0u8);
                let (mut gmin, mut gmax) = (255u8, 0u8);
                let (mut bmin, mut bmax) = (255u8, 0u8);
                for c in b {
                    rmin = rmin.min(c[0]);
                    rmax = rmax.max(c[0]);
                    gmin = gmin.min(c[1]);
                    gmax = gmax.max(c[1]);
                    bmin = bmin.min(c[2]);
                    bmax = bmax.max(c[2]);
                }
                let ranges = [rmax - rmin, gmax - gmin, bmax - bmin];
                let (axis, &span) = ranges.iter().enumerate().max_by_key(|(_, s)| *s)?;
                (span > 0).then_some((i, axis, u32::from(span) * (b.len() as u32).ilog2().max(1)))
            })
            .max_by_key(|&(_, _, score)| score)
            .map(|(i, axis, _)| (i, axis))
        else {
            break;
        };

        let mut bucket = buckets.swap_remove(split_idx);
        bucket.sort_by_key(|c| (c[axis], c[0], c[1], c[2]));
        let mut mid = bucket.len() / 2;
        let min_v = bucket[0][axis];
        let max_v = bucket[bucket.len() - 1][axis];
        if bucket[0][axis] == bucket[mid][axis] || bucket[mid - 1][axis] == max_v {
            let thr = ((u16::from(min_v) + u16::from(max_v)) / 2) as u8;
            mid = bucket
                .partition_point(|c| c[axis] <= thr)
                .clamp(1, bucket.len() - 1);
        }
        let right = bucket.split_off(mid);
        buckets.push(bucket);
        buckets.push(right);
    }

    buckets
        .into_iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let n = b.len() as u64;
            let (sr, sg, sb) = b.iter().fold((0u64, 0u64, 0u64), |(r, g, bl), c| {
                (
                    r + u64::from(c[0]),
                    g + u64::from(c[1]),
                    bl + u64::from(c[2]),
                )
            });
            [
                ((sr + n / 2) / n) as u8,
                ((sg + n / 2) / n) as u8,
                ((sb + n / 2) / n) as u8,
            ]
        })
        .collect()
}

/// Number of built-in procedural showcase images for the Studio's Inspire button.
pub const INSPIRE_COUNT: usize = 4;

/// Generates one of the built-in showcase images (`360 x 240` RGBA) in pure Rust.
pub fn inspire_sample(index: usize, buddy: bool) -> (&'static str, RgbaImage) {
    let (w, h) = (360u32, 240u32);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let which = index % INSPIRE_COUNT;
    let title = match which {
        0 => "Sample 1 · Crescent & peaks",
        1 => "Sample 2 · Sunset horizon",
        2 => "Sample 3 · Faceted ring",
        _ if buddy => "Sample 4 · Clawd by starlight",
        _ => "Sample 4 · Spiral nebula",
    };

    for y in 0..h {
        let ny = f64::from(y) / f64::from(h);
        for x in 0..w {
            let nx = f64::from(x) / f64::from(w);
            let [r, g, b] = match which {
                0 => sample_crescent_peaks(nx, ny),
                1 => sample_sunset(nx, ny),
                2 => sample_torus(nx, ny),
                _ if buddy => sample_clawd_card(nx, ny),
                _ => sample_nebula(nx, ny),
            };
            let o = ((y * w + x) * 4) as usize;
            px[o] = r;
            px[o + 1] = g;
            px[o + 2] = b;
            px[o + 3] = 255;
        }
    }
    (
        title,
        RgbaImage {
            width: w,
            height: h,
            pixels: px,
        },
    )
}

fn sample_crescent_peaks(nx: f64, ny: f64) -> [u8; 3] {
    // Sky gradient + crescent moon + twin alpine ridges + lake reflection.
    let water_y = 0.72;
    let (ux, uy, mir) = if ny > water_y {
        let ripple = ((ny - water_y) * 190.0).sin() * 0.008;
        (
            (nx + ripple).clamp(0.0, 1.0),
            (water_y - (ny - water_y) * 0.88).clamp(0.0, water_y),
            true,
        )
    } else {
        (nx, ny, false)
    };

    // Crescent moon at (0.66, 0.27)
    let dx = (ux - 0.66) * 1.5;
    let dy = uy - 0.27;
    let d_outer = (dx * dx + dy * dy).sqrt();
    let d_cut = ((dx - 0.065) * (dx - 0.065) + (dy + 0.025) * (dy + 0.025)).sqrt();

    let ridge_far = 0.56
        - 0.19 * (1.0 - ((ux - 0.28).abs() * 3.4).min(1.0))
        - 0.14 * (1.0 - ((ux - 0.74).abs() * 3.8).min(1.0))
        + 0.02 * (ux * 38.0).sin();
    let ridge_near =
        0.68 - 0.26 * (1.0 - ((ux - 0.42).abs() * 2.8).min(1.0)) - 0.03 * (ux * 29.0).cos();

    let mut rgb = if uy > ridge_near {
        let slope = if ux < 0.42 { 0.52 } else { 0.88 };
        let v = (52.0 + slope * 78.0 * (1.0 - (uy - ridge_near) * 1.9).clamp(0.2, 1.0)) as u8;
        [v.saturating_sub(12), v, v.saturating_add(18)]
    } else if uy > ridge_far {
        let v = (34.0 + (0.72 - uy) * 95.0) as u8;
        [v.saturating_sub(8), v, v.saturating_add(24)]
    } else if d_outer < 0.145 && d_cut > 0.125 {
        [250, 240, 206]
    } else {
        let glow = (1.0 - (d_outer / 0.28).min(1.0)).powi(3) * 26.0;
        let base = 6.0 + uy * 18.0 + glow;
        [
            base as u8,
            (base * 1.08) as u8,
            (base * 1.35).min(255.0) as u8,
        ]
    };
    if mir {
        rgb = [
            (u16::from(rgb[0]) * 6 / 10) as u8,
            (u16::from(rgb[1]) * 7 / 10) as u8,
            (u16::from(rgb[2]) * 8 / 10) as u8,
        ];
    }
    rgb
}

fn sample_sunset(nx: f64, ny: f64) -> [u8; 3] {
    let horizon = 0.62;
    let dx = (nx - 0.5) * 1.5;
    let dy = ny - 0.44;
    let d = (dx * dx + dy * dy).sqrt();
    if ny < horizon {
        if d < 0.24 {
            // Striped retro sun
            let band = ((ny - 0.34) * 55.0) as i32;
            if ny > 0.38 && band % 3 == 0 {
                return [38, 18, 52];
            }
            let t = ((ny - 0.20) / 0.30).clamp(0.0, 1.0);
            return [252, (225.0 - t * 115.0) as u8, (95.0 + t * 45.0) as u8];
        }
        let hill = horizon - 0.08 * (nx * 9.0).sin().abs() - 0.04 * (nx * 21.0 + 1.0).sin().abs();
        if ny > hill {
            return [24, 16, 36];
        }
        let t = (ny / horizon).clamp(0.0, 1.0);
        [
            (28.0 + t * 140.0) as u8,
            (20.0 + t * 62.0) as u8,
            (58.0 + t * 55.0) as u8,
        ]
    } else {
        let depth = (ny - horizon) / (1.0 - horizon);
        let sun_col = (1.0 - ((nx - 0.5).abs() * (3.2 + depth * 3.0)).min(1.0)).max(0.0);
        let wave = ((depth * 85.0 + nx * 18.0).sin() * 0.5 + 0.5) * sun_col;
        [
            (22.0 + wave * 220.0) as u8,
            (18.0 + wave * 120.0) as u8,
            (44.0 + wave * 65.0 + (1.0 - depth) * 28.0) as u8,
        ]
    }
}

fn sample_torus(nx: f64, ny: f64) -> [u8; 3] {
    let u = (nx - 0.5) * 2.4;
    let v = (ny - 0.5) * 1.65;
    // Tilted 3D ring normal approximation
    let r = (u * u + (v * 1.28) * (v * 1.28)).sqrt();
    let major = 0.62;
    let minor = 0.25;
    let dr = r - major;
    let inside = minor * minor - dr * dr;
    if inside <= 0.0 {
        let vignette = (1.0 - (r * 0.6).min(1.0)) * 22.0;
        return [
            vignette as u8,
            (vignette * 1.05) as u8,
            (vignette * 1.25) as u8,
        ];
    }
    let nz = inside.sqrt() / minor;
    let inv_r = if r > 1e-4 { 1.0 / r } else { 0.0 };
    let nx3 = (dr / minor) * (u * inv_r);
    let ny3 = (dr / minor) * (v * 1.28 * inv_r);
    let diff = (-0.52 * nx3 - 0.58 * ny3 + 0.62 * nz).max(0.06);
    let spec = diff.powi(6) * 0.55;
    let l = (diff * 0.85 + spec).clamp(0.0, 1.0);
    [
        (30.0 + l * 220.0) as u8,
        (38.0 + l * 198.0) as u8,
        (54.0 + l * 175.0) as u8,
    ]
}

fn sample_clawd_card(nx: f64, ny: f64) -> [u8; 3] {
    // Pixel-art Clawd silhouette centred over a warm campfire halo.
    let gx = (nx * 45.0).floor() as i32 - 22;
    let gy = (ny * 30.0).floor() as i32 - 15;
    // Body [-7..=7, -5..=2], arms [-9..-7] & [8..10] at [-2..=1], legs at [-6,-5], [-2,-1], [1,2], [5,6] at [3..=5]
    let in_body = (-7..=7).contains(&gx) && (-5..=2).contains(&gy);
    let in_arms = ((-9..=-8).contains(&gx) || (8..=9).contains(&gx)) && (-2..=1).contains(&gy);
    let in_legs = ([-6, -5, -2, -1, 2, 3, 5, 6].contains(&gx)) && (3..=5).contains(&gy);
    let in_eyes = ([-4, -3, 3, 4].contains(&gx)) && gy == -2;

    if in_eyes {
        return [24, 18, 16];
    }
    if in_body || in_arms {
        let top = if gy <= -4 { 24 } else { 0 };
        return [226u8.saturating_add(top), 132u8.saturating_add(top / 2), 98];
    }
    if in_legs {
        return [184, 94, 69];
    }
    if gy == 6 && (-14..=14).contains(&gx) {
        return [120, 112, 98];
    }
    let d = ((nx - 0.5) * (nx - 0.5) * 1.6 + (ny - 0.52) * (ny - 0.52)).sqrt();
    let glow = (1.0 - (d / 0.55).min(1.0)).powi(2);
    [
        (10.0 + glow * 85.0) as u8,
        (12.0 + glow * 48.0) as u8,
        (18.0 + glow * 28.0) as u8,
    ]
}

fn sample_nebula(nx: f64, ny: f64) -> [u8; 3] {
    let dx = (nx - 0.5) * 1.6;
    let dy = ny - 0.5;
    let r = (dx * dx + dy * dy).sqrt();
    let a = dy.atan2(dx);
    let arm = ((2.0 * a - r * 11.0).cos() * 0.5 + 0.5).powi(3);
    let core = (1.0 - (r / 0.55).min(1.0)).powi(2);
    let v = (core * 0.85 + arm * (1.0 - (r / 0.72).min(1.0)) * 0.75).clamp(0.0, 1.0);
    [
        (12.0 + v * 235.0) as u8,
        (14.0 + v * 195.0) as u8,
        (24.0 + v * 240.0) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;

    fn split_gradient() -> RgbaImage {
        let (w, h) = (80, 48);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = (x * 255 / (w - 1)) as u8;
                let r = v;
                let g = (y * 255 / (h - 1)) as u8;
                let b = 255 - v;
                px.extend_from_slice(&[r, g, b, 255]);
            }
        }
        RgbaImage::new(w, h, px).unwrap()
    }

    #[test]
    fn dark_and_light_polarities_invert_character_weights() {
        let src = split_gradient();
        let mut cfg = AsciiConfig {
            columns: 40,
            color_mode: AsciiColorMode::Black,
            ..AsciiConfig::default()
        };
        let on_black = convert_standalone(&src, &cfg, Theme::Dark);
        let last = (on_black.cols * on_black.rows - 1) as usize;
        assert_eq!(on_black.chars[0], ' ');
        assert_eq!(on_black.chars[last], '@');

        cfg.color_mode = AsciiColorMode::White;
        let on_white = convert_standalone(&src, &cfg, Theme::Light);
        assert_eq!(on_white.chars[0], '@');
        assert_eq!(on_white.chars[last], ' ');
    }

    #[test]
    fn braille_and_custom_ramps_produce_expected_characters() {
        let src = split_gradient();
        let braille_cfg = AsciiConfig {
            columns: 32,
            ramp: RampPreset::Braille,
            ..AsciiConfig::default()
        };
        let braille = convert_standalone(&src, &braille_cfg, Theme::Dark);
        assert!(
            braille
                .chars
                .iter()
                .any(|&c| ('\u{2801}'..='\u{28FF}').contains(&c))
        );

        let custom_cfg = AsciiConfig {
            columns: 32,
            ramp: RampPreset::Custom,
            custom_ramp: "#. ".into(),
            ..AsciiConfig::default()
        };
        let custom = convert_standalone(&src, &custom_cfg, Theme::Dark);
        assert!(custom.chars.iter().all(|c| ['#', '.', ' '].contains(c)));
    }

    #[test]
    fn none_background_exports_transparent_rgba_png_in_light_and_dark_ink() {
        let src = split_gradient();
        let cfg = AsciiConfig {
            columns: 30,
            color_mode: AsciiColorMode::None,
            ..AsciiConfig::default()
        };
        let art_dark = convert_standalone(&src, &cfg, Theme::Dark);
        let (w, h, rgba_light_ink) = render_rgba(&art_dark, &cfg, Theme::Dark, 12, true);
        assert_eq!(
            rgba_light_ink[3], 0,
            "top-left margin pixel must be transparent"
        );
        let light_ink_px = rgba_light_ink
            .as_chunks::<4>()
            .0
            .iter()
            .find(|p| p[3] > 200)
            .expect("opaque light-ink pixel");
        assert!(
            light_ink_px[0] > 200,
            "Theme::Dark None mode should render light ink"
        );

        let art_light = convert_standalone(&src, &cfg, Theme::Light);
        let (_, _, rgba_dark_ink) = render_rgba(&art_light, &cfg, Theme::Light, 12, true);
        let dark_ink_px = rgba_dark_ink
            .as_chunks::<4>()
            .0
            .iter()
            .find(|p| p[3] > 200)
            .expect("opaque dark-ink pixel");
        assert!(
            dark_ink_px[0] < 40,
            "Theme::Light None mode should render dark ink"
        );

        let png = encode_rgba_png(w, h, &rgba_light_ink).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn custom_ramp_sanitizes_unmapped_unicode_and_weather_scale_uses_scene_grid() {
        // Emoji, control chars, and duplicate characters are stripped or deduplicated,
        // falling back cleanly if nothing renderable was typed.
        let fallback = RampPreset::Custom.heavy_to_light("🚀🎉\n\t   ");
        assert_eq!(fallback.last(), Some(&' '));
        assert!(fallback.len() >= 2);
        assert!(
            fallback
                .iter()
                .take(fallback.len() - 1)
                .all(|&c| GlyphAtlas::can_render_char(c))
        );

        let deduped = RampPreset::Custom.heavy_to_light("@@@###...🚀 ");
        assert_eq!(deduped, vec!['@', '#', '.', ' ']);

        // Extreme aspect ratio (400x1) and low column count (30 cols) do not panic
        // and keep `SceneGraph.grid` at standard monitor particle resolution.
        let wide = RgbaImage::new(400, 1, vec![220; 400 * 4]).unwrap();
        let cfg = AsciiConfig {
            columns: 30,
            weather: "snow".into(),
            ..AsciiConfig::default()
        };
        assert!(cfg.has_live_weather());
        let pkg = build_photo_package(&wide, &cfg, 1920, 1080, 99).unwrap();
        let g = pkg.graph(Theme::Dark).unwrap();
        let expected = clairlune_scene::grid_spec(1920, 1080);
        assert_eq!(
            (g.grid.cell_w, g.grid.cell_h),
            (expected.cell_w, expected.cell_h)
        );
    }

    #[test]
    fn photo_packages_save_load_and_animate_weather_in_palette_and_photo_modes() {
        let (_, src) = inspire_sample(0, true);
        let dir = std::env::temp_dir().join(format!("clairlune-img2ascii-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        for (idx, mode) in [
            AsciiColorMode::Palette("nord".into()),
            AsciiColorMode::Photo,
            AsciiColorMode::Black,
        ]
        .into_iter()
        .enumerate()
        {
            let cfg = AsciiConfig {
                columns: 80,
                ramp: RampPreset::Detailed,
                color_mode: mode,
                weather: "snow".into(),
                ..AsciiConfig::default()
            };
            let pkg = build_photo_package(&src, &cfg, 640, 360, 12345).unwrap();
            let sub = dir.join(format!("mode-{idx}"));
            pkg.save(&sub).unwrap();
            let loaded = ScenePackage::load(&sub).unwrap();
            let theme = loaded.themes()[0];
            let (still, pal) = loaded.still(theme, false).unwrap();
            assert_eq!((still.width, still.height), (640, 360));
            assert!(pal.len() <= 255);

            let mut inc = loaded.incremental(theme).unwrap();
            let dirty = loaded
                .advance(&mut inc, theme, false, 1500, Clock::Smooth)
                .unwrap();
            assert!(!dirty.is_empty(), "snow weather should animate frames");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
