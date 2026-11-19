//! `monitors` and `set`: list the monitors, and render one scene per monitor
//! at its physical size and set it as the desktop wallpaper (M5).

use crate::{ThemeArg, random_seed, restrict_activity, restrict_archetype};
use clairlune_platform::{Monitor, StillStore, detect, init_dpi_awareness};
use clairlune_render::{encode_png, render_still};
use clairlune_scene::hash::hash_label;
use clairlune_scene::{Aspect, BuildError, BuildOptions, ContentContext, Theme, compose};
use clap::Args;
use std::time::Instant;

#[derive(Args)]
pub struct SetArgs {
    /// Base seed: the primary monitor's scene. Other monitors derive theirs
    /// from it and their id. Random (and printed) when omitted.
    #[arg(long)]
    seed: Option<u64>,
    /// Theme. Scenes that only exist in the other theme use that one.
    #[arg(long, value_enum, default_value_t = ThemeArg::Dark)]
    theme: ThemeArg,
    /// Force an activity on every monitor.
    #[arg(long)]
    activity: Option<String>,
    /// Force a landscape on every monitor.
    #[arg(long)]
    archetype: Option<String>,
    /// Without the character.
    #[arg(long)]
    no_buddy: bool,
    /// Only this monitor (an id printed by `monitors`).
    #[arg(long)]
    monitor: Option<String>,
    /// Render and store the stills, but leave the wallpaper as it is.
    #[arg(long)]
    dry_run: bool,
}

/// Prints the monitors, their ids and current wallpapers.
pub fn monitors() -> Result<(), String> {
    init_dpi_awareness();
    let host = detect().map_err(|e| e.to_string())?;
    let list = host.monitors().map_err(|e| e.to_string())?;
    let caps = host.capabilities();
    println!(
        "{} ({} monitor{}, per-monitor images: {}, live: {})",
        host.name(),
        list.len(),
        if list.len() == 1 { "" } else { "s" },
        if caps.per_monitor { "yes" } else { "no" },
        if caps.live { "yes" } else { "not yet" },
    );
    for m in &list {
        let current = match host.current(m) {
            Ok(Some(p)) => p.display().to_string(),
            Ok(None) => "(no image)".to_string(),
            Err(e) => format!("(unknown: {e})"),
        };
        println!(
            "{} {}x{} at ({}, {})",
            if m.primary { "*" } else { " " },
            m.width,
            m.height,
            m.x,
            m.y
        );
        println!("    id       {}", m.id);
        println!("    current  {current}");
    }
    Ok(())
}

/// A monitor's seed: the base seed on the primary monitor, derived from the
/// base seed and the monitor id elsewhere (PLAN.md section 2.1).
pub fn monitor_seed(base: u64, m: &Monitor) -> u64 {
    if m.primary {
        base
    } else {
        hash_label(base, &m.id)
    }
}

pub fn set(args: SetArgs) -> Result<(), String> {
    init_dpi_awareness();
    let host = detect().map_err(|e| e.to_string())?;
    let mut monitors = host.monitors().map_err(|e| e.to_string())?;
    if let Some(id) = &args.monitor {
        monitors.retain(|m| &m.id == id);
        if monitors.is_empty() {
            return Err(format!(
                "no connected monitor `{id}`; run `monitors` for the ids"
            ));
        }
    }
    if monitors.is_empty() {
        return Err("no monitors found".to_string());
    }
    let store = StillStore::default_location().map_err(|e| format!("still store: {e}"))?;
    let mut ctx = ContentContext::default();
    if let Some(id) = &args.archetype {
        restrict_archetype(&mut ctx, id)?;
    }
    if let Some(id) = &args.activity {
        restrict_activity(&mut ctx, id)?;
    }
    if !args.dry_run {
        // The first takeover remembers the user's own wallpaper for `restore`.
        let snapshot = host.snapshot().map_err(|e| e.to_string())?;
        if store
            .remember_original(&snapshot)
            .map_err(|e| format!("saving the current wallpaper: {e}"))?
        {
            println!("saved your current wallpaper; `restore` puts it back");
        }
    }
    let base = args.seed.unwrap_or_else(random_seed);
    let wanted: Theme = args.theme.into();
    println!("base seed {base}");
    for m in &monitors {
        let started = Instant::now();
        let seed = monitor_seed(base, m);
        let recipe = compose(seed, Aspect::from_size(m.width, m.height), &ctx);
        let opts = |theme| BuildOptions {
            width: m.width,
            height: m.height,
            theme,
            buddy: !args.no_buddy,
        };
        let mut theme = wanted;
        let still = match render_still(&recipe, &opts(theme), None) {
            Err(BuildError::ThemeLocked(_)) => {
                theme = if wanted == Theme::Light {
                    Theme::Dark
                } else {
                    Theme::Light
                };
                render_still(&recipe, &opts(theme), None)
            }
            other => other,
        }
        .map_err(|e| format!("seed {seed}: {e}"))?;
        let mut png = Vec::new();
        encode_png(&still.image, &still.palette, &mut png).map_err(|e| e.to_string())?;
        let path = store
            .write(&m.id, &png)
            .map_err(|e| format!("writing the still: {e}"))?;
        let mut note = String::from("stored (dry run)");
        if !args.dry_run {
            host.set_static(m, &path).map_err(|e| e.to_string())?;
            let removed = store.prune(&m.id, &path);
            note = format!("set, {removed} old still(s) removed");
        }
        tracing::info!(
            target: "clairlune_cli",
            monitor = %m.id,
            seed,
            recipe_hash = %recipe.hash_hex(),
            width = m.width,
            height = m.height,
            theme = theme.as_str(),
            path = %path.display(),
            dry_run = args.dry_run,
            "wallpaper still"
        );
        println!(
            "{} {}x{}  seed {seed}  {} in {}  ({})  {:.0} ms",
            if m.primary { "*" } else { " " },
            m.width,
            m.height,
            recipe.activity.as_deref().unwrap_or("-"),
            recipe.archetype,
            theme.as_str(),
            started.elapsed().as_secs_f64() * 1e3
        );
        println!("    {}  [{note}]", path.display());
    }
    Ok(())
}

/// Puts back the wallpaper that was there before Clairlune first set one, and
/// removes Clairlune's stills.
pub fn restore() -> Result<(), String> {
    init_dpi_awareness();
    let host = detect().map_err(|e| e.to_string())?;
    let store = StillStore::default_location().map_err(|e| format!("still store: {e}"))?;
    let Some(snapshot) = store.original().map_err(|e| e.to_string())? else {
        return Err("nothing to restore: Clairlune has not set a wallpaper here".to_string());
    };
    host.restore(&snapshot).map_err(|e| e.to_string())?;
    store.forget_original().map_err(|e| e.to_string())?;
    let removed = store.clear_stills();
    for (id, path) in &snapshot.monitors {
        let what = path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(no image)".to_string());
        println!("restored {what}\n    on {id}");
    }
    println!("removed {removed} Clairlune still(s)");
    Ok(())
}
