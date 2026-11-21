//! Applying wallpapers, on a worker thread: compose one scene per monitor (or
//! re-render saved recipes), render at each monitor's physical size, archive
//! the set, then set each still through the platform host. The static image is
//! always set first and kept current, so a crash never leaves a blank desktop.

use crate::archive::{Archive, Entry, EntryRecipes, MonitorScene, NewEntry};
use crate::settings::{Settings, ThemeMode, ThemeSettings};
use clairlune_platform::{Monitor, StillStore, WallpaperHost};
use clairlune_render::{IndexedImage, IndexedPalette, PixelRect, ScenePackage, encode_png};
use clairlune_scene::hash::hash_label;
use clairlune_scene::recipe::HistoryEntry;
use clairlune_scene::{Aspect, SceneRecipe, Theme, compose};
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::Instant;

/// Work for the worker thread.
pub enum Job {
    /// A new scene per monitor from a base seed.
    New {
        seed: u64,
        settings: Box<Settings>,
        /// Made now, shown then (a timeline slot prepared ahead of its start).
        at: Option<Instant>,
        /// Scrolling scenes (journeys) look right at this frame rate.
        scrolling: bool,
    },
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
        /// Made now, shown then (a timeline slot prepared ahead of its start).
        at: Option<Instant>,
        /// A new scene for this one monitor (its id), from this seed; the
        /// others keep theirs.
        renew: Option<(String, u64)>,
        /// Scrolling scenes (journeys) look right at this frame rate.
        scrolling: bool,
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
    /// Span mode: the part of the scene this monitor shows. `None`: all of it.
    pub view: Option<PixelRect>,
}

/// Span mode: the monitors' bounding box as one virtual monitor, and each
/// real monitor's rectangle inside it.
pub fn span_of(monitors: &[Monitor]) -> (Monitor, Vec<(Monitor, PixelRect)>) {
    let x0 = monitors.iter().map(|m| m.x).min().unwrap_or(0);
    let y0 = monitors.iter().map(|m| m.y).min().unwrap_or(0);
    let x1 = monitors
        .iter()
        .map(|m| m.x + m.width as i32)
        .max()
        .unwrap_or(0);
    let y1 = monitors
        .iter()
        .map(|m| m.y + m.height as i32)
        .max()
        .unwrap_or(0);
    let whole = Monitor {
        id: "span".into(),
        x: x0,
        y: y0,
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
        primary: true,
    };
    let views = monitors
        .iter()
        .map(|m| {
            let r = PixelRect {
                x: (m.x - x0) as u32,
                y: (m.y - y0) as u32,
                w: m.width,
                h: m.height,
            };
            (m.clone(), r)
        })
        .collect();
    (whole, views)
}

/// The part of an image inside `r`.
fn crop(img: &IndexedImage, r: PixelRect) -> IndexedImage {
    let mut pixels = Vec::with_capacity((r.w * r.h) as usize);
    for y in r.y..r.y + r.h {
        let a = (y * img.width + r.x) as usize;
        pixels.extend_from_slice(&img.pixels[a..a + r.w as usize]);
    }
    IndexedImage {
        width: r.w,
        height: r.h,
        pixels,
    }
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

/// The theme the settings resolve to right now.
pub fn resolve_theme(t: &ThemeSettings) -> Theme {
    resolve_theme_at(t, &jiff::ZonedDateTime::now())
}

/// The theme the settings resolve to at `now` (local time).
pub fn resolve_theme_at(t: &ThemeSettings, now: &jiff::ZonedDateTime) -> Theme {
    match t.mode {
        ThemeMode::Light => Theme::Light,
        ThemeMode::Dark => Theme::Dark,
        ThemeMode::FollowSystem => match clairlune_platform::system_prefers_dark() {
            Some(false) => Theme::Light,
            _ => Theme::Dark,
        },
        ThemeMode::Schedule => {
            if t.follow_sun
                && let Some((lat, lon)) = crate::sun::parse_location(&t.location)
            {
                return if crate::sun::is_day(now.date(), lat, lon, now.timestamp()) {
                    Theme::Light
                } else {
                    Theme::Dark
                };
            }
            // Times that don't parse fall back to the defaults.
            let dark = crate::settings::parse_clock(&t.dark_from).unwrap_or(19 * 60);
            let light = crate::settings::parse_clock(&t.light_from).unwrap_or(7 * 60);
            let m =
                u32::from(now.hour().unsigned_abs()) * 60 + u32::from(now.minute().unsigned_abs());
            let is_dark = if dark > light {
                m >= dark || m < light
            } else {
                dark <= m && m < light
            };
            if is_dark { Theme::Dark } else { Theme::Light }
        }
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
    want: &Want,
) -> Result<(u64, Vec<ScenePackage>, usize, bool), String> {
    let (base_seed, recipes, changed) = scenes(
        saved.map(|s| s.recipes),
        seed,
        monitors,
        settings,
        recent,
        want,
    );
    let mut out = Vec::new();
    let mut replayed = 0;
    for (m, recipe) in monitors.iter().zip(&recipes) {
        let frozen = saved.and_then(|src| {
            let dir = src.dir?;
            let j = src
                .recipes
                .monitors
                .iter()
                .position(|s| {
                    s.monitor == m.id && (s.width, s.height) == (m.width, m.height) && s.recipe == *recipe
                })?;
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
    Ok((base_seed, out, replayed, changed))
}

/// What the new scenes must fit.
struct Want {
    /// The theme they will be shown in.
    theme: Theme,
    /// Re-showing what's on screen (a theme change): a saved scene with
    /// nothing for this theme gives way to a new one.
    refresh: bool,
    /// One monitor (its id) to give a new scene, from this seed.
    renew: Option<(String, u64)>,
    /// Span mode: the focal group's place on the virtual monitor.
    focus_pm: Option<u32>,
    /// Scrolling scenes (journeys) are fine: live plays smoothly enough,
    /// or the wallpaper is a still frame.
    scrolling: bool,
}

impl Want {
    /// Whether a saved scene still fits: the theme has a variant for it, and
    /// it doesn't scroll where scrolling can't look right.
    fn fits(&self, r: &SceneRecipe) -> bool {
        r.tod_for(self.theme).is_some() && (self.scrolling || !r.is_journey())
    }
}

fn scenes(
    job_recipes: Option<&EntryRecipes>,
    seed: u64,
    monitors: &[Monitor],
    settings: &Settings,
    recent: &mut Recent,
    want: &Want,
) -> (u64, Vec<SceneRecipe>, bool) {
    let mut ctx = settings.content_context();
    // Only scenes that happen in the theme they'll be shown in: no daytime
    // scene on a dark desktop.
    ctx.theme = Some(want.theme);
    // No scrolling scenes at a frame rate too low to scroll smoothly.
    if !want.scrolling {
        let catalog = clairlune_scene::activity::builtin_catalog();
        ctx.enabled_activities.retain(|id| {
            catalog
                .get(id)
                .is_none_or(|a| a.motion.kind != clairlune_scene::activity::MotionKind::Journey)
        });
    }
    // Composes a new scene with the recent picks, then remembers it, so the
    // next monitor (and the next wallpaper) avoids it. A span scene puts its
    // focal group on the primary monitor (`focus_pm`), never across a seam.
    let mut new_scene = |seed: u64, m: &Monitor| {
        ctx.recent = recent.entries().to_vec();
        let mut r = compose(seed, Aspect::from_size(m.width, m.height), &ctx);
        if let Some(f) = want.focus_pm {
            r.composition.focus_x_pm = f;
        }
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
            false,
        ),
        Some(r) => {
            let mut changed = false;
            let recipes = monitors
                .iter()
                .map(|m| {
                    if let Some((id, s)) = &want.renew
                        && *id == m.id
                    {
                        changed = true;
                        return new_scene(monitor_seed(*s, m), m);
                    }
                    match r.monitors.iter().find(|s| s.monitor == m.id) {
                        Some(s) if !want.refresh || want.fits(&s.recipe) => s.recipe.clone(),
                        // The theme changed and this scene has nothing for
                        // it (or it scrolls, and live went to a low frame
                        // rate): a new one, the same each time for this
                        // wallpaper and theme.
                        Some(_) => {
                            changed = true;
                            let salt = hash_label(r.base_seed, want.theme.as_str());
                            new_scene(monitor_seed(salt, m), m)
                        }
                        None => new_scene(monitor_seed(r.base_seed, m), m),
                    }
                })
                .collect();
            (r.base_seed, recipes, changed)
        }
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
        Job::New {
            seed,
            settings,
            at,
            scrolling,
        } => apply(
            None, seed, &settings, true, host, store, archive, recent, at, scrolling,
        ),
        Job::Recipes {
            recipes,
            source,
            theme,
            buddy,
            settings,
            archive: add,
            at,
            renew,
            scrolling,
        } => apply(
            Some(Source {
                recipes: &recipes,
                dir: source.as_deref(),
                theme,
                buddy,
                renew: renew.as_ref().map(|(id, s)| (id.as_str(), *s)),
            }),
            0,
            &settings,
            add,
            host,
            store,
            archive,
            recent,
            at,
            scrolling,
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
    /// One monitor to give a new scene (its id, and the seed).
    renew: Option<(&'a str, u64)>,
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
    at: Option<Instant>,
    scrolling: bool,
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
    // Span mode: compose, package and archive one scene the size of all the
    // monitors together; each monitor then shows its part.
    let screens = monitors.clone();
    let span = (settings.display.span && screens.len() > 1).then(|| span_of(&screens));
    let (monitors, focus_pm) = match &span {
        Some((whole, views)) => {
            let primary = views.iter().find(|(m, _)| m.primary).unwrap_or(&views[0]).1;
            let centre =
                u64::from(primary.x + primary.w / 2) * 1000 / u64::from(whole.width.max(1));
            (vec![whole.clone()], Some(centre as u32))
        }
        None => (monitors, None),
    };
    let wanted = saved
        .and_then(|s| s.theme)
        .unwrap_or_else(|| resolve_theme(&settings.theme));
    let buddy = saved.and_then(|s| s.buddy).unwrap_or(settings.buddy.show);
    let want = Want {
        theme: wanted,
        // Showing what's there again in the current theme (not a previous
        // wallpaper, nor a library item with a theme of its own).
        refresh: saved.is_some_and(|s| s.theme.is_none()) && !add_to_archive,
        renew: saved
            .and_then(|s| s.renew)
            .map(|(id, s)| (id.to_string(), s)),
        focus_pm,
        scrolling,
    };
    let (base_seed, packages, replayed, changed) =
        packages(saved, seed, &monitors, settings, recent, &want)?;
    // A monitor got a new scene: that's a new wallpaper, so Previous can go
    // back to the one before.
    let add_to_archive = add_to_archive || changed;
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
    // Prepared ahead (a timeline slot): everything is made; show it on time.
    if let Some(at) = at {
        let now = Instant::now();
        if at > now {
            std::thread::sleep(at - now);
        }
    }
    // The thumbnail: the primary scene shrunk to 480 px wide.
    let (img, palette) = primary_still.ok_or("no monitors")?;
    let thumb = shrink(&img, 480);
    let span_still = span.as_ref().map(|_| (img.clone(), palette.clone()));
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
    if let (Some(host), Some((_, views)), Some((img, palette))) = (host, &span, &span_still) {
        // Each monitor gets its part of the one scene.
        for (m, view) in views {
            let png = png_of(&crop(img, *view), palette)?;
            let path = store
                .write(&m.id, &png)
                .map_err(|e| format!("still: {e}"))?;
            host.set_static(m, &path).map_err(|e| e.to_string())?;
            store.prune(&m.id, &path);
            live.push(LiveScene {
                monitor: m.clone(),
                package: packages[0].clone(),
                theme: themes[0],
                buddy,
                still: Some(path),
                view: Some(*view),
            });
        }
    } else if let Some(host) = host {
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
                view: None,
            });
        }
    }
    let primary = &recipes[0];
    let ms = started.elapsed().as_millis() as u64;
    tracing::info!(
        target: "clairlune",
        seed = base_seed,
        theme = used_theme.as_str(),
        monitors = screens.len(),
        span = span.is_some(),
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
        monitors: screens.len(),
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

    fn want(theme: Theme) -> Want {
        Want {
            theme,
            refresh: false,
            renew: None,
            focus_pm: None,
            scrolling: true,
        }
    }

    fn two_monitors() -> [Monitor; 2] {
        [
            monitor(640, 360),
            Monitor {
                id: "second".into(),
                x: 640,
                primary: false,
                ..monitor(640, 360)
            },
        ]
    }

    /// Saved recipes for these monitors, one per monitor.
    fn saved(monitors: &[Monitor], recipes: &[SceneRecipe]) -> EntryRecipes {
        EntryRecipes {
            base_seed: 5,
            theme: "dark".into(),
            monitors: monitors
                .iter()
                .zip(recipes)
                .map(|(m, r)| MonitorScene {
                    monitor: m.id.clone(),
                    width: m.width,
                    height: m.height,
                    recipe: r.clone(),
                })
                .collect(),
        }
    }

    #[test]
    fn new_scenes_happen_in_the_theme_they_are_shown_in() {
        let settings = Settings::default();
        let mut recent = Recent::default();
        for theme in [Theme::Dark, Theme::Light] {
            for seed in 0..40u64 {
                let (_, recipes, _) = scenes(
                    None,
                    seed,
                    &two_monitors(),
                    &settings,
                    &mut recent,
                    &want(theme),
                );
                for r in &recipes {
                    assert!(r.tod_for(theme).is_some(), "seed {seed}: {:?}", r.activity);
                }
            }
        }
    }

    #[test]
    fn a_theme_change_replaces_a_scene_without_that_theme() {
        let settings = Settings::default();
        let screens = two_monitors();
        let day_only = |seed| {
            let ctx = clairlune_scene::ContentContext {
                enabled_activities: vec!["tennis".into()],
                ..clairlune_scene::ContentContext::default()
            };
            compose(seed, Aspect::from_size(640, 360), &ctx)
        };
        let both = compose(9, Aspect::from_size(640, 360), &settings.content_context());
        let both = if both.is_theme_locked() {
            day_only(1)
        } else {
            both
        };
        let day = day_only(2);
        assert!(day.tod_for(Theme::Dark).is_none());
        let entry = saved(&screens, &[both.clone(), day.clone()]);
        // Shown again in dark after the system went dark: the day-only
        // scene gives way, the other stays.
        let mut w = want(Theme::Dark);
        w.refresh = true;
        let (_, recipes, changed) = scenes(
            Some(&entry),
            0,
            &screens,
            &settings,
            &mut Recent::default(),
            &w,
        );
        assert!(changed);
        assert_eq!(recipes[0], both);
        assert!(recipes[1].tod_for(Theme::Dark).is_some());
        // The same replacement each time (no flicker between re-renders).
        let (_, again, _) = scenes(
            Some(&entry),
            0,
            &screens,
            &settings,
            &mut Recent::default(),
            &w,
        );
        assert_eq!(again[1].activity, recipes[1].activity);
        // Not a re-render (Previous): the saved scenes as they were.
        let (_, kept, changed) = scenes(
            Some(&entry),
            0,
            &screens,
            &settings,
            &mut Recent::default(),
            &want(Theme::Dark),
        );
        assert!(!changed);
        assert_eq!(kept, vec![both, day]);
    }

    #[test]
    fn no_scrolling_scenes_at_a_low_frame_rate() {
        let mut settings = Settings::default();
        settings.content.activities = vec!["hike_ridge".into(), "warm_hands".into()];
        let screens = two_monitors();
        let mut w = want(Theme::Dark);
        w.scrolling = false;
        for seed in 0..30u64 {
            let (_, recipes, _) =
                scenes(None, seed, &screens, &settings, &mut Recent::default(), &w);
            assert!(recipes.iter().all(|r| !r.is_journey()), "seed {seed}");
        }
        // A journey on screen when the frame rate drops gives way.
        let hike = {
            let ctx = clairlune_scene::ContentContext {
                enabled_activities: vec!["hike_ridge".into()],
                enabled_archetypes: vec!["mountains".into()],
                ..clairlune_scene::ContentContext::default()
            };
            compose(1, Aspect::from_size(640, 360), &ctx)
        };
        assert!(hike.is_journey());
        let (_, kept, _) = scenes(None, 2, &screens, &settings, &mut Recent::default(), &w);
        let entry = saved(&screens, &[kept[0].clone(), hike]);
        w.refresh = true;
        let (_, after, changed) = scenes(
            Some(&entry),
            0,
            &screens,
            &settings,
            &mut Recent::default(),
            &w,
        );
        assert!(changed);
        assert_eq!(after[0], kept[0]);
        assert!(!after[1].is_journey());
    }

    #[test]
    fn one_monitor_gets_a_new_scene_and_the_other_keeps_its_own() {
        let settings = Settings::default();
        let screens = two_monitors();
        let mut recent = Recent::default();
        let (_, first, _) = scenes(
            None,
            3,
            &screens,
            &settings,
            &mut recent,
            &want(Theme::Dark),
        );
        let entry = saved(&screens, &first);
        let mut w = want(Theme::Dark);
        w.renew = Some(("second".into(), 1234));
        let (_, next, changed) = scenes(Some(&entry), 0, &screens, &settings, &mut recent, &w);
        assert!(changed);
        assert_eq!(next[0], first[0]);
        assert_ne!(next[1], first[1]);
        assert!(next[1].tod_for(Theme::Dark).is_some());
    }

    #[test]
    fn saved_scenes_replay_when_the_monitor_matches() {
        let dir = std::env::temp_dir().join(format!("clairlune-apply-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let archive = Archive::open(&dir).unwrap();
        let settings = Settings::default();
        let m = monitor(640, 360);
        // A new wallpaper: composed and captured.
        let (seed, pkgs, replayed, _) = packages(
            None,
            77,
            std::slice::from_ref(&m),
            &settings,
            &mut Recent::default(),
            &want(Theme::Dark),
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
            renew: None,
        };
        // Same monitor: replayed from the package.
        let (_, again, replayed, _) = packages(
            Some(src),
            0,
            std::slice::from_ref(&m),
            &settings,
            &mut Recent::default(),
            &want(Theme::Dark),
        )
        .unwrap();
        assert_eq!(replayed, 1);
        assert_eq!(again[0].recipe, recipes.monitors[0].recipe);
        // A different size: rendered again from the recipe.
        let (_, resized, replayed, _) = packages(
            Some(src),
            0,
            &[monitor(800, 450)],
            &settings,
            &mut Recent::default(),
            &want(Theme::Dark),
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
            let (_, recipes, _) =
                scenes(None, seed, &two, &settings, &mut recent, &want(Theme::Dark));
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

    #[test]
    fn span_covers_every_monitor_with_its_own_part() {
        let m = |id: &str, x: i32, y: i32, w: u32, h: u32, primary: bool| Monitor {
            id: id.into(),
            x,
            y,
            width: w,
            height: h,
            primary,
        };
        // A taller primary and a smaller monitor to its right, lower down.
        let (whole, views) = span_of(&[
            m("a", 0, 0, 2560, 1440, true),
            m("b", 2560, 180, 1920, 1080, false),
        ]);
        assert_eq!(
            (whole.x, whole.y, whole.width, whole.height),
            (0, 0, 4480, 1440)
        );
        assert_eq!(
            views[0].1,
            PixelRect {
                x: 0,
                y: 0,
                w: 2560,
                h: 1440
            }
        );
        assert_eq!(
            views[1].1,
            PixelRect {
                x: 2560,
                y: 180,
                w: 1920,
                h: 1080
            }
        );
        // A monitor to the left of the primary (negative coordinates).
        let (whole, views) = span_of(&[
            m("a", 0, 0, 1920, 1080, true),
            m("b", -1920, 0, 1920, 1080, false),
        ]);
        assert_eq!((whole.x, whole.width), (-1920, 3840));
        assert_eq!(views[0].1.x, 1920);
        assert_eq!(views[1].1.x, 0);
    }

    /// Records what each monitor was set to.
    struct FakeHost {
        monitors: Vec<Monitor>,
        set: std::sync::Mutex<Vec<(String, PathBuf)>>,
    }

    impl WallpaperHost for FakeHost {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn capabilities(&self) -> clairlune_platform::Capabilities {
            clairlune_platform::Capabilities {
                per_monitor: true,
                live: false,
            }
        }
        fn monitors(&self) -> Result<Vec<Monitor>, clairlune_platform::PlatformError> {
            Ok(self.monitors.clone())
        }
        fn current(
            &self,
            _: &Monitor,
        ) -> Result<Option<PathBuf>, clairlune_platform::PlatformError> {
            Ok(None)
        }
        fn set_static(
            &self,
            m: &Monitor,
            path: &Path,
        ) -> Result<(), clairlune_platform::PlatformError> {
            self.set
                .lock()
                .unwrap()
                .push((m.id.clone(), path.to_path_buf()));
            Ok(())
        }
        fn snapshot(
            &self,
        ) -> Result<clairlune_platform::Snapshot, clairlune_platform::PlatformError> {
            Ok(clairlune_platform::Snapshot {
                monitors: Vec::new(),
                position: None,
            })
        }
        fn restore(
            &self,
            _: &clairlune_platform::Snapshot,
        ) -> Result<(), clairlune_platform::PlatformError> {
            Ok(())
        }
    }

    /// Span mode end to end: one scene the size of both monitors, archived
    /// once; each monitor gets a still of its own size cut from it, and a live
    /// scene showing its part, with the focal group on the primary monitor.
    #[test]
    fn span_mode_spreads_one_scene_across_the_monitors() {
        let dir = std::env::temp_dir().join(format!("clairlune-span-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let host = FakeHost {
            monitors: vec![
                Monitor {
                    id: "left".into(),
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 360,
                    primary: true,
                },
                Monitor {
                    id: "right".into(),
                    x: 640,
                    y: 40,
                    width: 480,
                    height: 270,
                    primary: false,
                },
            ],
            set: std::sync::Mutex::new(Vec::new()),
        };
        let store = StillStore::new(dir.join("current")).unwrap();
        let archive = Archive::open(dir.join("archive")).unwrap();
        let mut settings = Settings::default();
        settings.display.span = true;
        let done = apply(
            None,
            11,
            &settings,
            true,
            Some(&host),
            &store,
            &archive,
            &mut Recent::default(),
            None,
            true,
        )
        .unwrap();
        let Done::Applied { live, monitors, .. } = done else {
            panic!("applied")
        };
        assert_eq!(monitors, 2);
        assert_eq!(live.len(), 2);
        // One scene, the size of the bounding box, shared by both.
        assert!(Arc::ptr_eq(&live[0].package, &live[1].package));
        assert_eq!((live[0].package.width, live[0].package.height), (1120, 360));
        assert_eq!(
            live[0].view,
            Some(PixelRect {
                x: 0,
                y: 0,
                w: 640,
                h: 360
            })
        );
        assert_eq!(
            live[1].view,
            Some(PixelRect {
                x: 640,
                y: 40,
                w: 480,
                h: 270
            })
        );
        // Each monitor's still is its own size.
        let set = host.set.lock().unwrap();
        assert_eq!(set.len(), 2);
        for ((id, path), m) in set.iter().zip(&host.monitors) {
            assert_eq!(id, &m.id);
            let dec =
                png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
            let info = dec.read_info().unwrap().info().clone();
            assert_eq!((info.width, info.height), (m.width, m.height));
        }
        // The focal group sits on the primary (left) monitor.
        let graph = live[0].package.graph(live[0].theme).unwrap();
        let track = graph.character.unwrap();
        if !track.flies() {
            let (lo, hi) = track.x_range();
            assert!(
                hi < 640 && lo >= 0,
                "the group at {lo}..{hi} isn't on the primary monitor"
            );
        }
        // Archived once, as one scene.
        assert_eq!(archive.entries().len(), 1);
        drop(set);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
