//! `gif`: export a seamless looping GIF of a scene.

use crate::{ThemeArg, parse_size, random_seed};
use clairlune_render::{ScenePackage, Smoothness, encode_gif, encode_package_gif};
use clairlune_scene::activity::builtin_catalog;
use clairlune_scene::{Aspect, BuildOptions, ContentContext, SceneRecipe, compose};
use clap::{Args, ValueEnum};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

#[derive(Args)]
pub struct GifArgs {
    /// Scene seed. A random seed is used (and printed) when omitted.
    #[arg(long)]
    seed: Option<u64>,
    /// Render an existing recipe JSON instead of composing from a seed.
    #[arg(long, conflicts_with = "seed")]
    recipe: Option<PathBuf>,
    /// Force an activity (e.g. `stroll`).
    #[arg(long, conflicts_with = "recipe")]
    activity: Option<String>,
    #[arg(long, value_enum, default_value_t = ThemeArg::Dark)]
    theme: ThemeArg,
    /// Output size. GIF is a sharing format: 960x540 or 1280x720 keeps files small.
    #[arg(long, default_value = "960x540", value_parser = parse_size)]
    size: (u32, u32),
    /// Loop length in seconds: a multiple of 6 (the particle period).
    #[arg(long, default_value_t = 6, value_parser = clap::value_parser!(u64).range(1..=600))]
    loop_s: u64,
    #[arg(long, value_enum, default_value_t = SmoothnessArg::Balanced)]
    smoothness: SmoothnessArg,
    /// Render without the character.
    #[arg(long)]
    no_buddy: bool,
    /// Export a saved scene package instead, at its own size (see `package`).
    #[arg(long, conflicts_with_all = ["seed", "recipe", "activity"])]
    package: Option<PathBuf>,
    /// Output GIF path.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum SmoothnessArg {
    BatterySaver,
    Balanced,
    Smooth,
    VerySmooth,
    UltraSmooth,
}

impl From<SmoothnessArg> for Smoothness {
    fn from(s: SmoothnessArg) -> Self {
        match s {
            SmoothnessArg::BatterySaver => Smoothness::BatterySaver,
            SmoothnessArg::Balanced => Smoothness::Balanced,
            SmoothnessArg::Smooth => Smoothness::Smooth,
            SmoothnessArg::VerySmooth => Smoothness::VerySmooth,
            SmoothnessArg::UltraSmooth => Smoothness::UltraSmooth,
        }
    }
}

/// The output file, its folder made first.
fn create(out: &std::path::Path) -> Result<BufWriter<File>, String> {
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    File::create(out)
        .map(BufWriter::new)
        .map_err(|e| format!("writing {}: {e}", out.display()))
}

/// An encoding error, with the half-written file taken away.
fn failed(out: &std::path::Path, e: impl std::fmt::Display) -> String {
    let _ = std::fs::remove_file(out);
    e.to_string()
}

pub fn gif(args: GifArgs) -> Result<(), String> {
    if let Some(dir) = &args.package {
        let pkg = ScenePackage::load(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let wanted = args.theme.into();
        let theme = if pkg.themes().contains(&wanted) {
            wanted
        } else {
            pkg.themes()[0]
        };
        let file = create(&args.out)?;
        let stats = encode_package_gif(
            &pkg,
            theme,
            !args.no_buddy,
            args.loop_s * 1000,
            args.smoothness.into(),
            file,
        )
        .map_err(|e| failed(&args.out, e))?;
        let bytes = std::fs::metadata(&args.out).map(|m| m.len()).unwrap_or(0);
        println!(
            "exported {} as {}x{}, {} s loop, {} frames written, {:.1} MB: {}",
            dir.display(),
            stats.width,
            stats.height,
            stats.loop_ms / 1000,
            stats.frames_written,
            bytes as f64 / 1_048_576.0,
            args.out.display()
        );
        return Ok(());
    }
    let (width, height) = args.size;
    let recipe = match &args.recipe {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            SceneRecipe::from_json(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?
        }
        None => {
            let mut ctx = ContentContext::default();
            if let Some(id) = &args.activity {
                if builtin_catalog().get(id).is_none() {
                    return Err(format!("unknown activity `{id}`"));
                }
                ctx.enabled_activities = vec![id.clone()];
            }
            compose(
                args.seed.unwrap_or_else(random_seed),
                Aspect::from_size(width, height),
                &ctx,
            )
        }
    };
    let wanted: clairlune_scene::Theme = args.theme.into();
    let opts = BuildOptions {
        width,
        height,
        // A scene that only happens in the other theme is shown in that one.
        theme: match (recipe.tod_for(wanted), wanted) {
            (Some(_), t) => t,
            (None, clairlune_scene::Theme::Light) => clairlune_scene::Theme::Dark,
            (None, clairlune_scene::Theme::Dark) => clairlune_scene::Theme::Light,
        },
        buddy: !args.no_buddy,
    };
    let file = create(&args.out)?;
    let stats = encode_gif(
        &recipe,
        &opts,
        args.loop_s * 1000,
        args.smoothness.into(),
        file,
    )
    .map_err(|e| failed(&args.out, e))?;
    let bytes = std::fs::metadata(&args.out).map(|m| m.len()).unwrap_or(0);
    println!("Clairlune GIF");
    println!(
        "  seed      {}  ({})",
        recipe.seed,
        recipe.activity.as_deref().unwrap_or("no activity")
    );
    println!(
        "  size      {}x{}, {} s loop",
        stats.width,
        stats.height,
        stats.loop_ms / 1000
    );
    println!(
        "  frames    {} rendered, {} written (identical frames merged)",
        stats.frames_rendered, stats.frames_written
    );
    println!("  time      {} ms", stats.elapsed_ms);
    println!(
        "  wrote     {} ({:.1} MB)",
        args.out.display(),
        bytes as f64 / 1_048_576.0
    );
    if bytes > 25 * 1_048_576 {
        println!("  note      over 25 MB; try a smaller --size or a lower --smoothness");
    }
    Ok(())
}
