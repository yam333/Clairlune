//! Applying wallpapers, on a worker thread: compose one scene per monitor (or
//! re-render saved recipes), render at each monitor's physical size, archive
//! the set, then set each still through the platform host. The static image is
//! always set first and kept current, so a crash never leaves a blank desktop.

use crate::archive::{Archive, Entry, EntryRecipes, MonitorScene, NewEntry};
use crate::settings::{Settings, ThemeMode};
use clairlune_platform::{Monitor, StillStore, WallpaperHost};
use clairlune_render::{IndexedImage, IndexedPalette, ScenePackage, encode_png};
use clairlune_scene::hash::hash_label;
use clairlune_scene::recipe::HistoryEntry;
use clairlune_scene::{Aspect, SceneRecipe, Theme, compose};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Instant;

/// Work for the worker thread.
pub enum Job {
    /// A new scene per monitor from a base seed.
    New { seed: u64, settings: Box<Settings> },
    /// Saved recipes again (previous wallpaper, theme change, resolution
    /// change): same scenes, rendered for the current monitors and theme.
    Recipes {
        recipes: Box<EntryRecipes>,
        /// The folder they came from (archive entry or library item): its
        /// frozen scenes are played as saved when the monitors still match.
        source: Option<PathBuf>,
        /// A library item's own theme and Clawd choices, over the settings.
        theme: Option<Theme>,
        buddy: Option<bool>,
        settings: Box<Settings>,
        /// Archive it as a new entry (previous wallpaper) or only re-render
        /// the current one (theme change).
        archive: bool,
    },
    /// Copy the current wallpaper's primary image to the export folder.
    /// From `source` (an archive entry or library item folder), or the newest
    /// archive entry.
    Export {
        dest: PathBuf,
        source: Option<PathBuf>,
    },
    /// Export a saved scene as a looping GIF.
    ExportGif {
        dest: PathBuf,
        source: Option<PathBuf>,
        theme: Theme,
        buddy: bool,
    },
    /// Put back the wallpaper the user had before Clairlune.
    Restore,
}

/// One monitor's scene as applied, for playing it live.
#[derive(Clone, Debug)]
#[cfg_attr(not(windows), allow(dead_code))] // live playback is Windows-only so far
pub struct LiveScene {
    pub monitor: Monitor,
    pub package: Arc<ScenePackage>,
    pub theme: Theme,
    pub buddy: bool,
    /// The still set as this monitor's wallpaper (the live frame at the hero
    /// time), set again when live playback stops.
    pub still: Option<PathBuf>,
}

/// What happened, for the tray tooltip and logs.
#[derive(Clone, Debug)]
pub enum Done {
    Applied {
        /// The folder of what's on screen (archive entry or library item).
        source: Option<PathBuf>,
        seed: u64,
        theme: Theme,
        archetype: String,
        activity: Option<String>,
        monitors: usize,
        ms: u64,
        /// Each monitor's scene, to play live (empty without a host).
        live: Vec<LiveScene>,
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

pub(crate) fn png_of(img: &IndexedImage, palette: &IndexedPalette) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    encode_png(img, palette, &mut png).map_err(|e| e.to_string())?;
    Ok(png)
}

/// Nearest-neighbour shrink to `width` (thumbnails keep the palette).
pub(crate) fn shrink(img: &IndexedImage, width: u32) -> IndexedImage {
    let width = width.min(img.width).max(1);
    let height = ((u64::from(img.height) * u64::from(width)) / u64::from(img.width)).max(1) as u32;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        let sy = (u64::from(y) * u64::from(img.height) / u64::from(height)) as usize;
        for x in 0..width {
            let sx = (u64::from(x) * u64::from(img.width) / u64::from(width)) as usize;
            pixels.push(img.pixels[sy * img.width as usize + sx]);
        }
    }
    IndexedImage {
        width,
        height,
        pixels,
    }
}

/// One frozen scene per monitor. A saved entry's scenes are replayed as they
/// were saved when the monitor id and size still match (no generator
/// involved); otherwise the recipe is rendered again, and new wallpapers are
/// composed from the seed. Returns the base seed, the scenes, and how many
/// were replayed.
fn packages(
    saved: Option<Source>,
    seed: u64,
    monitors: &[Monitor],
    settings: &Settings,
    recent: &mut Recent,
) -> Result<(u64, Vec<ScenePackage>, usize), String> {
    let (base_seed, recipes) = scenes(saved.map(|s| s.recipes), seed, monitors, settings, recent);
    let mut out = Vec::new();
    let mut replayed = 0;
    for (m, recipe) in monitors.iter().zip(&recipes) {
        let frozen = saved.and_then(|src| {
            let dir = src.dir?;
            let j = src
                .recipes
                .monitors
                .iter()
                .position(|s| s.monitor == m.id && (s.width, s.height) == (m.width, m.height))?;
            match ScenePackage::load(&dir.join("scene").join(j.to_string())) {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!(target: "clairlune", dir = %dir.display(), error = %e, "saved scene unreadable; rendering the recipe");
                    None
                }
            }
        });
        let pkg = match frozen {
            Some(p) => {
                replayed += 1;
                p
            }
            None => ScenePackage::capture(recipe, m.width, m.height).map_err(|e| e.to_string())?,
        };
        out.push(pkg);
    }
    Ok((base_seed, out, replayed))
}

fn scenes(
    job_recipes: Option<&EntryRecipes>,
    seed: u64,
    monitors: &[Monitor],
    settings: &Settings,
    recent: &mut Recent,
) -> (u64, Vec<SceneRecipe>) {
    let mut ctx = settings.content_context();
    // Composes a new scene with the recent picks, then remembers it, so the
    // next monitor (and the next wallpaper) avoids it.
    let mut new_scene = |seed: u64, m: &Monitor| {
        ctx.recent = recent.entries().to_vec();
        let r = compose(seed, Aspect::from_size(m.width, m.height), &ctx);
        recent.push(&r);
        r
    };
    match job_recipes {
        None => (
            seed,
            monitors
                .iter()
                .map(|m| new_scene(monitor_seed(seed, m), m))
                .collect(),
        ),
        Some(r) => (
            r.base_seed,
            monitors
                .iter()
                .map(|m| match r.monitors.iter().find(|s| s.monitor == m.id) {
                    Some(s) => s.recipe.clone(),
                    None => new_scene(monitor_seed(r.base_seed, m), m),
                })
                .collect(),
        ),
    }
}

/// The most recent picks, newest last (PLAN.md section 4.7): new scenes skip
/// the last activity and weigh down recent ones and the last landscape, so
/// wallpapers in a row, and the monitors of one wallpaper, differ. Kept in
/// `<data>/recent.json` across restarts.
#[derive(Debug, Default)]
pub struct Recent {
    entries: Vec<HistoryEntry>,
    path: Option<PathBuf>,
}

impl Recent {
    /// How many picks are remembered.
    pub const KEEP: usize = 12;

    /// Loads the list; a missing or unreadable file is an empty one.
    pub fn load(path: PathBuf) -> Recent {
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Recent {
            entries,
            path: Some(path),
        }
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Remembers a composed scene (and saves the list).
    fn push(&mut self, r: &SceneRecipe) {
        self.entries.push(HistoryEntry {
            activity: r.activity.clone(),
            archetype: r.archetype.clone(),
            platform: r.platform.clone(),
            prop: r.props.first().cloned(),
        });
        let extra = self.entries.len().saturating_sub(Self::KEEP);
        self.entries.drain(..extra);
        if let Some(path) = &self.path
            && let Ok(json) = serde_json::to_string_pretty(&self.entries)
            && let Err(e) = std::fs::write(path, json)
        {
            tracing::warn!(target: "clairlune", error = %e, "could not save recent picks");
        }
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
    let mut recent = Recent::load(
        archive_dir
            .parent()
            .unwrap_or(&archive_dir)
            .join("recent.json"),
    );
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
            (Ok(archive), Ok(store)) => run(job, host.as_deref(), store, archive, &mut recent),
            (Err(e), _) | (_, Err(e)) => Done::Failed(format!("data folder: {e}")),
        };
        if let Done::Failed(e) = &result {
            tracing::error!(target: "clairlune", error = %e, "job failed");
        }
        done(result);
    }
}

fn run(
    job: Job,
    host: Option<&dyn WallpaperHost>,
    store: &StillStore,
    archive: &Archive,
    recent: &mut Recent,
) -> Done {
    match job {
        Job::Export { dest, source } => export(archive, &dest, source.as_deref()),
        Job::ExportGif {
            dest,
            source,
            theme,
            buddy,
        } => export_gif(archive, &dest, source.as_deref(), theme, buddy),
        Job::Restore => restore(host, store),
        Job::New { seed, settings } => {
            apply(None, seed, &settings, true, host, store, archive, recent)
        }
        Job::Recipes {
            recipes,
            source,
            theme,
            buddy,
            settings,
            archive: add,
        } => apply(
            Some(Source {
                recipes: &recipes,
                dir: source.as_deref(),
                theme,
                buddy,
            }),
            0,
            &settings,
            add,
            host,
            store,
            archive,
            recent,
        ),
    }
    .unwrap_or_else(Done::Failed)
}

/// Saved scenes to show again, and how.
#[derive(Clone, Copy)]
struct Source<'a> {
    recipes: &'a EntryRecipes,
    /// Folder holding `scene/<i>/` packages.
    dir: Option<&'a Path>,
    theme: Option<Theme>,
    buddy: Option<bool>,
}

#[allow(clippy::too_many_arguments)]
fn apply(
    saved: Option<Source>,
    seed: u64,
    settings: &Settings,
    add_to_archive: bool,
    host: Option<&dyn WallpaperHost>,
    store: &StillStore,
    archive: &Archive,
    recent: &mut Recent,
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
    let wanted = saved
        .and_then(|s| s.theme)
        .unwrap_or_else(|| resolve_theme(settings.theme.mode));
    let buddy = saved.and_then(|s| s.buddy).unwrap_or(settings.buddy.show);
    let (base_seed, packages, replayed) = packages(saved, seed, &monitors, settings, recent)?;
    let packages: Vec<Arc<ScenePackage>> = packages.into_iter().map(Arc::new).collect();
    let recipes: Vec<SceneRecipe> = packages.iter().map(|p| p.recipe.clone()).collect();
    let mut pngs = Vec::new();
    let mut themes = Vec::new();
    let mut used_theme = wanted;
    let mut primary_still = None;
    for (i, pkg) in packages.iter().enumerate() {
        // Night-only scenes keep their theme.
        let theme = if pkg.themes().contains(&wanted) {
            wanted
        } else {
            pkg.themes()[0]
        };
        themes.push(theme);
        let (img, palette) = pkg.still(theme, buddy).ok_or("scene has no frames")?;
        pngs.push(png_of(&img, palette)?);
        if i == 0 {
            used_theme = theme;
            primary_still = Some((img, palette.clone()));
        }
    }
    // The thumbnail: the primary scene shrunk to 480 px wide.
    let (img, palette) = primary_still.ok_or("no monitors")?;
    let thumb = shrink(&img, 480);
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
    let source = if add_to_archive {
        let entry = archive
            .add(&NewEntry {
                created: crate::timefmt::now_unix(),
                recipes: entry_recipes.clone(),
                pngs: pngs.clone(),
                thumb_png: png_of(&thumb, &palette)?,
                packages: packages.clone(),
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
        Some(archive.dir().join(&entry.name))
    } else {
        saved.and_then(|s| s.dir.map(Path::to_path_buf))
    };
    let mut live = Vec::new();
    if let Some(host) = host {
        for (i, (m, png)) in monitors.iter().zip(&pngs).enumerate() {
            let path = store.write(&m.id, png).map_err(|e| format!("still: {e}"))?;
            host.set_static(m, &path).map_err(|e| e.to_string())?;
            store.prune(&m.id, &path);
            live.push(LiveScene {
                monitor: m.clone(),
                package: packages[i].clone(),
                theme: themes[i],
                buddy,
                still: Some(path),
            });
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
        replayed,
        ms,
        memory_mb = crate::memory_mb(),
        "wallpaper applied"
    );
    Ok(Done::Applied {
        source,
        seed: base_seed,
        theme: used_theme,
        archetype: primary.archetype.clone(),
        activity: primary.activity.clone(),
        monitors: monitors.len(),
        ms,
        live,
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

/// The folder to export from: `source`, or the newest archive entry.
fn export_source(archive: &Archive, source: Option<&Path>) -> Result<PathBuf, String> {
    match source {
        Some(dir) => Ok(dir.to_path_buf()),
        None => {
            let entry: Entry = archive.latest().ok_or("nothing to export yet")?;
            Ok(archive.dir().join(entry.name))
        }
    }
}

fn file_stem(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "clairlune".into())
}

/// Exports a saved scene (primary monitor) as a 6 s looping GIF, played from
/// its frozen package, so it matches what was on screen.
fn export_gif(
    archive: &Archive,
    dest: &Path,
    source: Option<&Path>,
    theme: Theme,
    buddy: bool,
) -> Result<Done, String> {
    let dir = export_source(archive, source)?;
    let pkg = ScenePackage::load(&dir.join("scene").join("0"))
        .map_err(|e| format!("saved scene: {e}"))?;
    let theme = if pkg.themes().contains(&theme) {
        theme
    } else {
        pkg.themes()[0]
    };
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let out = dest.join(format!("{}.gif", file_stem(&dir)));
    let file = std::io::BufWriter::new(std::fs::File::create(&out).map_err(|e| e.to_string())?);
    let stats = clairlune_render::encode_package_gif(
        &pkg,
        theme,
        buddy,
        6000,
        clairlune_render::Smoothness::Balanced,
        file,
    )
    .map_err(|e| e.to_string())?;
    tracing::info!(target: "clairlune", path = %out.display(), frames = stats.frames_written, ms = stats.elapsed_ms, "exported gif");
    Ok(Done::Exported(out))
}

/// Copies a saved scene's primary image to `dest`.
fn export(archive: &Archive, dest: &Path, source: Option<&Path>) -> Result<Done, String> {
    let dir = export_source(archive, source)?;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let src = dir.join("wallpaper-0.png");
    let out = dest.join(format!("{}.png", file_stem(&dir)));
    std::fs::copy(&src, &out).map_err(|e| format!("{}: {e}", src.display()))?;
    tracing::info!(target: "clairlune", path = %out.display(), "exported");
    Ok(Done::Exported(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(w: u32, h: u32) -> Monitor {
        Monitor {
            id: r"\?\DISPLAY#TEST#1".into(),
            x: 0,
            y: 0,
            width: w,
            height: h,
            primary: true,
        }
    }

    #[test]
    fn saved_scenes_replay_when_the_monitor_matches() {
        let dir = std::env::temp_dir().join(format!("clairlune-apply-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let archive = Archive::open(&dir).unwrap();
        let settings = Settings::default();
        let m = monitor(640, 360);
        // A new wallpaper: composed and captured.
        let (seed, pkgs, replayed) = packages(
            None,
            77,
            std::slice::from_ref(&m),
            &settings,
            &mut Recent::default(),
        )
        .unwrap();
        assert_eq!((seed, replayed), (77, 0));
        let recipes = EntryRecipes {
            base_seed: seed,
            theme: "dark".into(),
            monitors: vec![MonitorScene {
                monitor: m.id.clone(),
                width: 640,
                height: 360,
                recipe: pkgs[0].recipe.clone(),
            }],
        };
        let entry = archive
            .add(&NewEntry {
                created: 1,
                recipes: recipes.clone(),
                pngs: vec![vec![0; 4]],
                thumb_png: vec![0; 4],
                packages: pkgs.into_iter().map(Arc::new).collect(),
            })
            .unwrap();
        let entry_dir = archive.dir().join(&entry.name);
        let src = Source {
            recipes: &recipes,
            dir: Some(&entry_dir),
            theme: None,
            buddy: None,
        };
        // Same monitor: replayed from the package.
        let (_, again, replayed) = packages(
            Some(src),
            0,
            std::slice::from_ref(&m),
            &settings,
            &mut Recent::default(),
        )
        .unwrap();
        assert_eq!(replayed, 1);
        assert_eq!(again[0].recipe, recipes.monitors[0].recipe);
        // A different size: rendered again from the recipe.
        let (_, resized, replayed) = packages(
            Some(src),
            0,
            &[monitor(800, 450)],
            &settings,
            &mut Recent::default(),
        )
        .unwrap();
        assert_eq!(replayed, 0);
        assert_eq!((resized[0].width, resized[0].height), (800, 450));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recent_picks_keep_wallpapers_and_monitors_apart() {
        let settings = Settings::default();
        let path =
            std::env::temp_dir().join(format!("clairlune-recent-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut recent = Recent::load(path.clone());
        let two = [
            monitor(640, 360),
            Monitor {
                id: "second".into(),
                x: 640,
                primary: false,
                ..monitor(640, 360)
            },
        ];
        let mut last: Option<String> = None;
        for seed in 0..20u64 {
            let (_, recipes) = scenes(None, seed, &two, &settings, &mut recent);
            let acts: Vec<_> = recipes.iter().map(|r| r.activity.clone()).collect();
            // The two monitors of one wallpaper differ...
            assert_ne!(acts[0], acts[1], "seed {seed}");
            // ...and a wallpaper doesn't start with the activity just shown.
            assert_ne!(acts[0], last, "seed {seed}");
            last = acts[1].clone();
        }
        assert_eq!(recent.entries().len(), Recent::KEEP);
        // The list survives a restart.
        assert_eq!(Recent::load(path.clone()).entries(), recent.entries());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn thumbnails_shrink_to_width() {
        let img = IndexedImage {
            width: 1920,
            height: 1080,
            pixels: vec![3; 1920 * 1080],
        };
        let t = shrink(&img, 480);
        assert_eq!((t.width, t.height), (480, 270));
        assert!(t.pixels.iter().all(|&p| p == 3));
    }
}
