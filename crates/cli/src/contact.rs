//! `contact-sheet`: many seeds as small tiles in one image, for checking variety
//! (activities, landscapes, palettes, character art) at a glance.

use crate::{ThemeArg, parse_size};
use clairlune_render::{render_still, to_rgba};
use clairlune_scene::activity::builtin_catalog;
use clairlune_scene::{Aspect, BuildError, BuildOptions, ContentContext, Theme, compose};
use clap::Args;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

#[derive(Args)]
pub struct ContactArgs {
    /// Number of seeds.
    #[arg(long, default_value_t = 12)]
    count: u32,
    /// First seed; the sheet shows seeds from..from+count.
    #[arg(long, default_value_t = 1)]
    from: u64,
    /// Tiles per row.
    #[arg(long, default_value_t = 4)]
    columns: u32,
    /// Size of each tile.
    #[arg(long, default_value = "480x270", value_parser = parse_size)]
    tile: (u32, u32),
    /// Theme. Night-only scenes are shown in dark either way.
    #[arg(long, value_enum, default_value_t = ThemeArg::Dark)]
    theme: ThemeArg,
    /// Only this activity.
    #[arg(long)]
    activity: Option<String>,
    /// Only this landscape.
    #[arg(long)]
    archetype: Option<String>,
    /// Render without the character.
    #[arg(long)]
    no_buddy: bool,
    /// Output PNG.
    #[arg(long)]
    out: PathBuf,
}

const GAP: u32 = 6;
const GAP_RGB: [u8; 3] = [0x80, 0x80, 0x80];

pub fn contact_sheet(args: ContactArgs) -> Result<(), String> {
    let (tw, th) = args.tile;
    let cols = args.columns.clamp(1, args.count.max(1));
    let rows = args.count.div_ceil(cols);
    let (w, h) = (cols * tw + (cols + 1) * GAP, rows * th + (rows + 1) * GAP);
    let mut rgb = vec![0u8; (w * h * 3) as usize];
    for px in rgb.as_chunks_mut::<3>().0 {
        *px = GAP_RGB;
    }

    let mut ctx = ContentContext::default();
    if let Some(id) = &args.archetype {
        crate::restrict_archetype(&mut ctx, id)?;
    }
    if let Some(id) = &args.activity {
        if builtin_catalog().get(id).is_none() {
            return Err(format!("unknown activity `{id}`"));
        }
        ctx.enabled_activities = vec![id.clone()];
    }
    let wanted: Theme = args.theme.into();
    for i in 0..args.count {
        let seed = args.from + u64::from(i);
        let recipe = compose(seed, Aspect::from_size(tw, th), &ctx);
        let mut opts = BuildOptions {
            width: tw,
            height: th,
            theme: wanted,
            buddy: !args.no_buddy,
        };
        let still = match render_still(&recipe, &opts, None) {
            Err(BuildError::ThemeLocked(_)) => {
                opts.theme = if wanted == Theme::Light {
                    Theme::Dark
                } else {
                    Theme::Light
                };
                render_still(&recipe, &opts, None)
            }
            other => other,
        }
        .map_err(|e| format!("seed {seed}: {e}"))?;
        let tile = to_rgba(&still.image, &still.palette);
        let (x0, y0) = (GAP + (i % cols) * (tw + GAP), GAP + (i / cols) * (th + GAP));
        for y in 0..th {
            for x in 0..tw {
                let s = ((y * tw + x) * 4) as usize;
                let d = (((y0 + y) * w + x0 + x) * 3) as usize;
                rgb[d..d + 3].copy_from_slice(&tile[s..s + 3]);
            }
        }
        println!(
            "  seed {seed:>6}  {:<13} {:<14} {}",
            recipe.activity.as_deref().unwrap_or("-"),
            recipe.archetype,
            opts.theme.as_str()
        );
    }

    if let Some(parent) = args.out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = BufWriter::new(File::create(&args.out).map_err(|e| e.to_string())?);
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&rgb).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    println!(
        "wrote {} ({w}x{h}, {} tiles)",
        args.out.display(),
        args.count
    );
    Ok(())
}
