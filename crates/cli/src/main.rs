//! `clairlune-cli`: developer tool for rendering scenes without the desktop app.

mod contact;

use clairlune_render::{render_still, save_png};
use clairlune_scene::activity::builtin_catalog;
use clairlune_scene::{
    APP_NAME, Aspect, BuildError, BuildOptions, ContentContext, SceneRecipe, Theme, compose,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

#[derive(Parser)]
#[command(
    name = "clairlune-cli",
    version,
    about = "Render Clairlune ASCII-art wallpapers from a seed"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compose a scene from a seed (or load a recipe) and render a still PNG.
    Generate(GenerateArgs),
    /// Render many seeds as small tiles in one image, to check variety at a glance.
    ContactSheet(contact::ContactArgs),
}

#[derive(Args)]
struct GenerateArgs {
    /// Scene seed. A random seed is used (and printed) when omitted.
    #[arg(long)]
    seed: Option<u64>,
    /// Render an existing recipe JSON instead of composing from a seed.
    #[arg(long, conflicts_with = "seed")]
    recipe: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = ThemeArg::Dark)]
    theme: ThemeArg,
    /// Output size, e.g. 3840x2160.
    #[arg(long, default_value = "1920x1080", value_parser = parse_size)]
    size: (u32, u32),
    /// Output PNG path.
    #[arg(long)]
    out: PathBuf,
    /// Force an activity (e.g. `stroll`). Lists the ids when unknown.
    #[arg(long, conflicts_with = "recipe")]
    activity: Option<String>,
    /// Render without the character (the unattended variant).
    #[arg(long)]
    no_buddy: bool,
    /// Also write the scene recipe as JSON.
    #[arg(long)]
    recipe_out: Option<PathBuf>,
    /// Animation time in milliseconds. Defaults to the scene's hero time.
    #[arg(long)]
    time_ms: Option<u64>,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ThemeArg {
    Light,
    Dark,
}

impl From<ThemeArg> for Theme {
    fn from(t: ThemeArg) -> Self {
        match t {
            ThemeArg::Light => Theme::Light,
            ThemeArg::Dark => Theme::Dark,
        }
    }
}

pub(crate) fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("expected WIDTHxHEIGHT, got `{s}`"))?;
    let w: u32 = w
        .trim()
        .parse()
        .map_err(|_| format!("bad width in `{s}`"))?;
    let h: u32 = h
        .trim()
        .parse()
        .map_err(|_| format!("bad height in `{s}`"))?;
    if w == 0 || h == 0 {
        return Err("width and height must be positive".into());
    }
    Ok((w, h))
}

/// A seed from OS entropy: `RandomState` keys come from the OS random source.
fn random_seed() -> u64 {
    RandomState::new().hash_one(0x5EED_u64)
}

fn generate(args: GenerateArgs) -> Result<(), String> {
    let (width, height) = args.size;
    let started = Instant::now();
    let recipe = match &args.recipe {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("reading {}: {e}", path.display()))?;
            SceneRecipe::from_json(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?
        }
        None => {
            let seed = args.seed.unwrap_or_else(random_seed);
            let mut ctx = ContentContext::default();
            if let Some(id) = &args.activity {
                let catalog = builtin_catalog();
                if catalog.get(id).is_none() {
                    let ids: Vec<&str> = catalog.activities.iter().map(|a| a.id.as_str()).collect();
                    return Err(format!(
                        "unknown activity `{id}`; available: {}",
                        ids.join(", ")
                    ));
                }
                ctx.enabled_activities = vec![id.clone()];
            }
            compose(seed, Aspect::from_size(width, height), &ctx)
        }
    };
    let opts = BuildOptions {
        width,
        height,
        theme: args.theme.into(),
        buddy: !args.no_buddy,
    };
    let still = render_still(&recipe, &opts, args.time_ms).map_err(|e| match e {
        BuildError::ThemeLocked(t) => format!(
            "seed {} ({}) has no {} variant: the activity only happens at {}. Use the other theme or another seed.",
            recipe.seed,
            recipe.activity.as_deref().unwrap_or("no activity"),
            t.as_str(),
            if t == Theme::Light { "night" } else { "daytime" },
        ),
        other => other.to_string(),
    })?;
    let rendered = started.elapsed();

    if let Some(parent) = args.out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    save_png(&args.out, &still.image, &still.palette)
        .map_err(|e| format!("writing {}: {e}", args.out.display()))?;
    if let Some(path) = &args.recipe_out {
        std::fs::write(path, recipe.to_json())
            .map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    let total = started.elapsed();
    let bytes = std::fs::metadata(&args.out).map(|m| m.len()).unwrap_or(0);

    println!("{APP_NAME} scene");
    println!("  seed      {}", recipe.seed);
    println!(
        "  activity  {}",
        recipe
            .activity
            .as_deref()
            .unwrap_or("(none: archetype-first fallback)")
    );
    println!(
        "  archetype {}  ({} {:?})",
        recipe.archetype,
        opts.theme.as_str(),
        still.graph.tod
    );
    println!(
        "  size      {width}x{height}  grid {}x{} cells of {}x{} px",
        still.graph.grid.cols,
        still.graph.grid.rows,
        still.graph.grid.cell_w,
        still.graph.grid.cell_h
    );
    println!(
        "  frame     t = {} ms (hero {} ms)",
        still.t_ms,
        clairlune_render::hero_time(&still.graph)
    );
    println!("  palette   {} colours", still.palette.len());
    println!(
        "  time      render {:.0} ms, total {:.0} ms",
        rendered.as_secs_f64() * 1e3,
        total.as_secs_f64() * 1e3
    );
    println!(
        "  wrote     {} ({:.1} KB)",
        args.out.display(),
        bytes as f64 / 1024.0
    );
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Generate(args) => generate(args),
        Command::ContactSheet(args) => contact::contact_sheet(args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
