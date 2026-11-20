//! Applying wallpapers, on a worker thread: compose one scene per monitor (or
//! re-render saved recipes), render at each monitor's physical size, archive
//! the set, then set each still through the platform host. The static image is
//! always set first and kept current, so a crash never leaves a blank desktop.

use crate::archive::{Archive, Entry, EntryRecipes, MonitorScene, NewEntry};
use crate::settings::{Settings, ThemeMode};
use clairlune_platform::{Monitor, StillStore, WallpaperHost};
use clairlune_render::{Still, encode_png, render_still};
use clairlune_scene::hash::hash_label;
use clairlune_scene::{Aspect, BuildError, BuildOptions, SceneRecipe, Theme, compose};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

/// Work for the worker thread.
pub enum Job {
    /// A new scene per monitor from a base seed.
    New { seed: u64, settings: Box<Settings> },
    /// Saved recipes again (previous wallpaper, theme change, resolution
    /// change): same scenes, rendered for the current monitors and theme.
    Recipes {
        recipes: Box<EntryRecipes>,
        settings: Box<Settings>,
        /// Archive it as a new entry (previous wallpaper) or only re-render
        /// the current one (theme change).
        archive: bool,
    },
    /// Copy the current wallpaper's primary image to the export folder.
    Export { dest: PathBuf },
    /// Put back the wallpaper the user had before Clairlune.
    Restore,
}

/// What happened, for the tray tooltip and logs.
#[derive(Clone, Debug)]
pub enum Done {
    Applied {
        entry_name: String,
        seed: u64,
        theme: Theme,
        archetype: String,
        activity: Option<String>,
        monitors: usize,
        ms: u64,
    },
    Exported(PathBuf),
    /// This system has no wallpaper host yet: scenes are made and archived,
    /// but not set. The reason is for the user.
    NoHost(String),
    /// The user's own wallpaper is back.
    Restored,
    Failed(String),
}

/// The theme a setting resolves to right now.
pub fn resolve_theme(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::Light => Theme::Light,
        ThemeMode::Dark => Theme::Dark,
        ThemeMode::FollowSystem => match clairlune_platform::system_prefers_dark() {
            Some(false) => Theme::Light,
            _ => Theme::Dark,
        },
    }
}

/// A monitor's seed: the base seed on the primary, derived from the base seed
/// and the monitor id elsewhere (PLAN.md section 2.1).
pub fn monitor_seed(base: u64, m: &Monitor) -> u64 {
    if m.primary {
        base
    } else {
        hash_label(base, &m.id)
    }
}

/// Renders a recipe in the wanted theme, or in the theme it is locked to.
fn render(
    recipe: &SceneRecipe,
    w: u32,
    h: u32,
    theme: Theme,
    buddy: bool,
) -> Result<(Still, Theme), String> {
    let opts = |theme| BuildOptions {
        width: w,
        height: h,
        theme,
        buddy,
    };
    match render_still(recipe, &opts(theme), None) {
        Ok(s) => Ok((s, theme)),
        Err(BuildError::ThemeLocked(_)) => {
            let other = if theme == Theme::Light {
                Theme::Dark
            } else {
                Theme::Light
            };
            render_still(recipe, &opts(other), None)
                .map(|s| (s, other))
                .map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn png_of(s: &Still) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    encode_png(&s.image, &s.palette, &mut png).map_err(|e| e.to_string())?;
    Ok(png)
}

/// The scenes for this run: new ones from a seed, or saved recipes matched to
/// the connected monitors (by id; unmatched monitors derive from the first).
fn scenes(
    job_recipes: Option<&EntryRecipes>,
    seed: u64,
    monitors: &[Monitor],
    settings: &Settings,
) -> (u64, Vec<SceneRecipe>) {
    let ctx = settings.content_context();
    match job_recipes {
        None => (
            seed,
            monitors
                .iter()
                .map(|m| {
                    compose(
                        monitor_seed(seed, m),
                        Aspect::from_size(m.width, m.height),
                        &ctx,
                    )
                })
                .collect(),
        ),
        Some(r) => (
            r.base_seed,
            monitors
                .iter()
                .map(|m| match r.monitors.iter().find(|s| s.monitor == m.id) {
                    Some(s) => s.recipe.clone(),
                    None => compose(
                        monitor_seed(r.base_seed, m),
                        Aspect::from_size(m.width, m.height),
                        &ctx,
                    ),
                })
                .collect(),
        ),
    }
}

/// Runs jobs until the sender goes away, reporting each result through `done`.
pub fn worker(
    jobs: mpsc::Receiver<Job>,
    done: impl Fn(Done) + Send + 'static,
    archive_dir: PathBuf,
) {
    // COM and the wallpaper host belong to this thread.
    let host = match clairlune_platform::detect() {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!(target: "clairlune", error = %e, "no wallpaper host; scenes are archived but not set");
            done(Done::NoHost(format!(
                "Clairlune can't set the wallpaper on this system yet; scenes are saved in the archive. ({e})"
            )));
            None
        }
    };
    let store = StillStore::default_location();
    let archive = Archive::open(&archive_dir);
    // Before the first change, remember the user's own wallpaper so it can be
    // put back (`clairlune-cli restore`, later a button in Settings).
    if let (Some(h), Ok(s)) = (&host, &store) {
        match h.snapshot().map(|snap| s.remember_if_users(&snap)) {
            Ok(Ok(true)) => {
                tracing::info!(target: "clairlune", "remembered the current wallpaper for restore")
            }
            Ok(Ok(false)) => {}
            Ok(Err(e)) => {
                tracing::warn!(target: "clairlune", error = %e, "could not save the current wallpaper")
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "could not read the current wallpaper")
            }
        }
    }
    for job in jobs {
        let result = match (&archive, &store) {
            (Ok(archive), Ok(store)) => run(job, host.as_deref(), store, archive),
            (Err(e), _) | (_, Err(e)) => Done::Failed(format!("data folder: {e}")),
        };
        if let Done::Failed(e) = &result {
            tracing::error!(target: "clairlune", error = %e, "job failed");
        }
        done(result);
    }
}

fn run(job: Job, host: Option<&dyn WallpaperHost>, store: &StillStore, archive: &Archive) -> Done {
    match job {
        Job::Export { dest } => export(archive, &dest),
        Job::Restore => restore(host, store),
        Job::New { seed, settings } => apply(None, seed, &settings, true, host, store, archive),
        Job::Recipes {
            recipes,
            settings,
            archive: add,
        } => apply(Some(&recipes), 0, &settings, add, host, store, archive),
    }
    .unwrap_or_else(Done::Failed)
}

fn apply(
    recipes: Option<&EntryRecipes>,
    seed: u64,
    settings: &Settings,
    add_to_archive: bool,
    host: Option<&dyn WallpaperHost>,
    store: &StillStore,
    archive: &Archive,
) -> Result<Done, String> {
    let started = Instant::now();
    let monitors = match host {
        Some(h) => h.monitors().map_err(|e| e.to_string())?,
        None => vec![Monitor {
            id: "default".into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            primary: true,
        }],
    };
    if monitors.is_empty() {
        return Err("no monitors found".into());
    }
    let wanted = resolve_theme(settings.theme.mode);
    let (base_seed, recipes) = scenes(recipes, seed, &monitors, settings);
    let mut pngs = Vec::new();
    let mut used_theme = wanted;
    for (i, (m, r)) in monitors.iter().zip(&recipes).enumerate() {
        let (still, theme) = render(r, m.width, m.height, wanted, settings.buddy.show)?;
        if i == 0 {
            used_theme = theme;
        }
        pngs.push(png_of(&still)?);
    }
    // The thumbnail: the primary scene, 480 px wide.
    let (pw, ph) = (monitors[0].width, monitors[0].height);
    let th = ((480.0 * f64::from(ph) / f64::from(pw)).round() as u32).max(270);
    let (thumb, _) = render(&recipes[0], 480, th, used_theme, settings.buddy.show)?;
    let entry_recipes = EntryRecipes {
        base_seed,
        theme: used_theme.as_str().to_string(),
        monitors: monitors
            .iter()
            .zip(&recipes)
            .map(|(m, r)| MonitorScene {
                monitor: m.id.clone(),
                width: m.width,
                height: m.height,
                recipe: r.clone(),
            })
            .collect(),
    };
    let entry_name = if add_to_archive {
        let entry = archive
            .add(&NewEntry {
                created: crate::timefmt::now_unix(),
                recipes: entry_recipes.clone(),
                pngs: pngs.clone(),
                thumb_png: png_of(&thumb)?,
            })
            .map_err(|e| format!("archive: {e}"))?;
        let swept = archive.sweep(
            crate::timefmt::now_unix(),
            settings.archive.keep_days,
            settings.archive.max_mb.saturating_mul(1024 * 1024),
        );
        if swept.removed > 0 {
            tracing::info!(target: "clairlune", removed = swept.removed, freed = swept.freed, "archive retention");
        }
        entry.name
    } else {
        archive.latest().map(|e| e.name).unwrap_or_default()
    };
    if let Some(host) = host {
        for (m, png) in monitors.iter().zip(&pngs) {
            let path = store.write(&m.id, png).map_err(|e| format!("still: {e}"))?;
            host.set_static(m, &path).map_err(|e| e.to_string())?;
            store.prune(&m.id, &path);
        }
    }
    let primary = &recipes[0];
    let ms = started.elapsed().as_millis() as u64;
    tracing::info!(
        target: "clairlune",
        seed = base_seed,
        theme = used_theme.as_str(),
        monitors = monitors.len(),
        archetype = %primary.archetype,
        activity = primary.activity.as_deref().unwrap_or("-"),
        recipe_hash = %primary.hash_hex(),
        ms,
        "wallpaper applied"
    );
    Ok(Done::Applied {
        entry_name,
        seed: base_seed,
        theme: used_theme,
        archetype: primary.archetype.clone(),
        activity: primary.activity.clone(),
        monitors: monitors.len(),
        ms,
    })
}

/// Puts back the remembered wallpaper and removes Clairlune's stills.
fn restore(host: Option<&dyn WallpaperHost>, store: &StillStore) -> Result<Done, String> {
    let host = host.ok_or("no wallpaper host on this system")?;
    let snapshot = store
        .original()
        .map_err(|e| e.to_string())?
        .ok_or("there is no earlier wallpaper to put back")?;
    host.restore(&snapshot).map_err(|e| e.to_string())?;
    store.forget_original().map_err(|e| e.to_string())?;
    let removed = store.clear_stills();
    tracing::info!(target: "clairlune", removed, "restored the user's wallpaper");
    Ok(Done::Restored)
}

/// Copies the newest entry's primary image to `dest`.
fn export(archive: &Archive, dest: &std::path::Path) -> Result<Done, String> {
    let entry: Entry = archive.latest().ok_or("nothing to export yet")?;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let src = archive.dir().join(&entry.name).join("wallpaper-0.png");
    let out = dest.join(format!("{}.png", entry.name));
    std::fs::copy(&src, &out).map_err(|e| format!("{}: {e}", src.display()))?;
    tracing::info!(target: "clairlune", path = %out.display(), "exported");
    Ok(Done::Exported(out))
}
