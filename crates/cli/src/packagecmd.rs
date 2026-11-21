//! `package`: save a scene as a frozen scene package (PLAN.md section 9c), and
//! play one back as a still (`generate --package`) or a GIF (`gif --package`).

use crate::{ThemeArg, parse_size, random_seed, restrict_activity, restrict_archetype};
use clairlune_render::clock::Clock;
use clairlune_render::{ScenePackage, save_png};
use clairlune_scene::{Aspect, ContentContext, Theme, compose};
use clap::Args;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct PackageArgs {
    /// Scene seed. A random seed is used (and printed) when omitted.
    #[arg(long)]
    seed: Option<u64>,
    /// Force an activity.
    #[arg(long)]
    activity: Option<String>,
    /// Force a landscape.
    #[arg(long)]
    archetype: Option<String>,
    /// The size the scene is frozen at.
    #[arg(long, default_value = "1920x1080", value_parser = parse_size)]
    size: (u32, u32),
    /// Output folder.
    #[arg(long)]
    out: PathBuf,
}

fn folder_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

pub fn package(args: PackageArgs) -> Result<(), String> {
    let (w, h) = args.size;
    let mut ctx = ContentContext::default();
    if let Some(id) = &args.archetype {
        restrict_archetype(&mut ctx, id)?;
    }
    if let Some(id) = &args.activity {
        restrict_activity(&mut ctx, id)?;
    }
    let seed = args.seed.unwrap_or_else(random_seed);
    let recipe = compose(seed, Aspect::from_size(w, h), &ctx);
    let pkg = ScenePackage::capture(&recipe, w, h).map_err(|e| e.to_string())?;
    if args.out.is_file() {
        return Err(format!(
            "{} is a file; a package is a folder",
            args.out.display()
        ));
    }
    pkg.save(&args.out)
        .map_err(|e| format!("writing {}: {e}", args.out.display()))?;
    let themes: Vec<&str> = pkg.themes().iter().map(|t| t.as_str()).collect();
    println!("Clairlune scene package");
    println!(
        "  scene     seed {seed}, {} in {}",
        recipe.activity.as_deref().unwrap_or("no activity"),
        recipe.archetype
    );
    println!("  size      {w}x{h}, themes {}", themes.join(" and "));
    println!(
        "  wrote     {} ({:.0} KB)",
        args.out.display(),
        folder_size(&args.out) as f64 / 1024.0
    );
    Ok(())
}

/// `generate --package`: a still from a saved scene.
pub fn still_from_package(
    dir: &Path,
    theme: ThemeArg,
    buddy: bool,
    time_ms: Option<u64>,
    out: &Path,
) -> Result<(), String> {
    let pkg = ScenePackage::load(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let wanted: Theme = theme.into();
    let theme = if pkg.themes().contains(&wanted) {
        wanted
    } else {
        pkg.themes()[0]
    };
    let t = time_ms.unwrap_or_else(|| pkg.hero_ms(theme).unwrap_or(0));
    let (img, palette) = pkg
        .frame(theme, buddy, t, Clock::Live)
        .ok_or("the package has no frames")?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    save_png(out, &img, palette).map_err(|e| format!("writing {}: {e}", out.display()))?;
    println!(
        "played {} ({}x{}, {}, t = {t} ms, generator {}) to {}",
        dir.display(),
        pkg.width,
        pkg.height,
        theme.as_str(),
        pkg.generator_version,
        out.display()
    );
    Ok(())
}
