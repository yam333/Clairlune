//! `clairlune-cli`: developer tool for rendering scenes without the desktop app.

mod contact;
mod gifcmd;
mod livecmd;
mod packagecmd;
mod preview;
mod setcmd;

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
    /// More log output on stderr: -v info, -vv debug, -vvv trace. CLAIRLUNE_LOG overrides.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
    /// Also write logs as JSON lines to this file (debug level unless CLAIRLUNE_LOG is set).
    #[arg(long, global = true)]
    log_file: Option<PathBuf>,
}

/// What the CLI is working on, for crash reports.
static SCENE_CONTEXT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn set_scene_context(s: String) {
    if let Ok(mut g) = SCENE_CONTEXT.lock() {
        *g = s;
    }
}

/// stderr (human-readable) plus an optional JSON-lines file (docs/logging.md).
fn init_logging(verbose: u8, log_file: Option<&std::path::Path>) -> Result<(), String> {
    use tracing_subscriber::{EnvFilter, Layer, fmt, prelude::*};
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = |default: &str| {
        EnvFilter::try_from_env("CLAIRLUNE_LOG").unwrap_or_else(|_| EnvFilter::new(default))
    };
    let stderr = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_filter(filter(level));
    let file = match log_file {
        Some(p) => {
            let f =
                std::fs::File::create(p).map_err(|e| format!("log file {}: {e}", p.display()))?;
            let file_level = if verbose >= 3 { "trace" } else { "debug" };
            Some(
                fmt::layer()
                    .json()
                    .with_writer(std::sync::Mutex::new(f))
                    .with_filter(filter(file_level)),
            )
        }
        None => None,
    };
    tracing_subscriber::registry()
        .with(stderr)
        .with(file)
        .init();
    Ok(())
}

/// Logs panics with their location and the scene being made, then points at --log-file.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let scene = SCENE_CONTEXT.lock().map(|g| g.clone()).unwrap_or_default();
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "clairlune_cli", %location, scene = %scene, panic = %info, %backtrace, "panic");
        eprintln!(
            "clairlune-cli crashed at {location} while working on: {scene}. Re-run with --log-file <path> -vv for details."
        );
    }));
}

#[derive(Subcommand)]
enum Command {
    /// Compose a scene from a seed (or load a recipe) and render a still PNG.
    Generate(GenerateArgs),
    /// Render many seeds as small tiles in one image, to check variety at a glance.
    ContactSheet(contact::ContactArgs),
    /// Export a seamless looping GIF (a sharing format; saved wallpapers use scene packages).
    Gif(gifcmd::GifArgs),
    /// List the monitors: ids, physical sizes and current wallpapers.
    Monitors,
    /// Render a scene per monitor at its physical size and set it as the wallpaper.
    Set(setcmd::SetArgs),
    /// Put back the wallpaper you had before Clairlune first set one.
    Restore,
    /// Freeze a scene into a package that plays without the generator.
    Package(packagecmd::PackageArgs),
    /// Play a scene behind the desktop icons for a while (Windows).
    Live(livecmd::LiveArgs),
    /// Open a window with an activity in the scenes it fits; reloads the
    /// activity files and sprite sheets when they change (art authoring).
    PreviewActivity(preview::PreviewArgs),
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
    /// Force a landscape (e.g. `mountains`). Lists the ids when unknown.
    #[arg(long, conflicts_with = "recipe")]
    archetype: Option<String>,
    /// Force the weather (e.g. `birds`), where the landscape and activity
    /// allow it. Lists the ids when unknown.
    #[arg(long, conflicts_with = "recipe")]
    weather: Option<String>,
    /// Force a look: `line` (line art) or `dither` (light and dark masses).
    #[arg(long, conflicts_with = "recipe")]
    look: Option<String>,
    /// Force a zoom: `close` (Clawd as in 0.13), `wide` or `vast`.
    #[arg(long, conflicts_with = "recipe")]
    zoom: Option<String>,
    /// Ink contrast: `soft`, `normal`, `high`, or `vary` (one per seed).
    #[arg(long, conflicts_with = "recipe")]
    contrast: Option<String>,
    /// Render without the character (the unattended variant).
    #[arg(long)]
    no_buddy: bool,
    /// Also write the scene recipe as JSON.
    #[arg(long)]
    recipe_out: Option<PathBuf>,
    /// Animation time in milliseconds. Defaults to the scene's hero time.
    #[arg(long, value_parser = clap::value_parser!(u64).range(..=MAX_TIME_MS))]
    time_ms: Option<u64>,
    /// Play a saved scene package instead (see `package`); no generator involved.
    #[arg(long, conflicts_with_all = ["seed", "recipe", "activity", "archetype", "look", "contrast", "recipe_out"])]
    package: Option<PathBuf>,
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
    if w > MAX_SIDE || h > MAX_SIDE || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(format!(
            "`{s}` is too big: at most {MAX_SIDE} a side and {MAX_PIXELS} pixels (8K is 7680x4320)"
        ));
    }
    Ok((w, h))
}

/// The latest animation time accepted (about 285,000 years).
pub(crate) const MAX_TIME_MS: u64 = 1 << 53;

/// The largest side and area a scene is made at: past 8K both ways, short
/// of sizes that would take hours or all the memory.
const MAX_SIDE: u32 = 16_384;
const MAX_PIXELS: u64 = 16_384 * 8_640;

/// Limits composition to one landscape; errors list the ones that exist.
pub(crate) fn restrict_archetype(ctx: &mut ContentContext, id: &str) -> Result<(), String> {
    let ok = clairlune_scene::archetype::archetype(id).is_some_and(|a| a.implemented);
    if !ok {
        let ids: Vec<&str> = clairlune_scene::archetype::ARCHETYPE_INFO
            .iter()
            .filter(|a| a.implemented)
            .map(|a| a.id)
            .collect();
        return Err(format!(
            "unknown or unbuilt landscape `{id}`; available: {}",
            ids.join(", ")
        ));
    }
    ctx.enabled_archetypes = vec![id.to_string()];
    Ok(())
}

/// Limits composition to one weather; errors list the ones that exist.
pub(crate) fn restrict_weather(ctx: &mut ContentContext, id: &str) -> Result<(), String> {
    use clairlune_scene::archetype::WEATHERS;
    if !WEATHERS.contains(&id) {
        return Err(format!(
            "unknown weather `{id}`; available: {}",
            WEATHERS.join(", ")
        ));
    }
    ctx.enabled_weather = vec![id.to_string()];
    Ok(())
}

/// Limits composition to one activity; errors list the ones that exist.
pub(crate) fn restrict_activity(ctx: &mut ContentContext, id: &str) -> Result<(), String> {
    let catalog = builtin_catalog();
    if catalog.get(id).is_none() {
        let ids: Vec<&str> = catalog.activities.iter().map(|a| a.id.as_str()).collect();
        return Err(format!(
            "unknown activity `{id}`; available: {}",
            ids.join(", ")
        ));
    }
    ctx.enabled_activities = vec![id.to_string()];
    Ok(())
}

/// A seed from OS entropy: `RandomState` keys come from the OS random source.
/// Forces a look and a contrast (either may be left to the defaults).
/// Limits composition to one zoom.
pub(crate) fn restrict_zoom(ctx: &mut ContentContext, zoom: Option<&str>) -> Result<(), String> {
    use clairlune_scene::ZOOMS;
    if let Some(z) = zoom {
        if !ZOOMS.contains(&z) {
            return Err(format!("unknown zoom `{z}`; zooms: {}", ZOOMS.join(", ")));
        }
        ctx.enabled_zooms = vec![z.to_string()];
    }
    Ok(())
}

pub(crate) fn restrict_look(
    ctx: &mut ContentContext,
    look: Option<&str>,
    contrast: Option<&str>,
) -> Result<(), String> {
    use clairlune_scene::recipe::{CONTRASTS, LOOKS};
    if let Some(l) = look {
        if !LOOKS.contains(&l) {
            return Err(format!("unknown look `{l}`; looks: {}", LOOKS.join(", ")));
        }
        ctx.enabled_looks = vec![l.to_string()];
    }
    if let Some(c) = contrast {
        if !CONTRASTS.contains(&c) && c != "vary" {
            return Err(format!(
                "unknown contrast `{c}`; contrasts: {}, vary",
                CONTRASTS.join(", ")
            ));
        }
        ctx.contrast = c.to_string();
    }
    Ok(())
}

pub(crate) fn random_seed() -> u64 {
    RandomState::new().hash_one(0x5EED_u64)
}

fn generate(args: GenerateArgs) -> Result<(), String> {
    if let Some(dir) = &args.package {
        return packagecmd::still_from_package(
            dir,
            args.theme,
            !args.no_buddy,
            args.time_ms,
            &args.out,
        );
    }
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
            if let Some(id) = &args.archetype {
                restrict_archetype(&mut ctx, id)?;
            }
            if let Some(id) = &args.activity {
                restrict_activity(&mut ctx, id)?;
            }
            if let Some(id) = &args.weather {
                restrict_weather(&mut ctx, id)?;
            }
            restrict_look(&mut ctx, args.look.as_deref(), args.contrast.as_deref())?;
            restrict_zoom(&mut ctx, args.zoom.as_deref())?;
            compose(seed, Aspect::from_size(width, height), &ctx)
        }
    };
    set_scene_context(format!(
        "seed {} activity {} recipe {} {}x{}",
        recipe.seed,
        recipe.activity.as_deref().unwrap_or("-"),
        recipe.hash_hex(),
        width,
        height
    ));
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
    if let Err(e) = init_logging(cli.verbose, cli.log_file.as_deref()) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    install_panic_hook();
    tracing::info!(
        target: "clairlune_cli",
        version = env!("CARGO_PKG_VERSION"),
        generator_version = clairlune_scene::GENERATOR_VERSION,
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "clairlune-cli starting"
    );
    let result = match cli.command {
        Command::Generate(args) => generate(args),
        Command::ContactSheet(args) => contact::contact_sheet(args),
        Command::Gif(args) => gifcmd::gif(args),
        Command::Monitors => setcmd::monitors(),
        Command::Set(args) => setcmd::set(args),
        Command::Restore => setcmd::restore(),
        Command::Package(args) => packagecmd::package(args),
        Command::Live(args) => livecmd::live(args),
        Command::PreviewActivity(args) => preview::preview(args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
