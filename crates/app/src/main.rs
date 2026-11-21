// No console window for the tray app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! The Clairlune desktop app: a tray icon that sets a new procedural wallpaper
//! on a schedule, one scene per monitor, keeping an archive of past ones.
//!
//! One event loop for the whole process (PLAN.md section 3.4): the tray, and
//! later the live wallpaper windows and the UI windows, all live on it. Work
//! that takes time (rendering, archiving) runs on a worker thread and reports
//! back as a user event.

mod apply;
mod archive;
mod diagnostics;
mod icon;
mod ipc;
mod library;
mod live;
mod saver;
mod settings;
mod sun;
mod timefmt;
mod timeline;
mod tokens;
mod tray;
mod ui;
mod update;

use apply::{Done, Job};
use clairlune_scene::{APP_NAME, Theme};
use settings::Settings;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

/// How often to look for settings edits and system theme changes.
const CHECK_EVERY: Duration = Duration::from_secs(20);

/// Sequential pool positions and the last pick per slot, kept in
/// `<data>/state.json` so they survive restarts.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct PoolState {
    #[serde(default)]
    positions: std::collections::BTreeMap<String, usize>,
    #[serde(default)]
    last: std::collections::BTreeMap<String, String>,
}

/// A monitor as far as the wallpaper cares: name, position and size in
/// physical pixels (a DPI change changes the size).
type MonitorKey = (Option<String>, i32, i32, u32, u32);

fn monitor_layout(event_loop: &ActiveEventLoop) -> Vec<MonitorKey> {
    let mut v: Vec<MonitorKey> = event_loop
        .available_monitors()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            (m.name(), p.x, p.y, s.width, s.height)
        })
        .collect();
    v.sort();
    v
}

/// How far ahead of a timeline slot's start it is made.
const TIMELINE_LEAD: jiff::SignedDuration = jiff::SignedDuration::from_secs(4);

/// How long the monitor layout must stay still before the wallpaper follows.
const MONITOR_SETTLE: Duration = Duration::from_millis(1500);
/// Where no system message says the monitors changed, how often to look.
const MONITOR_POLL: Duration = Duration::from_secs(5);

enum AppEvent {
    Menu(tray_icon::menu::MenuEvent),
    Done(Done),
    /// A second launch asked for Settings.
    OpenSettings,
    /// The update check's answer.
    UpdateChecked(Result<update::Release, String>),
    /// The live metronome: a frame may be due. It arrives even while one
    /// of the app's windows is dragged or resized (Windows runs its own
    /// message loop then, and winit's timers stop; user events still come).
    LiveFrame,
}

struct App {
    settings: Settings,
    settings_path: PathBuf,
    settings_stamp: Option<SystemTime>,
    data_dir: PathBuf,
    jobs: mpsc::Sender<Job>,
    /// For threads that report back (the update check).
    proxy: EventLoopProxy<AppEvent>,
    /// What the last update check found, and whether one is running (and
    /// was asked for by hand: then its answer shows even when it's "up to
    /// date").
    update: update::UpdateState,
    checking_updates: Option<bool>,
    busy: bool,
    /// When the schedule next changes the wallpaper (None: only on demand).
    next_change: Option<Instant>,
    next_check: Instant,
    /// When the last usage line was logged, and the CPU time then.
    usage_at: Instant,
    usage_cpu: Duration,
    /// The theme the current wallpaper was rendered for.
    shown_theme: Option<Theme>,
    /// The theme the wallpaper on screen was made for: a theme change is a
    /// change from this, not from what a one-theme scene could show.
    asked_theme: Option<Theme>,
    /// The wallpaper on screen was picked by the user (Previous, the
    /// Archive, the Library, a timeline item): kept as picked.
    chosen: bool,
    /// The wallpaper on screen has a scrolling scene (a journey).
    shown_journey: bool,
    /// The primary monitor's image on screen, when it shows a whole scene
    /// (not a part of one spanning the monitors).
    shown_still: Option<PathBuf>,
    /// Whether scrolling scenes looked right at the last check.
    scrolling_ok: bool,
    /// The next re-apply gives this monitor (its id) a new scene, from this
    /// seed (New wallpaper on one monitor).
    renew: Option<(String, u64)>,
    /// The folder of what's on screen: an archive entry or a library item.
    current: Option<PathBuf>,
    /// A library item's own theme and Clawd choices, kept while it shows.
    current_theme: Option<Theme>,
    current_buddy: Option<bool>,
    /// What's on screen, for the Settings window.
    status: ui::view::Status,
    tray: Option<tray::Tray>,
    /// The live wallpaper (Live mode), playing what's on screen.
    live: live::Live,
    /// The live metronome's frame interval in microseconds (0: idle).
    metronome: Arc<AtomicU64>,
    /// Hears display changes and Explorer restarts (Windows).
    #[cfg(windows)]
    watcher: Option<clairlune_platform::windows_live::SystemWatcher>,
    /// Hears Space changes: a picture is set on the current Space only (macOS).
    #[cfg(target_os = "macos")]
    spaces: clairlune_platform::macos::SpaceWatcher,
    /// The monitor layout the wallpaper was made for, when a change was
    /// noticed (it settles before anything is redone), and when to look for
    /// changes where nothing tells us (not Windows).
    monitor_layout: Option<Vec<MonitorKey>>,
    monitors_changed_at: Option<Instant>,
    next_monitor_poll: Instant,
    /// The daily timeline (Timeline rotation mode), its file and state.
    timeline: timeline::Timeline,
    timeline_path: PathBuf,
    timeline_stamp: Option<SystemTime>,
    /// The slot start applied last, and when the next one starts.
    timeline_since: Option<jiff::Timestamp>,
    /// What that slot shows (`None` for an empty day's Generate schedule),
    /// so an edit elsewhere in the timeline doesn't change the wallpaper.
    timeline_applied: Option<timeline::Slot>,
    timeline_next: Option<jiff::Timestamp>,
    /// The timeline's day is empty: the Generate schedule runs.
    timeline_generate: bool,
    /// "New wallpaper" during a slot shows a random scene until the next one.
    manual_until: Option<jiff::Timestamp>,
    /// The next job is made now and shown at this instant (a slot prepared
    /// ahead of its start).
    apply_at: Option<Instant>,
    pools: PoolState,
    /// The Settings window, while open, and the section it last showed.
    settings_window: Option<ui::UiWindow>,
    /// The Library window, while open, and the items it lists.
    library_window: Option<ui::UiWindow>,
    /// The Timeline window, while open.
    timeline_window: Option<ui::UiWindow>,
    /// The Archive window, while open, and the entries it lists.
    archive_window: Option<ui::UiWindow>,
    archive_items: Vec<archive::Entry>,
    library_items: Vec<library::Item>,
    /// Where the timeline uses each library item.
    library_uses: std::collections::BTreeMap<String, Vec<String>>,
    last_section: ui::view::Section,
    /// `--ui-test`: scripted open/close cycles with screenshots, then exit.
    ui_test: Option<UiTest>,
    /// Started quietly (at sign-in): no window at launch.
    background: bool,
    /// The scene on screen, for the screen saver; and whether the saver
    /// was on when last looked at (turning it on opens System Settings).
    saver_scene: Option<clairlune_scene::saver::SaverScene>,
    saver_on: bool,
    /// The taskbar's colour when last looked at (the tray mark follows it).
    taskbar_light: Option<bool>,
    /// The monitors the tray's "New wallpaper on" lists, for a tray made
    /// again after being hidden.
    tray_monitors: Vec<clairlune_platform::Monitor>,
}

/// The M6 toolkit spike's pass criterion, scripted: open and close Settings
/// three times on the one event loop (alongside the tray), screenshot each
/// opening in a different theme and section, then quit.
struct UiTest {
    out: PathBuf,
    steps: Vec<(Duration, UiStep)>,
    started: Instant,
    next: usize,
}

#[derive(Clone, Copy)]
enum UiStep {
    Open(Theme, ui::view::Section),
    /// Settings with the first run's question over it.
    OpenAsking(Theme),
    /// The tray icon hidden or shown again, as the setting does it.
    Tray(bool),
    OpenLibrary,
    /// The Timeline window in a theme, with a slot (day, index) selected.
    OpenTimeline(Theme, usize, usize),
    CloseTimeline,
    OpenArchive,
    CloseArchive,
    /// Of the Library window when it's open, else of Settings.
    Screenshot(&'static str),
    Close,
    CloseLibrary,
    Exit,
}

impl UiTest {
    fn new(out: PathBuf) -> Self {
        use ui::view::Section;
        let ms = Duration::from_millis;
        let mut steps = Vec::new();
        let cycles = [
            (Theme::Dark, Section::Rotation, "settings-dark-rotation.png"),
            (Theme::Light, Section::Buddy, "settings-light-buddy.png"),
            (Theme::Dark, Section::Live, "settings-dark-live.png"),
            (Theme::Dark, Section::Content, "settings-dark-content.png"),
        ];
        for (i, (theme, section, shot)) in cycles.into_iter().enumerate() {
            let t0 = 1500 * i as u64;
            steps.push((ms(t0), UiStep::Open(theme, section)));
            steps.push((ms(t0 + 700), UiStep::Screenshot(shot)));
            steps.push((ms(t0 + 1200), UiStep::Close));
        }
        steps.push((ms(6300), UiStep::OpenLibrary));
        steps.push((ms(7100), UiStep::Screenshot("library.png")));
        steps.push((ms(7600), UiStep::CloseLibrary));
        let timelines = [
            (Theme::Dark, 0, 1, "timeline-dark.png"),
            (Theme::Light, 0, 4, "timeline-light.png"),
        ];
        for (i, (theme, day, index, shot)) in timelines.into_iter().enumerate() {
            let t0 = 8100 + 1700 * i as u64;
            steps.push((ms(t0), UiStep::OpenTimeline(theme, day, index)));
            steps.push((ms(t0 + 900), UiStep::Screenshot(shot)));
            steps.push((ms(t0 + 1400), UiStep::CloseTimeline));
        }
        steps.push((ms(11600), UiStep::OpenArchive));
        steps.push((ms(12400), UiStep::Screenshot("archive.png")));
        steps.push((ms(12900), UiStep::CloseArchive));
        // The theme schedule, following the sun (seeded in settings.toml).
        steps.push((ms(13400), UiStep::Open(Theme::Light, Section::Theme)));
        steps.push((ms(14100), UiStep::Screenshot("settings-light-theme.png")));
        steps.push((ms(14600), UiStep::Close));
        // General (start at login, the tray icon, quit), then the first
        // run's question.
        steps.push((ms(15100), UiStep::Open(Theme::Dark, Section::General)));
        steps.push((ms(15800), UiStep::Screenshot("settings-dark-general.png")));
        steps.push((ms(16300), UiStep::Close));
        steps.push((ms(16800), UiStep::OpenAsking(Theme::Light)));
        steps.push((ms(17600), UiStep::Screenshot("first-run-question.png")));
        steps.push((ms(18100), UiStep::Close));
        steps.push((ms(18400), UiStep::Tray(false)));
        steps.push((ms(18700), UiStep::Tray(true)));
        steps.push((ms(19000), UiStep::Exit));
        UiTest {
            out,
            steps,
            started: Instant::now(),
            next: 0,
        }
    }

    fn due(&mut self) -> Option<UiStep> {
        let (at, step) = *self.steps.get(self.next)?;
        (self.started.elapsed() >= at).then(|| {
            self.next += 1;
            step
        })
    }

    fn wake(&self) -> Option<Instant> {
        self.steps.get(self.next).map(|(at, _)| self.started + *at)
    }
}

fn capitalise_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn random_seed() -> u64 {
    std::collections::hash_map::RandomState::new().hash_one(0x5EED_u64)
}

fn file_stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Opens a folder in the file manager, or a text file in a text editor.
fn open(path: &Path, text: bool) {
    #[cfg(windows)]
    let mut cmd = std::process::Command::new(if text { "notepad.exe" } else { "explorer.exe" });
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        if text {
            c.arg("-t");
        }
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let _ = text;
        std::process::Command::new("xdg-open")
    };
    if let Err(e) = cmd.arg(path).spawn() {
        tracing::warn!(target: "clairlune", path = %path.display(), error = %e, "could not open");
    }
}

/// Opens a web page in the default browser.
fn open_url(url: &str) {
    #[cfg(windows)]
    let mut cmd = {
        // `start` is a cmd builtin; the empty title keeps the URL as the target.
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    if let Err(e) = cmd.arg(url).spawn() {
        tracing::warn!(target: "clairlune", url, error = %e, "could not open");
    }
}

impl App {
    fn timeline_mode(&self) -> bool {
        self.settings.rotation.mode == settings::RotationMode::Timeline && !self.timeline.is_empty()
    }

    fn interval(&self) -> Option<Duration> {
        // In Timeline mode the slots decide, except on an empty day.
        if self.timeline_mode() && !self.timeline_generate {
            return None;
        }
        let m = self.settings.schedule.change_every_minutes;
        (m > 0 && !self.settings.schedule.paused).then(|| Duration::from_secs(u64::from(m) * 60))
    }

    fn reschedule(&mut self) {
        self.next_change = self.interval().map(|d| Instant::now() + d);
    }

    fn submit(&mut self, job: Job) {
        let export = matches!(job, Job::Export { .. } | Job::ExportGif { .. });
        // Restore waits its turn behind a wallpaper being made, rather than
        // being dropped: the user asked for their own wallpaper back.
        let restore = matches!(job, Job::Restore);
        if self.busy && !export && !restore {
            return;
        }
        if !export {
            self.busy = true;
            if let Some(t) = &self.tray {
                t.set_busy(true);
            }
        }
        if self.jobs.send(job).is_err() {
            tracing::error!(target: "clairlune", "worker thread is gone");
        }
    }

    /// Keeps the metronome at the live frame rate while live plays.
    fn set_metronome(&self, playing: bool) {
        let us = if playing {
            self.live.frame_interval().as_micros() as u64
        } else {
            0
        };
        self.metronome.store(us, Ordering::Relaxed);
    }

    /// Marks (or clears) that the user's own wallpaper is on screen after
    /// Restore: nothing changes it on its own until Clairlune applies one
    /// by request or the schedule resumes. Kept across restarts.
    fn set_restored(&self, on: bool) {
        let path = self.data_dir.join("restored");
        let _ = if on {
            std::fs::write(&path, b"")
        } else {
            std::fs::remove_file(&path)
        };
    }

    fn restored(&self) -> bool {
        self.data_dir.join("restored").exists()
    }

    fn new_wallpaper(&mut self) {
        self.chosen = false;
        self.current_theme = None;
        self.current_buddy = None;
        let seed = random_seed();
        let at = self.apply_at.take();
        let scrolling = self.live.scrolling_ok();
        self.submit(Job::New {
            seed,
            settings: Box::new(self.settings.clone()),
            at,
            scrolling,
        });
    }

    fn archive(&self) -> Option<archive::Archive> {
        archive::Archive::open(self.data_dir.join("archive")).ok()
    }

    /// Re-applies saved scenes: the previous archive entry, or what's on
    /// screen again (for a theme change).
    fn reapply(&mut self, previous: bool) {
        let Some(archive) = self.archive() else {
            return;
        };
        let (dir, theme, buddy) = if previous {
            // The archive entry before the current one; from a library item,
            // the newest archive entry.
            let current_name = self
                .current
                .as_ref()
                .filter(|p| p.starts_with(archive.dir()))
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned());
            let entry = match current_name {
                Some(n) => archive.before(&n),
                None => archive.latest(),
            };
            let Some(entry) = entry else {
                return;
            };
            self.current_theme = None;
            self.current_buddy = None;
            (archive.dir().join(entry.name), None, None)
        } else {
            let Some(dir) = self.current.clone() else {
                return;
            };
            (dir, self.current_theme, self.current_buddy)
        };
        if previous {
            self.chosen = true;
        }
        // Going back shows the older entry as it is (archiving it again
        // would make the next Previous come back here).
        let refresh = !previous && !self.chosen;
        self.apply_folder(dir, theme, buddy, false, refresh);
    }

    /// A new scene on one monitor (its id); the others keep theirs. It is
    /// a new wallpaper, archived, so Previous goes back.
    fn new_wallpaper_on(&mut self, monitor: String) {
        let Some(dir) = self.current.clone() else {
            return self.new_wallpaper_by_hand();
        };
        self.current_theme = None;
        self.current_buddy = None;
        self.chosen = false;
        self.renew = Some((monitor, random_seed()));
        // Archived only if a monitor really got a new scene (it may have
        // been unplugged since the menu was made).
        self.apply_folder(dir, None, None, false, false);
    }

    /// Shows the scenes saved in a folder (archive entry or library item).
    fn apply_folder(
        &mut self,
        dir: PathBuf,
        theme: Option<Theme>,
        buddy: Option<bool>,
        archive: bool,
        refresh: bool,
    ) {
        let recipes = std::fs::read_to_string(dir.join("recipe.json"))
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()));
        let at = self.apply_at.take();
        let renew = self.renew.take();
        let scrolling = self.live.scrolling_ok();
        match recipes {
            Ok(recipes) => self.submit(Job::Recipes {
                at,
                recipes: Box::new(recipes),
                source: Some(dir),
                theme,
                buddy,
                settings: Box::new(self.settings.clone()),
                archive,
                renew,
                scrolling,
                refresh,
            }),
            Err(e) => {
                tracing::warn!(target: "clairlune", dir = %dir.display(), error = %e, "unreadable saved scene")
            }
        }
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            tracing::warn!(target: "clairlune", error = %e, "could not save settings");
        }
        self.settings_stamp = file_stamp(&self.settings_path);
    }

    fn save_pools(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.pools) {
            let _ = std::fs::write(self.data_dir.join("state.json"), json);
        }
    }

    /// Applies the timeline's active slot when it changed (PLAN.md 9b).
    fn check_timeline(&mut self) {
        if !self.timeline_mode() {
            if self.timeline_since.take().is_some() {
                self.timeline_generate = false;
                self.reschedule();
            }
            return;
        }
        let now = jiff::Timestamp::now();
        let tz = jiff::tz::TimeZone::system();
        let Some(active) = timeline::resolve(&self.timeline, &tz, now) else {
            return;
        };
        self.timeline_next = Some(active.next);
        if self.ui_test.is_some() {
            return; // the UI test leaves the wallpaper alone
        }
        // Paused: the wallpaper stays until you resume.
        if self.settings.schedule.paused {
            return;
        }
        // Shortly before the next slot starts, make it now and show it at
        // its start (PLAN.md section 9b, pre-rendering).
        if !self.busy && self.timeline_since != Some(active.next) {
            let ahead = active.next.duration_since(now);
            if ahead > jiff::SignedDuration::ZERO
                && ahead <= TIMELINE_LEAD
                && let Some(up) = timeline::resolve(&self.timeline, &tz, active.next)
                && up.since == active.next
                && let Ok(wait) = std::time::Duration::try_from(ahead)
            {
                tracing::info!(target: "clairlune", seconds = wait.as_secs_f32(), "timeline: preparing the next slot");
                self.manual_until = None;
                self.timeline_since = Some(up.since);
                self.apply_at = Some(Instant::now() + wait);
                self.apply_pick(up.pick, up.since);
                self.apply_at = None;
                return;
            }
        }
        // The next slot is already on its way.
        if self.timeline_since.is_some_and(|s| s > now) {
            return;
        }
        if let Some(until) = self.manual_until {
            if now < until {
                return;
            }
            self.manual_until = None;
        }
        if self.timeline_since == Some(active.since) || self.busy {
            return;
        }
        self.timeline_since = Some(active.since);
        self.apply_pick(active.pick, active.since);
    }

    /// Shows what the timeline picked: a slot, or new scenes on an empty day.
    fn apply_pick(&mut self, pick: timeline::Pick, since: jiff::Timestamp) {
        match pick {
            timeline::Pick::Generate => {
                tracing::info!(target: "clairlune", "timeline: empty day, generating");
                self.timeline_applied = None;
                self.timeline_generate = true;
                self.new_wallpaper();
                self.reschedule();
            }
            timeline::Pick::Slot { weekday, index } => {
                self.timeline_applied = self.timeline.day(weekday).get(index).cloned();
                self.timeline_generate = false;
                self.next_change = None;
                self.apply_slot(weekday, index, since.as_second() as u64);
            }
        }
    }

    /// Shows one timeline slot.
    fn apply_slot(&mut self, weekday: jiff::civil::Weekday, index: usize, seed: u64) {
        let Some(slot) = self.timeline.day(weekday).get(index).cloned() else {
            return;
        };
        // Pools keep their place per slot: per weekday slot with one
        // plan per weekday, per slot of the one plan for every day.
        let key = if self.timeline.day_count() == 1 {
            format!("every-day-{index}")
        } else {
            format!(
                "{}-{index}",
                timeline::day_name(timeline::weekday_index(weekday))
            )
        };
        tracing::info!(target: "clairlune", slot = %key, start = %timeline::format_time(slot.start), "timeline slot");
        match slot.content {
            timeline::Content::Item { id } => self.show_library_item(&id, slot.theme, slot.buddy),
            timeline::Content::Pool { ids, tag, order } => {
                let items = self.library().map(|l| l.items()).unwrap_or_default();
                let mut candidates: Vec<String> = ids
                    .into_iter()
                    .filter(|id| items.iter().any(|i| &i.id == id))
                    .collect();
                if let Some(tag) = tag {
                    for i in items.iter().filter(|i| i.tags.contains(&tag)) {
                        if !candidates.contains(&i.id) {
                            candidates.push(i.id.clone());
                        }
                    }
                }
                let last = self.pools.last.get(&key).cloned();
                let position = self.pools.positions.entry(key.clone()).or_default();
                match timeline::pick_from_pool(&candidates, order, seed, last.as_deref(), position)
                {
                    Some(id) => {
                        self.pools.last.insert(key, id.clone());
                        self.save_pools();
                        self.show_library_item(&id, slot.theme, slot.buddy);
                    }
                    None => {
                        tracing::warn!(target: "clairlune", "timeline pool is empty; generating");
                        self.new_wallpaper();
                    }
                }
            }
            timeline::Content::Random {
                archetypes,
                activities,
                weather,
            } => {
                let mut s = self.settings.clone();
                if !archetypes.is_empty() {
                    s.content.archetypes = archetypes;
                }
                if !activities.is_empty() {
                    s.content.activities = activities;
                }
                if !weather.is_empty() {
                    s.content.weather = weather;
                }
                match slot.theme {
                    library::ThemeChoice::Light => s.theme.mode = settings::ThemeMode::Light,
                    library::ThemeChoice::Dark => s.theme.mode = settings::ThemeMode::Dark,
                    library::ThemeChoice::Follow => {}
                }
                match slot.buddy {
                    library::BuddyChoice::On => s.buddy.show = true,
                    library::BuddyChoice::Off => s.buddy.show = false,
                    library::BuddyChoice::Follow => {}
                }
                // The slot's own choices hold on screen: a theme change
                // doesn't re-render them away.
                self.current_theme = match slot.theme {
                    library::ThemeChoice::Light => Some(Theme::Light),
                    library::ThemeChoice::Dark => Some(Theme::Dark),
                    library::ThemeChoice::Follow => None,
                };
                self.current_buddy = match slot.buddy {
                    library::BuddyChoice::On => Some(true),
                    library::BuddyChoice::Off => Some(false),
                    library::BuddyChoice::Follow => None,
                };
                self.chosen = false;
                let at = self.apply_at.take();
                let scrolling = self.live.scrolling_ok();
                self.submit(Job::New {
                    seed: random_seed(),
                    settings: Box::new(s),
                    at,
                    scrolling,
                });
            }
        }
    }

    /// Shows a library item; a slot's choices win over the item's, which win
    /// over the global settings.
    fn show_library_item(
        &mut self,
        id: &str,
        slot_theme: library::ThemeChoice,
        slot_buddy: library::BuddyChoice,
    ) {
        let Some(lib) = self.library() else {
            return;
        };
        let Some(mut item) = lib.get(id) else {
            tracing::warn!(target: "clairlune", id, "timeline item is gone; generating");
            self.new_wallpaper();
            return;
        };
        if slot_theme != library::ThemeChoice::Follow {
            item.theme = slot_theme;
        }
        if slot_buddy != library::BuddyChoice::Follow {
            item.buddy = slot_buddy;
        }
        let (theme, buddy) = Self::library_choices(&item, &self.settings);
        self.current_theme = theme;
        self.current_buddy = buddy;
        self.chosen = true;
        self.apply_folder(lib.item_dir(id), theme, buddy, false, false);
    }

    /// After the timeline changed (in the editor or in the file): the active
    /// slot is shown afresh only if what it shows changed, so moving another
    /// slot doesn't replace the wallpaper.
    fn timeline_replanned(&mut self) {
        let active = timeline::resolve(
            &self.timeline,
            &jiff::tz::TimeZone::system(),
            jiff::Timestamp::now(),
        );
        let Some(active) = active else {
            self.timeline_since = None;
            return;
        };
        let same = match (&active.pick, &self.timeline_applied) {
            (timeline::Pick::Slot { weekday, index }, Some(applied)) => self
                .timeline
                .day(*weekday)
                .get(*index)
                .is_some_and(|s| s.shows_same(applied)),
            (timeline::Pick::Generate, None) => self.timeline_generate,
            _ => false,
        };
        if same && self.timeline_since.is_some() {
            self.timeline_since = Some(active.since);
            self.timeline_next = Some(active.next);
        } else {
            self.timeline_since = None;
        }
    }

    /// The Timeline window edited the timeline: save it and follow it.
    fn timeline_edited(&mut self) {
        if let Err(e) = self.timeline.save(&self.timeline_path) {
            tracing::warn!(target: "clairlune", error = %e, "could not save the timeline");
            self.status.note = Some(format!("Couldn't save the timeline: {e}"));
        }
        self.timeline_stamp = file_stamp(&self.timeline_path);
        self.timeline_replanned();
        self.check_timeline();
        self.refresh_library();
    }

    fn open_timeline(&mut self, event_loop: &ActiveEventLoop) {
        self.refresh_library();
        if self.timeline_window.is_some() {
            return;
        }
        let theme = apply::resolve_theme(&self.settings.theme);
        match ui::UiWindow::open(event_loop, ui::Kind::Timeline, theme, self.last_section) {
            Ok(w) => self.timeline_window = Some(w),
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "no timeline window; opening the file");
                if !self.timeline_path.exists() {
                    let _ = self.timeline.save(&self.timeline_path);
                }
                open(&self.timeline_path, true);
            }
        }
    }

    /// "New wallpaper" by hand: in Timeline mode it shows until the next slot.
    fn new_wallpaper_by_hand(&mut self) {
        if self.timeline_mode() && !self.timeline_generate {
            self.manual_until = self.timeline_next;
        }
        self.new_wallpaper();
        self.reschedule();
    }

    /// Starts an update check on a thread; its answer comes back as an
    /// event. `by_hand`: asked for in Settings.
    fn check_updates(&mut self, by_hand: bool) {
        if self.checking_updates.is_some() || self.ui_test.is_some() {
            return;
        }
        self.checking_updates = Some(by_hand);
        let proxy = self.proxy.clone();
        let started = std::thread::Builder::new()
            .name("clairlune-update".into())
            .spawn(move || {
                let _ = proxy.send_event(AppEvent::UpdateChecked(update::fetch()));
            });
        if started.is_err() {
            self.checking_updates = None;
        }
    }

    fn on_update_checked(&mut self, result: Result<update::Release, String>) {
        let by_hand = self.checking_updates.take().unwrap_or(false);
        let now = timefmt::now_unix();
        match result {
            Ok(r) => {
                tracing::info!(target: "clairlune", latest = %r.version, "update check");
                self.update = update::UpdateState {
                    checked: now,
                    latest: Some(r.version),
                    url: Some(r.url),
                };
                self.status.update_note = Some(match self.update.newer() {
                    Some(v) => format!("Clairlune {v} is out"),
                    None => format!("Clairlune {} is up to date", env!("CARGO_PKG_VERSION")),
                });
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "update check failed");
                // Tried: the next try is in a few hours, not at once.
                self.update.checked = now.saturating_sub(update::EVERY_SECS - 3 * 3600);
                if by_hand {
                    self.status.update_note = Some(format!("Couldn't check: {e}"));
                }
            }
        }
        self.update.save(&self.data_dir);
        if let Some(t) = &self.tray {
            t.show_update(self.update.newer());
        }
        if let Some(w) = &self.settings_window {
            w.request_redraw();
        }
    }

    /// Picks up edits to settings.toml and system theme changes.
    fn check(&mut self) {
        if self.settings.general.check_updates && self.update.due(timefmt::now_unix()) {
            self.check_updates(false);
        }
        let until = self.settings.general.detailed_logging_until;
        if until != 0 && until <= timefmt::now_unix() {
            self.settings.general.detailed_logging_until = 0;
            diagnostics::set_detailed(false);
            tracing::info!(target: "clairlune", "detailed logging ended");
            self.save_settings();
        }
        let tl_stamp = file_stamp(&self.timeline_path);
        if tl_stamp != self.timeline_stamp {
            self.timeline_stamp = tl_stamp;
            match timeline::Timeline::load(&self.timeline_path) {
                Ok(t) => {
                    if t != self.timeline {
                        tracing::info!(target: "clairlune", "timeline changed");
                        self.timeline = t;
                        self.timeline_replanned();
                        self.refresh_library();
                    }
                }
                Err(e) => {
                    tracing::warn!(target: "clairlune", error = %e, "timeline.toml did not load");
                    self.status.note = Some(format!("The timeline file has an error: {e}"));
                }
            }
        }
        // The Timeline window's "now" marker moves with the clock.
        if let Some(w) = &self.timeline_window {
            w.request_redraw();
        }
        let stamp = file_stamp(&self.settings_path);
        if stamp != self.settings_stamp {
            self.settings_stamp = stamp;
            match Settings::reload(&self.settings_path) {
                Ok(new) if new != self.settings => {
                    tracing::info!(target: "clairlune", "settings changed on disk");
                    let schedule_changed = new.schedule != self.settings.schedule;
                    let display_changed =
                        new.display != self.settings.display || new.live != self.settings.live;
                    self.settings = new;
                    if let Some(t) = &self.tray {
                        t.show_theme(self.settings.theme.mode);
                        t.show_paused(self.settings.schedule.paused);
                    }
                    if display_changed {
                        self.follow_display_settings();
                    }
                    self.sync_tray();
                    self.sync_saver(false);
                    if schedule_changed {
                        self.reschedule();
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(target: "clairlune", error = %e, "settings.toml did not parse; keeping the current settings");
                    self.status.note = Some(format!(
                        "settings.toml has an error, so the current settings stay: {e}"
                    ));
                }
            }
        }
        // Live went to a frame rate too low to scroll smoothly (Battery
        // saver): a journey on screen gives way.
        let scrolling = self.live.scrolling_ok();
        // (Noted only when not busy: a wallpaper being made is checked
        // once it's up.)
        if scrolling != self.scrolling_ok && !self.busy {
            self.scrolling_ok = scrolling;
            if !scrolling && self.shown_journey && self.current_theme.is_none() && !self.chosen {
                tracing::info!(target: "clairlune", "live frame rate too low to scroll; replacing the journey");
                self.reapply(false);
                return;
            }
        }
        // The tray mark follows the taskbar's colour (Windows).
        let light = clairlune_platform::taskbar_is_light();
        if light != self.taskbar_light {
            self.taskbar_light = light;
            if let Some(t) = &self.tray {
                t.refresh_mark();
            }
        }
        let theme = apply::resolve_theme(&self.settings.theme);
        if self.current_theme.is_none()
            && self.asked_theme.is_some_and(|t| t != theme)
            && !self.busy
        {
            tracing::info!(target: "clairlune", theme = theme.as_str(), "theme changed; re-rendering");
            self.reapply(false);
        }
        self.check_timeline();
    }

    fn on_action(&mut self, action: tray::Action, event_loop: &ActiveEventLoop) {
        use tray::Action;
        match action {
            Action::NewWallpaper => self.new_wallpaper_by_hand(),
            Action::NewWallpaperOn(i) => {
                if let Some(id) = self.tray.as_ref().and_then(|t| t.monitor_id(i)) {
                    self.new_wallpaper_on(id);
                }
            }
            Action::Previous => self.reapply(true),
            Action::Theme(mode) => {
                self.settings.theme.mode = mode;
                if let Some(t) = &self.tray {
                    t.show_theme(mode);
                }
                self.save_settings();
                self.check();
            }
            Action::ToggleLive => {
                self.settings.display.mode = match self.settings.display.mode {
                    settings::WallpaperMode::Live => settings::WallpaperMode::Still,
                    settings::WallpaperMode::Still => settings::WallpaperMode::Live,
                };
                self.save_settings();
                self.follow_display_settings();
                self.redraw_windows();
            }
            Action::TogglePause => {
                self.settings.schedule.paused = !self.settings.schedule.paused;
                if let Some(t) = &self.tray {
                    t.show_paused(self.settings.schedule.paused);
                }
                self.save_settings();
                self.reschedule();
            }
            Action::SavePng => {
                self.export_png();
            }
            Action::SaveGif => self.export_gif(),
            Action::SaveToLibrary => self.save_to_library(),
            Action::OpenLibrary => self.open_library(event_loop),
            Action::OpenTimeline => self.open_timeline(event_loop),
            Action::OpenArchive => self.open_archive(event_loop),
            Action::OpenSettings => self.open_settings(event_loop, None),
            Action::OpenUpdate => self.open_update(),
            Action::Quit => event_loop.exit(),
        }
    }

    /// Opens Settings (or brings it forward). Falls back to the settings file
    /// in a text editor if no window can be made (no GPU, for example).
    fn open_settings(&mut self, event_loop: &ActiveEventLoop, theme: Option<Theme>) {
        if let Some(w) = &self.settings_window {
            w.request_redraw();
            return;
        }
        let theme = theme.unwrap_or_else(|| apply::resolve_theme(&self.settings.theme));
        self.status.start_at_login = std::env::current_exe()
            .ok()
            .and_then(|exe| clairlune_platform::start_at_login(&exe));
        self.status.saver_available = saver::shipped().is_some();
        self.status.ask_start_at_login = !self.settings.general.asked_start_at_login
            && self.status.start_at_login == Some(false)
            && self.ui_test.is_none();
        self.status.note = None;
        match ui::UiWindow::open(event_loop, ui::Kind::Settings, theme, self.last_section) {
            Ok(w) => {
                self.settings_window = Some(w);
                self.refresh_window_thumb();
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "no settings window; opening the file");
                if !self.settings_path.exists() {
                    self.save_settings();
                }
                open(&self.settings_path, true);
            }
        }
    }

    /// The library folder.
    fn library(&self) -> Option<library::Library> {
        library::Library::open(self.data_dir.join("library")).ok()
    }

    fn refresh_library(&mut self) {
        self.library_items = self.library().map(|l| l.items()).unwrap_or_default();
        self.library_uses = self
            .library_items
            .iter()
            .map(|i| (i.id.clone(), self.timeline.uses(&i.id)))
            .filter(|(_, u)| !u.is_empty())
            .collect();
        self.redraw_windows();
    }

    fn redraw_windows(&self) {
        for w in [
            &self.settings_window,
            &self.library_window,
            &self.timeline_window,
            &self.archive_window,
        ]
        .into_iter()
        .flatten()
        {
            w.request_redraw();
        }
    }

    fn open_library(&mut self, event_loop: &ActiveEventLoop) {
        self.refresh_library();
        if self.library_window.is_some() {
            return;
        }
        let theme = apply::resolve_theme(&self.settings.theme);
        match ui::UiWindow::open(event_loop, ui::Kind::Library, theme, self.last_section) {
            Ok(w) => self.library_window = Some(w),
            Err(e) => tracing::warn!(target: "clairlune", error = %e, "no library window"),
        }
    }

    /// Copies the archive entry on screen into the library.
    fn save_to_library(&mut self) {
        let dir = self.current.clone();
        self.save_dir_to_library(dir);
    }

    /// Copies an archive entry into the library.
    fn save_dir_to_library(&mut self, dir: Option<PathBuf>) {
        let note = (|| {
            let dir = dir.ok_or("There's no wallpaper to save yet.")?;
            let lib = self
                .library()
                .ok_or("The library folder can't be opened.")?;
            if dir.starts_with(lib.dir()) {
                return Err("This wallpaper is already in the library.");
            }
            let archive = self.archive().ok_or("The archive can't be opened.")?;
            let name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
            let entry = name
                .and_then(|n| archive.get(&n))
                .ok_or("This wallpaper is no longer in the archive.")?;
            lib.save_entry(&dir, &entry, timefmt::now_unix())
                .map(|item| item.name)
                .map_err(|_| "Saving to the library failed; see the logs.")
        })();
        self.status.note = Some(match note {
            Ok(name) => format!("Saved to the library as {name}."),
            Err(e) => e.to_string(),
        });
        self.refresh_library();
    }

    fn refresh_archive(&mut self) {
        self.archive_items = self.archive().map(|a| a.entries()).unwrap_or_default();
        if let Some(w) = &self.archive_window {
            w.request_redraw();
        }
    }

    fn open_archive(&mut self, event_loop: &ActiveEventLoop) {
        self.refresh_archive();
        if self.archive_window.is_some() {
            return;
        }
        let theme = apply::resolve_theme(&self.settings.theme);
        match ui::UiWindow::open(event_loop, ui::Kind::Archive, theme, self.last_section) {
            Ok(w) => self.archive_window = Some(w),
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "no archive window; opening the folder");
                open(&self.data_dir.join("archive"), false);
            }
        }
    }

    /// The Archive window's requests.
    fn on_archive_action(&mut self, action: ui::view::UiAction) {
        use ui::view::UiAction;
        let Some(archive) = self.archive() else {
            return;
        };
        let dir_of = |name: &str| archive.dir().join(name);
        match action {
            UiAction::ApplyArchive(name) => {
                self.current_theme = None;
                self.current_buddy = None;
                self.chosen = true;
                self.apply_folder(dir_of(&name), None, None, false, false);
            }
            UiAction::PinArchive { name, pinned } => {
                if let Err(e) = archive.set_pinned(&name, pinned) {
                    self.status.note = Some(format!("Couldn't pin it: {e}"));
                }
            }
            UiAction::SaveArchiveToLibrary(name) => {
                self.save_dir_to_library(Some(dir_of(&name)));
            }
            UiAction::ExportArchive { name, gif } => {
                let dest = self.export_dir();
                let source = Some(dir_of(&name));
                if gif {
                    self.submit(Job::ExportGif {
                        dest,
                        source,
                        theme: apply::resolve_theme(&self.settings.theme),
                        buddy: self.settings.buddy.show,
                    });
                } else {
                    self.submit(Job::Export {
                        dest,
                        source,
                        still: None,
                    });
                }
            }
            UiAction::DeleteArchive(name) => {
                let dir = dir_of(&name);
                if self.current.as_deref() == Some(dir.as_path()) {
                    self.current = None;
                }
                self.status.note = Some(match archive.delete(&name) {
                    Ok(()) => "Deleted from the archive.".to_string(),
                    Err(e) => format!("Couldn't delete it: {e}"),
                });
            }
            _ => {}
        }
        self.refresh_archive();
    }

    fn library_choices(item: &library::Item, settings: &Settings) -> (Option<Theme>, Option<bool>) {
        let theme = match item.theme {
            library::ThemeChoice::Follow => None,
            library::ThemeChoice::Light => Some(Theme::Light),
            library::ThemeChoice::Dark => Some(Theme::Dark),
        };
        let buddy = match item.buddy {
            library::BuddyChoice::Follow => None,
            library::BuddyChoice::On => Some(true),
            library::BuddyChoice::Off => Some(false),
        };
        let _ = settings;
        (theme, buddy)
    }

    fn on_library_action(&mut self, action: ui::view::UiAction) {
        use ui::view::UiAction;
        let Some(lib) = self.library() else {
            return;
        };
        match action {
            UiAction::ApplyLibrary(id) => {
                if let Some(item) = lib.get(&id) {
                    let (theme, buddy) = Self::library_choices(&item, &self.settings);
                    self.current_theme = theme;
                    self.current_buddy = buddy;
                    self.chosen = true;
                    self.apply_folder(lib.item_dir(&id), theme, buddy, false, false);
                }
            }
            UiAction::UpdateLibrary(item) => {
                if let Err(e) = lib.update(&item) {
                    self.status.note = Some(format!("Couldn't save the change: {e}"));
                }
                // Showing this item: apply its new choices right away.
                if self.current.as_deref() == Some(lib.item_dir(&item.id).as_path()) {
                    let (theme, buddy) = Self::library_choices(&item, &self.settings);
                    self.current_theme = theme;
                    self.current_buddy = buddy;
                    self.reapply(false);
                }
            }
            UiAction::DeleteLibrary { id, forget } => {
                if self.current.as_deref() == Some(lib.item_dir(&id).as_path()) {
                    self.current = None;
                }
                if !self.timeline.uses(&id).is_empty() {
                    self.timeline.forget_item(&id, forget);
                    if let Err(e) = self.timeline.save(&self.timeline_path) {
                        tracing::warn!(target: "clairlune", error = %e, "could not save the timeline");
                    }
                    self.timeline_stamp = file_stamp(&self.timeline_path);
                    self.timeline_replanned();
                }
                self.status.note = Some(match lib.delete(&id) {
                    Ok(()) => "Deleted from the library.".to_string(),
                    Err(e) => format!("Couldn't delete it: {e}"),
                });
            }
            UiAction::ExportLibrary { id, gif } => {
                let dest = self.export_dir();
                let source = Some(lib.item_dir(&id));
                if gif {
                    let item = lib.get(&id);
                    let (theme, buddy) = item
                        .map(|i| Self::library_choices(&i, &self.settings))
                        .unwrap_or((None, None));
                    self.submit(Job::ExportGif {
                        dest,
                        source,
                        theme: theme.unwrap_or_else(|| apply::resolve_theme(&self.settings.theme)),
                        buddy: buddy.unwrap_or(self.settings.buddy.show),
                    });
                } else {
                    self.submit(Job::Export {
                        dest,
                        source,
                        still: None,
                    });
                }
            }
            _ => {}
        }
        self.refresh_library();
    }

    /// Loads the current wallpaper's thumbnail into the open Settings window.
    fn refresh_window_thumb(&mut self) {
        let (Some(w), Some(dir)) = (&mut self.settings_window, &self.current) else {
            return;
        };
        let path = dir.join("thumb.png");
        if let Some((size, rgba)) = std::fs::read(&path)
            .ok()
            .and_then(|b| ui::decode_png_rgba(&b))
        {
            w.set_thumb(size, &rgba);
        }
    }

    /// When to prepare the next timeline slot, if one is coming and isn't
    /// prepared yet.
    fn timeline_prepare_at(&self) -> Option<Instant> {
        if !self.timeline_mode() || self.ui_test.is_some() {
            return None;
        }
        let next = self.timeline_next?;
        if self.timeline_since == Some(next) {
            return None;
        }
        let ahead = next.duration_since(jiff::Timestamp::now()) - TIMELINE_LEAD;
        let wait = std::time::Duration::try_from(ahead).unwrap_or(Duration::ZERO);
        Some(Instant::now() + wait)
    }

    /// Notices monitor changes (system messages on Windows, a poll elsewhere)
    /// and, once the layout has settled and differs from the one the
    /// wallpaper was made for, makes the current scenes again at the new
    /// sizes: same seeds, saved scenes replayed where a monitor kept its size
    /// (PLAN.md section 2.1). Explorer restarts start live playback again.
    fn watch_monitors(&mut self, event_loop: &ActiveEventLoop, now: Instant) {
        let mut noticed = false;
        #[cfg(windows)]
        if let Some(w) = &self.watcher {
            noticed = w.take_displays_changed();
            if w.take_taskbar_created() {
                tracing::info!(target: "clairlune", "Explorer restarted");
                self.live.refresh();
            }
            if w.take_clock_changed() {
                tracing::info!(target: "clairlune", "the clock changed or the computer woke; checking the timeline");
                self.check_timeline();
            }
        }
        #[cfg(target_os = "macos")]
        if self.spaces.take_changed() && !self.busy {
            // The stills were set on the Space that was active then.
            self.live.reset_stills();
        }
        if !cfg!(windows) && now >= self.next_monitor_poll {
            self.next_monitor_poll = now + MONITOR_POLL;
            noticed = self.monitor_layout.as_ref() != Some(&monitor_layout(event_loop));
        }
        if self.monitor_layout.is_none() {
            self.monitor_layout = Some(monitor_layout(event_loop));
        }
        if noticed {
            self.monitors_changed_at = Some(now);
        }
        let Some(at) = self.monitors_changed_at else {
            return;
        };
        if now < at + MONITOR_SETTLE || self.busy || self.ui_test.is_some() {
            return;
        }
        self.monitors_changed_at = None;
        let layout = monitor_layout(event_loop);
        if self.monitor_layout.as_ref() == Some(&layout) {
            // A change that changed nothing (a work-area nudge).
            tracing::debug!(target: "clairlune", "display change settled; same monitors");
            return;
        }
        tracing::info!(target: "clairlune", monitors = layout.len(), "monitors changed; making the wallpaper again");
        self.monitor_layout = Some(layout);
        if self.restored() {
            // The user's own wallpaper is on: Clairlune leaves it alone.
        } else if self.current.is_some() {
            self.reapply(false);
        } else {
            self.new_wallpaper();
        }
    }

    /// Live or Still frame and the live frame rate, from the settings.
    fn follow_display_settings(&mut self) {
        let live = self.settings.display.mode == settings::WallpaperMode::Live;
        self.live.set_fps(self.settings.live.smoothness_fps);
        self.live.set_battery_policy(
            self.settings.live.battery_saver_on_battery,
            self.settings.live.pause_on_battery,
        );
        self.live
            .set_cover_pause(self.settings.live.pause_when_covered);
        self.live.set_enabled(live);
        if let Some(t) = &self.tray {
            t.show_live(live && live::supported().is_ok());
        }
    }

    fn status(&self) -> ui::view::Status {
        let mut s = self.status.clone();
        s.checking_updates = self.checking_updates.is_some();
        s.update_url = self.update.newer().and(self.update.url.clone());
        if s.update_note.is_none()
            && let Some(v) = self.update.newer()
        {
            s.update_note = Some(format!("Clairlune {v} is out"));
        }
        s.crash_reports = diagnostics::crash_reports(&self.data_dir).len();
        let until = self.settings.general.detailed_logging_until;
        s.detailed_until = (until > timefmt::now_unix()).then(|| {
            jiff::Timestamp::from_second(until as i64)
                .map(timeline::local_hhmm)
                .unwrap_or_default()
        });
        s.live = self.live.status.clone();
        s.live_supported = live::supported().err();
        s.timeline_next = self
            .timeline_next
            .filter(|_| self.timeline_mode())
            .map(|t| format!("Next slot at {}", timeline::local_hhmm(t)));
        s.timeline_problems = match self.library() {
            Some(lib) => self.timeline.problems(&|id| lib.get(id).is_some()),
            None => Vec::new(),
        };
        s.busy = self.busy;
        s.next_in_minutes = self
            .next_change
            .map(|t| t.saturating_duration_since(Instant::now()).as_secs() / 60);
        s
    }

    fn on_ui_action(&mut self, action: ui::view::UiAction, event_loop: &ActiveEventLoop) {
        use ui::view::UiAction;
        match action {
            UiAction::SettingsChanged => {
                self.save_settings();
                self.sync_tray();
                self.sync_saver(true);
                if let Some(t) = &self.tray {
                    t.show_theme(self.settings.theme.mode);
                    t.show_paused(self.settings.schedule.paused);
                }
                self.follow_display_settings();
                self.reschedule();
                let theme = apply::resolve_theme(&self.settings.theme);
                for w in [
                    &mut self.settings_window,
                    &mut self.library_window,
                    &mut self.timeline_window,
                    &mut self.archive_window,
                ]
                .into_iter()
                .flatten()
                {
                    w.set_theme(theme);
                }
                self.check();
            }
            UiAction::NewWallpaper => self.new_wallpaper_by_hand(),
            UiAction::EditTimeline => self.open_timeline(event_loop),
            UiAction::TimelineChanged => self.timeline_edited(),
            UiAction::Previous => self.reapply(true),
            UiAction::OpenArchive => self.open_archive(event_loop),
            UiAction::OpenArchiveFolder => open(&self.data_dir.join("archive"), false),
            a @ (UiAction::ApplyArchive(_)
            | UiAction::PinArchive { .. }
            | UiAction::SaveArchiveToLibrary(_)
            | UiAction::ExportArchive { .. }
            | UiAction::DeleteArchive(_)) => self.on_archive_action(a),
            UiAction::OpenLogs => open(&self.data_dir.join("logs"), false),
            UiAction::OpenCrashReports => open(&self.data_dir.join("crashes"), false),
            UiAction::SetDetailedLogging(on) => {
                self.settings.general.detailed_logging_until = if on {
                    timefmt::now_unix() + diagnostics::DETAILED_SECS
                } else {
                    0
                };
                diagnostics::set_detailed(on);
                tracing::info!(target: "clairlune", on, "detailed logging");
                self.save_settings();
            }
            UiAction::ExportDiagnostics => {
                let info = diagnostics::SystemInfo {
                    monitors: self
                        .monitor_layout
                        .iter()
                        .flatten()
                        .map(|(name, x, y, w, h)| {
                            format!("{w}x{h} at ({x}, {y}) {}", name.as_deref().unwrap_or(""))
                        })
                        .collect(),
                    live: self.live.status.clone(),
                };
                let config_dir = self
                    .settings_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| self.data_dir.clone());
                let dest = self.export_dir();
                self.status.note = Some(
                    match diagnostics::export_bundle(
                        &self.data_dir,
                        &config_dir,
                        self.current.as_deref(),
                        &info,
                        &dest,
                    ) {
                        Ok(path) => {
                            open(&dest, false);
                            format!("Saved {}", path.display())
                        }
                        Err(e) => format!("Couldn't export diagnostics: {e}"),
                    },
                );
            }
            UiAction::SavePng => {
                self.export_png();
            }
            UiAction::SaveToLibrary => self.save_to_library(),
            UiAction::OpenLibrary => self.open_library(event_loop),
            a @ (UiAction::ApplyLibrary(_)
            | UiAction::UpdateLibrary(_)
            | UiAction::DeleteLibrary { .. }
            | UiAction::ExportLibrary { .. }) => self.on_library_action(a),
            UiAction::SaveGif => {
                self.status.note = Some("Making the GIF...".to_string());
                self.export_gif();
            }
            UiAction::AnswerStartAtLogin(yes) => {
                self.settings.general.asked_start_at_login = true;
                self.status.ask_start_at_login = false;
                self.save_settings();
                if yes {
                    self.on_ui_action(UiAction::SetStartAtLogin(true), event_loop);
                }
            }
            UiAction::Quit => event_loop.exit(),
            UiAction::CheckUpdates => {
                self.status.update_note = None;
                self.check_updates(true);
            }
            UiAction::OpenUpdate => self.open_update(),
            UiAction::SetStartAtLogin(on) => {
                let result = std::env::current_exe()
                    .map_err(|e| e.to_string())
                    .and_then(|exe| {
                        clairlune_platform::set_start_at_login(&exe, on)
                            .map_err(|e| e.to_string())
                            .map(|()| clairlune_platform::start_at_login(&exe))
                    });
                match result {
                    Ok(state) => self.status.start_at_login = state,
                    Err(e) => {
                        self.status.note = Some(format!("Couldn't change start at login: {e}"))
                    }
                }
            }
            UiAction::RestoreWallpaper => {
                // Pause first, or the schedule would replace it again.
                self.settings.schedule.paused = true;
                self.save_settings();
                self.reschedule();
                if let Some(t) = &self.tray {
                    t.show_paused(true);
                }
                self.submit(Job::Restore);
            }
        }
        if let Some(w) = &self.settings_window {
            w.request_redraw();
        }
        let _ = event_loop;
    }

    fn run_ui_test(&mut self, event_loop: &ActiveEventLoop) {
        while let Some(step) = self.ui_test.as_mut().and_then(UiTest::due) {
            match step {
                UiStep::Open(theme, section) => {
                    let opened = Instant::now();
                    self.open_settings(event_loop, Some(theme));
                    if let Some(w) = &mut self.settings_window {
                        w.set_section(section);
                        if section == ui::view::Section::Buddy {
                            // The note a click on the toggle shows.
                            w.toast("Clawd skips the next wallpaper");
                        }
                    }
                    tracing::info!(target: "clairlune", ms = opened.elapsed().as_millis() as u64, memory_mb = memory_mb(), "ui test: settings opened");
                }
                UiStep::Tray(on) => {
                    self.settings.general.show_tray_icon = on;
                    self.sync_tray();
                    tracing::info!(target: "clairlune", shown = self.tray.is_some(), "ui test: tray");
                }
                UiStep::OpenAsking(theme) => {
                    self.open_settings(event_loop, Some(theme));
                    self.status.ask_start_at_login = true;
                    if let Some(w) = &self.settings_window {
                        w.request_redraw();
                    }
                }
                UiStep::Screenshot(name) => {
                    let out = self.ui_test.as_ref().map(|t| t.out.join(name));
                    // The window opened last.
                    let target = [
                        &mut self.archive_window,
                        &mut self.timeline_window,
                        &mut self.library_window,
                        &mut self.settings_window,
                    ]
                    .into_iter()
                    .flatten()
                    .next();
                    if let (Some(w), Some(out)) = (target, out) {
                        w.screenshot_to = Some(out);
                        w.request_redraw();
                    }
                }
                UiStep::OpenLibrary => {
                    self.open_library(event_loop);
                    if let (Some(w), Some(first)) =
                        (&mut self.library_window, self.library_items.first())
                    {
                        w.select_item(first);
                    }
                    tracing::info!(target: "clairlune", items = self.library_items.len(), memory_mb = memory_mb(), "ui test: library opened");
                }
                UiStep::CloseLibrary => {
                    self.library_window = None;
                    self.trim_if_idle();
                    tracing::info!(target: "clairlune", memory_mb = memory_mb(), "ui test: library closed");
                }
                UiStep::OpenTimeline(theme, day, index) => {
                    self.open_timeline(event_loop);
                    if let Some(w) = &mut self.timeline_window {
                        w.set_theme(theme);
                        w.select_slot(day, index);
                    }
                    tracing::info!(target: "clairlune", memory_mb = memory_mb(), "ui test: timeline opened");
                }
                UiStep::CloseTimeline => {
                    self.timeline_window = None;
                    self.trim_if_idle();
                    tracing::info!(target: "clairlune", memory_mb = memory_mb(), "ui test: timeline closed");
                }
                UiStep::OpenArchive => {
                    self.open_archive(event_loop);
                    let pick = self.archive_items.get(1).map(|e| e.name.clone());
                    if let (Some(w), Some(name)) = (&mut self.archive_window, pick) {
                        w.select_entry(&name);
                    }
                    tracing::info!(target: "clairlune", entries = self.archive_items.len(), memory_mb = memory_mb(), "ui test: archive opened");
                }
                UiStep::CloseArchive => {
                    self.archive_window = None;
                    self.trim_if_idle();
                    tracing::info!(target: "clairlune", memory_mb = memory_mb(), "ui test: archive closed");
                }
                UiStep::Close => {
                    self.settings_window = None;
                    self.trim_if_idle();
                    tracing::info!(target: "clairlune", memory_mb = memory_mb(), "ui test: settings closed");
                }
                UiStep::Exit => {
                    tracing::info!(target: "clairlune", "ui test passed: settings opened and closed 3 times on one loop");
                    event_loop.exit();
                }
            }
        }
    }

    /// With no window open, gives the memory a closed window or a finished
    /// render touched back to the system, so the app idles small.
    fn trim_if_idle(&self) {
        let open = [
            &self.settings_window,
            &self.library_window,
            &self.timeline_window,
            &self.archive_window,
        ]
        .iter()
        .any(|w| w.is_some());
        if !open {
            #[cfg(windows)]
            clairlune_platform::windows_live::trim_memory();
        }
    }

    /// Once a minute while something runs (live, or a window open), else
    /// every quarter hour: CPU, memory and what live drew, in the log, so a
    /// report of a busy app says where it goes.
    fn log_usage(&mut self, now: Instant) {
        let windows = [
            &self.settings_window,
            &self.library_window,
            &self.timeline_window,
            &self.archive_window,
        ]
        .iter()
        .filter(|w| w.is_some())
        .count();
        let busy = windows > 0 || self.live.frame_interval() < Duration::from_secs(1);
        let every = if busy {
            Duration::from_secs(60)
        } else {
            Duration::from_secs(15 * 60)
        };
        if now < self.usage_at + every {
            return;
        }
        let cpu = clairlune_platform::usage::process_cpu_time();
        let wall = now - self.usage_at;
        let pct = (cpu.saturating_sub(self.usage_cpu)).as_secs_f64() / wall.as_secs_f64().max(1e-3)
            * 100.0;
        let f = self.live.take_use();
        let avg_ms = if f.frames > 0 {
            f.busy.as_secs_f64() * 1e3 / f.frames as f64
        } else {
            0.0
        };
        tracing::info!(
            target: "clairlune",
            cpu_pct = format!("{pct:.1}"),
            memory_mb = memory_mb(),
            live_monitors = f.monitors,
            live_fps = f.fps,
            frames = f.frames,
            frame_ms_avg = format!("{avg_ms:.2}"),
            frame_ms_worst = format!("{:.2}", f.worst.as_secs_f64() * 1e3),
            windows,
            "usage"
        );
        self.usage_at = now;
        self.usage_cpu = cpu;
    }

    /// Installs or removes the screen saver as the setting says, and keeps
    /// an installed one the app's own version. `by_hand`: the user just
    /// turned it on, so System Settings opens to pick it.
    fn sync_saver(&mut self, by_hand: bool) {
        let want = self.settings.general.screen_saver && saver::shipped().is_some();
        let turned_on = want && !self.saver_on;
        self.saver_on = want;
        if want {
            if saver::installed_version().as_deref() != Some(env!("CARGO_PKG_VERSION")) {
                if let Err(e) = saver::install() {
                    tracing::warn!(target: "clairlune", error = %e, "screen saver not installed");
                    self.status.note = Some(format!("Couldn't add the screen saver: {e}"));
                    return;
                }
                tracing::info!(target: "clairlune", "screen saver installed");
            }
            self.write_saver_scene();
            if turned_on && by_hand {
                saver::open_settings();
            }
        } else if !self.settings.general.screen_saver && saver::installed_version().is_some() {
            match saver::uninstall() {
                Ok(()) => tracing::info!(target: "clairlune", "screen saver removed"),
                Err(e) => {
                    tracing::warn!(target: "clairlune", error = %e, "screen saver not removed")
                }
            }
        }
    }

    /// The newer version's release page, in the browser.
    fn open_update(&self) {
        let url = self.update.url.as_deref().unwrap_or(update::RELEASES_PAGE);
        open_url(url);
    }

    /// Hands the scene on screen to the screen saver, when it's on.
    fn write_saver_scene(&self) {
        if !self.settings.general.screen_saver || !cfg!(target_os = "macos") {
            return;
        }
        if let Some(scene) = &self.saver_scene
            && let Err(e) = saver::write_scene(&self.data_dir, scene)
        {
            tracing::warn!(target: "clairlune", error = %e, "screen saver scene not written");
        }
    }

    /// The tray icon is wanted but isn't up (no tray on this desktop, or it
    /// failed): Settings is the only way in, and closing it quits.
    fn tray_missing(&self) -> bool {
        self.settings.general.show_tray_icon && self.tray.is_none()
    }

    /// Shows or takes away the tray icon (the menu bar icon on macOS) as
    /// the settings say. Hidden, the app runs on; opening it again (a
    /// second launch) brings up Settings.
    fn sync_tray(&mut self) {
        self.sync_tray_inner();
        if let Some(t) = &self.tray {
            t.show_update(self.update.newer());
        }
    }

    fn sync_tray_inner(&mut self) {
        let want = self.settings.general.show_tray_icon;
        if !want {
            if self.tray.take().is_some() {
                tracing::info!(target: "clairlune", "tray icon hidden");
            }
            return;
        }
        if self.tray.is_some() {
            return;
        }
        match tray::Tray::new(
            self.settings.theme.mode,
            self.settings.schedule.paused,
            self.settings.display.mode == settings::WallpaperMode::Live
                && live::supported().is_ok(),
        ) {
            Ok(t) => {
                t.set_monitors(&self.tray_monitors);
                self.tray = Some(t);
                tracing::debug!(target: "clairlune", memory_mb = memory_mb(), "tray ready");
            }
            Err(e) => tracing::error!(target: "clairlune", error = %e, "no tray icon"),
        }
    }

    /// Saves the wallpaper on screen (not the newest archive entry: that's
    /// another one after Previous or a Library pick).
    fn export_png(&mut self) {
        let dest = self.export_dir();
        self.submit(Job::Export {
            dest,
            source: self.current.clone(),
            still: self.shown_still.clone(),
        });
    }

    fn export_gif(&mut self) {
        let dest = self.export_dir();
        let theme = self
            .shown_theme
            .unwrap_or_else(|| apply::resolve_theme(&self.settings.theme));
        self.submit(Job::ExportGif {
            dest,
            source: self.current.clone(),
            theme,
            buddy: self.current_buddy.unwrap_or(self.settings.buddy.show),
        });
    }

    /// The export folder: the setting, or `Pictures/Clairlune`.
    fn export_dir(&self) -> PathBuf {
        if self.settings.export.folder.trim().is_empty() {
            clairlune_platform::pictures_dir()
                .unwrap_or_else(|| self.data_dir.clone())
                .join(APP_NAME)
        } else {
            PathBuf::from(self.settings.export.folder.trim())
        }
    }

    fn on_done(&mut self, done: Done) {
        match done {
            Done::Applied {
                source,
                seed,
                theme,
                wanted,
                archetype,
                activity,
                monitors,
                ms,
                live,
            } => {
                self.busy = false;
                self.set_restored(false);
                self.current = source;
                diagnostics::set_current(self.current.clone());
                self.shown_theme = Some(theme);
                self.asked_theme = Some(wanted);
                self.shown_journey = live.iter().any(|s| s.package.recipe.is_journey());
                self.shown_still = live
                    .iter()
                    .find(|s| s.monitor.primary)
                    .or(live.first())
                    .filter(|s| s.view.is_none())
                    .and_then(|s| s.still.clone());
                // What the screen saver plays: these scenes, at each screen.
                self.saver_scene = (!live.is_empty()).then(|| clairlune_scene::saver::SaverScene {
                    theme,
                    buddy: live.first().is_none_or(|s| s.buddy),
                    fps: self.settings.live.smoothness_fps.clamp(6, 30),
                    monitors: live
                        .iter()
                        .map(|s| clairlune_scene::saver::SaverMonitor {
                            width: s.monitor.width,
                            height: s.monitor.height,
                            recipe: s.package.recipe.clone(),
                        })
                        .collect(),
                });
                self.write_saver_scene();
                // One scene per monitor: each can get a new one. (A span is
                // one scene.)
                let span = live.iter().any(|s| s.view.is_some());
                self.tray_monitors = if span {
                    Vec::new()
                } else {
                    live.iter().map(|s| s.monitor.clone()).collect()
                };
                if let Some(t) = &self.tray {
                    t.set_monitors(&self.tray_monitors);
                }
                self.live.show(live);
                self.refresh_archive();
                self.trim_if_idle();
                let what = match activity {
                    Some(a) => {
                        format!("{} in {}", a.replace('_', " "), archetype.replace('_', " "))
                    }
                    None => archetype.replace('_', " "),
                };
                let mut text = format!("{APP_NAME}: {what} (seed {seed})");
                if self.timeline_mode()
                    && let Some(next) = self.timeline_next
                {
                    text.push_str(&format!(", next at {}", timeline::local_hhmm(next)));
                }
                tracing::debug!(target: "clairlune", monitors, ms, "{text}");
                self.status.what = what;
                self.status.seed = Some(seed);
                self.status.theme = Some(theme);
                self.refresh_window_thumb();
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(&text);
                }
            }
            Done::Exported(path) => {
                tracing::info!(target: "clairlune", path = %path.display(), "saved");
                self.status.note = Some(format!("Saved {}", path.display()));
                if let Some(dir) = path.parent() {
                    open(dir, false);
                }
            }
            Done::NoHost(why) => {
                self.status.note = Some(why);
                if let Some(w) = &self.settings_window {
                    w.request_redraw();
                }
            }
            Done::Restored => {
                self.busy = false;
                self.set_restored(true);
                // Stop playing over it (with no scenes left, stopping
                // doesn't set Clairlune's stills again).
                self.live.show(Vec::new());
                self.shown_theme = None;
                self.shown_journey = false;
                self.shown_still = None;
                self.current = None;
                self.status.what.clear();
                self.status.seed = None;
                self.status.theme = None;
                self.status.note =
                    Some("Your wallpaper is back. The schedule is paused.".to_string());
                if let Some(w) = &mut self.settings_window {
                    w.clear_thumb();
                }
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(APP_NAME);
                }
            }
            Done::Failed(e) => {
                self.busy = false;
                tracing::debug!(target: "clairlune", error = %e, "shown in the tray tooltip");
                self.status.note = Some(capitalise_first(&e));
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(&format!("{APP_NAME}: {e}"));
                }
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause != StartCause::Init {
            return;
        }
        // macOS wants the tray created once the loop runs.
        self.sync_tray();
        if self.ui_test.is_none() {
            self.sync_saver(false);
        }
        if let Some(report) = diagnostics::new_crash_report(&self.data_dir) {
            tracing::warn!(target: "clairlune", report = %report.display(), "Clairlune stopped unexpectedly last time");
            self.status.note = Some(
                "Clairlune stopped unexpectedly last time. The crash report is under General, Crash reports."
                    .into(),
            );
        }
        // The first run asks, once, whether to start quietly at sign-in;
        // already registered (or nowhere to register) is an answer too.
        if !self.settings.general.asked_start_at_login && self.ui_test.is_none() {
            let registered = std::env::current_exe()
                .ok()
                .and_then(|exe| clairlune_platform::start_at_login(&exe));
            if registered != Some(false) {
                self.settings.general.asked_start_at_login = true;
                self.save_settings();
            }
        }
        // Settings opens at launch unless Clairlune started quietly (at
        // sign-in), and always when the tray it wants didn't come up: then
        // Settings is its only face and closing it quits (PLAN.md section
        // 10, "No tray available"). And for the first run's question.
        if self.ui_test.is_none()
            && (!self.background
                || self.tray_missing()
                || !self.settings.general.asked_start_at_login)
        {
            self.open_settings(event_loop, None);
        }
        let latest = self.archive().and_then(|a| a.latest());
        self.current = latest
            .as_ref()
            .and_then(|e| self.archive().map(|a| a.dir().join(&e.name)));
        if let Some(e) = &latest {
            self.status.what = match &e.activity {
                Some(a) => format!(
                    "{} in {}",
                    a.replace('_', " "),
                    e.archetype.replace('_', " ")
                ),
                None => e.archetype.replace('_', " "),
            };
            self.status.seed = Some(e.seed);
        }
        if self.ui_test.is_some() {
            // UI test: leave the wallpaper alone (this only finds the next slot).
            self.check_timeline();
            return;
        }
        self.shown_theme = latest.and_then(|e| match e.theme.as_str() {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        });
        // What it was made for isn't saved; take what it shows.
        self.asked_theme = self.shown_theme;
        let paused = self.settings.schedule.paused;
        if self.restored() {
            // The user's own wallpaper is back (Restore): leave it, and
            // nothing of Clairlune's is on screen to re-render.
            self.current = None;
            self.shown_theme = None;
        } else if self.timeline_mode() {
            // The timeline's active slot, right away.
            self.check_timeline();
        } else if (self.settings.schedule.change_at_login && !paused) || self.current.is_none() {
            self.new_wallpaper();
        } else if self.settings.display.mode == settings::WallpaperMode::Live {
            // Play what's there (the saved scenes, no new one).
            self.reapply(false);
        }
        self.reschedule();
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let status = self.status();
        let mut actions = Vec::new();
        let before = self.settings.clone();
        let library_dir = self.data_dir.join("library");
        let archive_dir = self.data_dir.join("archive");
        let current_entry = self
            .current
            .as_ref()
            .filter(|p| p.starts_with(&archive_dir))
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        let Some(w) = [
            &mut self.settings_window,
            &mut self.library_window,
            &mut self.timeline_window,
            &mut self.archive_window,
        ]
        .into_iter()
        .flatten()
        .find(|w| w.id() == id) else {
            return;
        };
        let mut model = ui::Model {
            settings: &mut self.settings,
            status: &status,
            library: &self.library_items,
            library_dir: &library_dir,
            uses: &self.library_uses,
            timeline: &mut self.timeline,
            archive: &self.archive_items,
            archive_dir: &archive_dir,
            current_entry: current_entry.as_deref(),
        };
        let outcome = w.on_event(&event, &mut model, &mut actions);
        let kind = w.kind();
        if kind == ui::Kind::Settings {
            self.last_section = w.section();
        }
        if let ui::Outcome::Close = outcome {
            match kind {
                ui::Kind::Settings => {
                    self.settings_window = None;
                    tracing::debug!(target: "clairlune", "settings closed");
                    if self.tray_missing()
                        && self.ui_test.is_none()
                        && self.library_window.is_none()
                        && self.timeline_window.is_none()
                        && self.archive_window.is_none()
                    {
                        event_loop.exit();
                    }
                }
                ui::Kind::Library => self.library_window = None,
                ui::Kind::Timeline => self.timeline_window = None,
                ui::Kind::Archive => self.archive_window = None,
            }
            self.trim_if_idle();
        }
        if self.settings == before {
            actions.retain(|a| *a != ui::view::UiAction::SettingsChanged);
        }
        for a in actions {
            self.on_ui_action(a, event_loop);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::Menu(e) => {
                if let Some(action) = self.tray.as_ref().and_then(|t| t.action(e.id())) {
                    tracing::debug!(target: "clairlune", ?action, "menu");
                    self.on_action(action, event_loop);
                }
            }
            AppEvent::Done(d) => self.on_done(d),
            AppEvent::OpenSettings => self.open_settings(event_loop, None),
            AppEvent::UpdateChecked(r) => self.on_update_checked(r),
            AppEvent::LiveFrame => {
                let next = self.live.tick(Instant::now());
                self.set_metronome(next.is_some());
            }
        }
        let _ = event_loop;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.run_ui_test(event_loop);
        let now = Instant::now();
        self.watch_monitors(event_loop, now);
        if self.next_change.is_some_and(|t| now >= t) && !self.busy {
            self.new_wallpaper();
            self.reschedule();
        }
        if now >= self.next_check {
            self.check();
            self.next_check = now + CHECK_EVERY;
        }
        self.log_usage(now);
        let live_next = self.live.tick(Instant::now());
        self.set_metronome(live_next.is_some());
        let mut wake = match self.next_change {
            Some(t) => t.min(self.next_check),
            None => self.next_check,
        };
        if let Some(t) = live_next {
            wake = wake.min(t);
        }
        if let Some(t) = self.monitors_changed_at {
            wake = wake.min(t + MONITOR_SETTLE);
        }
        // Wake in time to prepare the next timeline slot.
        if let Some(t) = self.timeline_prepare_at() {
            if Instant::now() >= t {
                if !self.busy {
                    self.check_timeline();
                }
            } else {
                wake = wake.min(t);
            }
        }
        if !cfg!(windows) {
            wake = wake.min(self.next_monitor_poll);
        }
        if let Some(t) = self.ui_test.as_ref().and_then(|t| t.wake()) {
            wake = wake.min(t);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        tracing::info!(target: "clairlune", "quitting");
    }
}

/// Resident memory in MB, for the log (Windows; 0 elsewhere).
pub(crate) fn memory_mb() -> u64 {
    clairlune_platform::usage::process_memory() >> 20
}

/// Logs to a daily file in `<data>/logs/` (seven kept), and to stderr in debug
/// builds. The guard flushes the file writer when dropped.
fn init_logging(data_dir: &Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let filter = diagnostics::filter_layer(false);
    // The appender scans the folder for old files to prune, so it must exist;
    // it keeps seven days, and the folder stays under 50 MB.
    let _ = std::fs::create_dir_all(data_dir.join("logs"));
    diagnostics::prune_logs(&data_dir.join("logs"), diagnostics::LOG_CAP_BYTES);
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("clairlune")
        .filename_suffix("log")
        .max_log_files(7)
        .build(data_dir.join("logs"));
    let (file_layer, guard) = match appender {
        Ok(a) => {
            let (writer, guard) = tracing_appender::non_blocking(a);
            (
                Some(fmt::layer().with_writer(writer).with_ansi(false)),
                Some(guard),
            )
        }
        Err(_) => (None, None),
    };
    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));
    tracing_subscriber::registry()
        .with(filter)
        .with(diagnostics::Tail)
        .with(file_layer)
        .with(stderr_layer)
        .init();
    diagnostics::install_crash_reports(data_dir);
    guard
}

/// Holds an OS lock on `<data>/instance.lock` while the app runs, so a second
/// copy exits instead of adding a second tray icon and schedule.
fn single_instance(data_dir: &Path) -> Result<std::fs::File, String> {
    let path = data_dir.join("instance.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err("already running".into()),
        Err(std::fs::TryLockError::Error(e)) => Err(e.to_string()),
    }
}

/// `--ui-test`: three archived scenes in the scratch folder, two of them
/// saved to the library, and a weekly timeline using them, so the windows
/// show real content.
fn seed_ui_test(data_dir: &Path) -> Result<(), String> {
    use clairlune_scene::{Aspect, ContentContext, compose};
    let archive = archive::Archive::open(data_dir.join("archive")).map_err(|e| e.to_string())?;
    let lib = library::Library::open(data_dir.join("library")).map_err(|e| e.to_string())?;
    let mut saved = Vec::new();
    for (i, seed) in [3u64, 7, 42].into_iter().enumerate() {
        let recipe = compose(seed, Aspect::from_size(16, 9), &ContentContext::default());
        let pkg = clairlune_render::ScenePackage::capture(&recipe, 960, 540)
            .map_err(|e| e.to_string())?;
        let theme = pkg.themes()[0];
        let (img, palette) = pkg.still(theme, true).ok_or("no frame")?;
        let png = apply::png_of(&img, palette)?;
        let thumb = apply::png_of(&apply::shrink(&img, 480), palette)?;
        let entry = archive
            .add(&archive::NewEntry {
                created: 1_000 + i as u64,
                recipes: archive::EntryRecipes {
                    base_seed: seed,
                    theme: theme.as_str().into(),
                    monitors: vec![archive::MonitorScene {
                        monitor: "ui-test".into(),
                        width: 960,
                        height: 540,
                        recipe,
                    }],
                },
                pngs: vec![png],
                thumb_png: thumb,
                packages: vec![std::sync::Arc::new(pkg)],
            })
            .map_err(|e| e.to_string())?;
        if i < 2 {
            let item = lib
                .save_entry(&archive.dir().join(&entry.name), &entry, 2_000 + i as u64)
                .map_err(|e| e.to_string())?;
            saved.push(item.id);
        }
    }
    let [a, b] = [&saved[0], &saved[1]];
    let tl = format!(
        r#"layout = "weekly"

[[day.monday.slot]]
start = "00:00"
content = {{ type = "random", archetypes = ["night_sky", "cave"] }}
theme = "dark"

[[day.monday.slot]]
start = "07:00"
content = {{ type = "item", id = "{a}" }}
theme = "light"

[[day.monday.slot]]
start = "12:30"
content = {{ type = "pool", ids = ["{a}", "{b}"], order = "shuffle" }}

[[day.monday.slot]]
start = "18:15"
content = {{ type = "item", id = "{b}" }}

[[day.monday.slot]]
start = "22:00"
content = {{ type = "random", archetypes = ["night_sky"], activities = ["stargaze"] }}
theme = "dark"

[[day.wednesday.slot]]
start = "08:00"
content = {{ type = "item", id = "{b}" }}

[[day.wednesday.slot]]
start = "17:45"
content = {{ type = "random" }}

[[day.friday.slot]]
start = "06:30"
content = {{ type = "item", id = "{a}" }}

[[day.friday.slot]]
start = "20:00"
content = {{ type = "item", id = "lib_gone" }}

[[day.saturday.slot]]
start = "09:00"
content = {{ type = "pool", ids = ["{b}", "{a}"], order = "sequential" }}

[[day.sunday.slot]]
start = "10:00"
content = {{ type = "random", archetypes = ["shore", "forest"] }}
"#
    );
    std::fs::write(data_dir.join("timeline.toml"), tl).map_err(|e| e.to_string())?;
    std::fs::write(
        data_dir.join("settings.toml"),
        "[rotation]\nmode = \"timeline\"\n\n[theme]\nmode = \"schedule\"\nfollow_sun = true\nlocation = \"40.71, -74.01\"\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn spawn_worker(
    rx: mpsc::Receiver<Job>,
    proxy: EventLoopProxy<AppEvent>,
    archive_dir: PathBuf,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("clairlune-worker".into())
        .spawn(move || {
            apply::worker(
                rx,
                move |d| {
                    let _ = proxy.send_event(AppEvent::Done(d));
                },
                archive_dir,
            )
        })
        .map(|_| ())
}

/// What the command line asks for before the app starts, if anything: the
/// version or the usage (package managers run `clairlune --version` to check
/// an install), printed by the caller, which then exits.
fn command_line_answer(args: &[String]) -> Option<String> {
    let first = args.get(1).map(String::as_str)?;
    match first {
        "--version" | "-V" => Some(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"))),
        "--help" | "-h" => Some(format!(
            "{APP_NAME} {}: procedural, animated ASCII-art wallpapers.\n\n\
             Run it without arguments: it lives in the notification area (the menu bar on macOS).\n\
             Options:\n  --background  start quietly: no window (how it starts at sign-in)\n  --version     print the version and exit\n  --help        print this and exit",
            env!("CARGO_PKG_VERSION")
        )),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(text) = command_line_answer(&args) {
        // A GUI build has no console of its own: borrow the caller's, if any.
        #[cfg(windows)]
        clairlune_platform::attach_parent_console();
        println!("{text}");
        return;
    }
    clairlune_platform::init_dpi_awareness();
    // `--ui-test <dir>`: the scripted Settings check, in a scratch data folder,
    // leaving the wallpaper alone.
    let ui_test_out = args
        .iter()
        .position(|a| a == "--ui-test")
        .map(|i| PathBuf::from(args.get(i + 1).cloned().unwrap_or_else(|| ".".into())));
    let data_dir = match &ui_test_out {
        Some(out) => Some(out.join("data")),
        None => clairlune_platform::app_data_dir(),
    };
    let Some(data_dir) = data_dir else {
        eprintln!("{APP_NAME}: no data directory");
        return;
    };
    let _ = std::fs::create_dir_all(&data_dir);
    let _log = init_logging(&data_dir);
    if ui_test_out.is_some()
        && let Err(e) = seed_ui_test(&data_dir)
    {
        tracing::warn!(target: "clairlune", error = %e, "ui test content");
    }
    // Started at sign-in: quietly. An entry from 0.12 or older starts the
    // app without saying so: quiet as it always was, and brought up to date.
    let mut background = args.iter().any(|a| a == clairlune_platform::BACKGROUND_ARG);
    if ui_test_out.is_none()
        && let Ok(exe) = std::env::current_exe()
        && clairlune_platform::login_entry_outdated(&exe)
    {
        background = true;
        if let Err(e) = clairlune_platform::set_start_at_login(&exe, true) {
            tracing::warn!(target: "clairlune", error = %e, "couldn't update the login entry");
        }
    }
    let _instance = match single_instance(&data_dir) {
        Ok(f) => f,
        Err(_) if background => return,
        Err(e) => {
            let asked = ipc::ask_running_copy(&data_dir);
            tracing::info!(target: "clairlune", reason = %e, asked, "another copy is running; asked it to open Settings");
            return;
        }
    };
    let settings_path = match &ui_test_out {
        Some(_) => data_dir.join("settings.toml"),
        None => clairlune_platform::app_config_dir()
            .unwrap_or_else(|| data_dir.clone())
            .join("settings.toml"),
    };
    let settings = Settings::load(&settings_path);
    let timeline_path = settings_path.with_file_name("timeline.toml");
    if settings.general.detailed_logging_until > timefmt::now_unix() {
        diagnostics::set_detailed(true);
    }
    tracing::info!(
        target: "clairlune",
        version = env!("CARGO_PKG_VERSION"),
        generator_version = clairlune_scene::GENERATOR_VERSION,
        os = std::env::consts::OS,
        settings = %settings_path.display(),
        data = %data_dir.display(),
        memory_mb = memory_mb(),
        "starting"
    );

    let event_loop = match EventLoop::<AppEvent>::with_user_event().build() {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(target: "clairlune", error = %e, "no event loop");
            return;
        }
    };
    let proxy: EventLoopProxy<AppEvent> = event_loop.create_proxy();
    {
        let p = proxy.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(move |e| {
            let _ = p.send_event(AppEvent::Menu(e));
        }));
        // Double-clicking the tray icon opens the app (Windows; the menu is
        // on a right click).
        let p = proxy.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |e| {
            if let tray_icon::TrayIconEvent::DoubleClick {
                button: tray_icon::MouseButton::Left,
                ..
            } = e
            {
                let _ = p.send_event(AppEvent::OpenSettings);
            }
        }));
    }

    if ui_test_out.is_none() {
        let p = proxy.clone();
        if let Err(e) = ipc::serve(&data_dir, move || {
            let _ = p.send_event(AppEvent::OpenSettings);
        }) {
            tracing::warn!(target: "clairlune", error = %e, "no single-instance channel; a second launch won't open Settings");
        }
    }
    let metronome_proxy = proxy.clone();
    let app_proxy = proxy.clone();
    let update_state = update::UpdateState::load(&data_dir);
    let (jobs, rx) = mpsc::channel();
    let archive_dir = data_dir.join("archive");
    // The UI test never touches the wallpaper, so it runs without a worker.
    let worker = if ui_test_out.is_some() {
        drop(rx);
        Ok(())
    } else {
        spawn_worker(rx, proxy, archive_dir)
    };
    if let Err(e) = worker {
        tracing::error!(target: "clairlune", error = %e, "no worker thread");
        return;
    }
    let pools: PoolState = std::fs::read_to_string(data_dir.join("state.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut live = live::Live::new(
        settings.display.mode == settings::WallpaperMode::Live,
        settings.live.smoothness_fps,
    );
    live.set_battery_policy(
        settings.live.battery_saver_on_battery,
        settings.live.pause_on_battery,
    );
    live.set_cover_pause(settings.live.pause_when_covered);
    #[cfg(windows)]
    let watcher = match clairlune_platform::windows_live::SystemWatcher::new() {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::warn!(target: "clairlune", error = %e, "no display-change watcher");
            None
        }
    };
    // The live metronome: a LiveFrame event every frame while live plays.
    let metronome = Arc::new(AtomicU64::new(0));
    {
        let (m, p) = (metronome.clone(), metronome_proxy);
        let _ = std::thread::Builder::new()
            .name("clairlune-live-metronome".into())
            .spawn(move || {
                loop {
                    let us = m.load(Ordering::Relaxed);
                    if us == 0 {
                        std::thread::sleep(Duration::from_millis(200));
                        continue;
                    }
                    std::thread::sleep(Duration::from_micros(us));
                    if p.send_event(AppEvent::LiveFrame).is_err() {
                        return; // the event loop is gone
                    }
                }
            });
    }
    let mut app = App {
        live,
        metronome,
        #[cfg(windows)]
        watcher,
        #[cfg(target_os = "macos")]
        spaces: clairlune_platform::macos::SpaceWatcher::new(),
        monitor_layout: None,
        monitors_changed_at: None,
        next_monitor_poll: Instant::now(),
        settings_stamp: file_stamp(&settings_path),
        settings,
        settings_path,
        data_dir,
        jobs,
        proxy: app_proxy,
        update: update_state,
        checking_updates: None,
        busy: false,
        next_change: None,
        next_check: Instant::now() + CHECK_EVERY,
        usage_at: Instant::now(),
        usage_cpu: clairlune_platform::usage::process_cpu_time(),
        shown_theme: None,
        asked_theme: None,
        chosen: false,
        shown_journey: false,
        shown_still: None,
        scrolling_ok: true,
        renew: None,
        current: None,
        current_theme: None,
        current_buddy: None,
        tray: None,
        timeline: timeline::Timeline::load(&timeline_path).unwrap_or_default(),
        timeline_stamp: file_stamp(&timeline_path),
        timeline_path,
        timeline_since: None,
        timeline_applied: None,
        timeline_next: None,
        timeline_generate: false,
        manual_until: None,
        apply_at: None,
        pools,
        settings_window: None,
        library_window: None,
        timeline_window: None,
        archive_window: None,
        archive_items: Vec::new(),
        library_items: Vec::new(),
        library_uses: std::collections::BTreeMap::new(),
        last_section: ui::view::Section::Rotation,
        status: ui::view::Status::default(),
        ui_test: ui_test_out.map(UiTest::new),
        background,
        saver_scene: None,
        saver_on: false,
        taskbar_light: clairlune_platform::taskbar_is_light(),
        tray_monitors: Vec::new(),
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        tracing::error!(target: "clairlune", error = %e, "event loop ended with an error");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use settings::ThemeMode;

    #[test]
    fn version_and_help_answer_without_starting() {
        let args = |a: &[&str]| -> Vec<String> {
            std::iter::once("clairlune")
                .chain(a.iter().copied())
                .map(String::from)
                .collect()
        };
        let v = command_line_answer(&args(&["--version"])).unwrap();
        assert_eq!(v, format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")));
        assert_eq!(command_line_answer(&args(&["-V"])), Some(v));
        assert!(
            command_line_answer(&args(&["--help"]))
                .unwrap()
                .contains("--version")
        );
        assert_eq!(command_line_answer(&args(&[])), None);
        assert_eq!(command_line_answer(&args(&["--ui-test", "x"])), None);
    }

    #[test]
    fn theme_mode_resolves() {
        let with = |mode| settings::ThemeSettings {
            mode,
            ..Default::default()
        };
        assert_eq!(apply::resolve_theme(&with(ThemeMode::Light)), Theme::Light);
        assert_eq!(apply::resolve_theme(&with(ThemeMode::Dark)), Theme::Dark);
        let _ = apply::resolve_theme(&with(ThemeMode::FollowSystem));
    }

    /// Schedule: dark between the two times (across midnight or not), or
    /// from sunset to sunrise at a place.
    #[test]
    fn a_theme_schedule_follows_the_clock_or_the_sun() {
        use jiff::civil::date;
        let ny = jiff::tz::TimeZone::get("America/New_York").unwrap();
        let at = |h: i8, m: i8| {
            date(2026, 6, 21)
                .at(h, m, 0, 0)
                .to_zoned_datetime(ny.clone())
                .unwrap()
        };
        let mut t = settings::ThemeSettings {
            mode: ThemeMode::Schedule,
            ..Default::default()
        };
        // 19:00 to 07:00, across midnight.
        assert_eq!(apply::resolve_theme_at(&t, &at(18, 59)), Theme::Light);
        assert_eq!(apply::resolve_theme_at(&t, &at(19, 0)), Theme::Dark);
        assert_eq!(apply::resolve_theme_at(&t, &at(2, 0)), Theme::Dark);
        assert_eq!(apply::resolve_theme_at(&t, &at(7, 0)), Theme::Light);
        // A dark stretch inside the day.
        (t.dark_from, t.light_from) = ("13:00".into(), "14:30".into());
        assert_eq!(apply::resolve_theme_at(&t, &at(12, 59)), Theme::Light);
        assert_eq!(apply::resolve_theme_at(&t, &at(14, 0)), Theme::Dark);
        assert_eq!(apply::resolve_theme_at(&t, &at(14, 30)), Theme::Light);
        // The sun in New York at midsummer: up 05:25 to 20:31.
        t.follow_sun = true;
        t.location = "40.71, -74.01".into();
        assert_eq!(apply::resolve_theme_at(&t, &at(5, 10)), Theme::Dark);
        assert_eq!(apply::resolve_theme_at(&t, &at(13, 30)), Theme::Light);
        assert_eq!(apply::resolve_theme_at(&t, &at(20, 20)), Theme::Light);
        assert_eq!(apply::resolve_theme_at(&t, &at(20, 45)), Theme::Dark);
        // No usable location: the times apply.
        t.location = "somewhere".into();
        assert_eq!(apply::resolve_theme_at(&t, &at(13, 30)), Theme::Dark);
    }

    #[test]
    fn a_second_instance_is_refused() {
        let dir = std::env::temp_dir().join(format!("clairlune-instance-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = single_instance(&dir).unwrap();
        assert!(single_instance(&dir).is_err());
        drop(first);
        assert!(single_instance(&dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
