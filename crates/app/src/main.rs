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
mod install;
mod ipc;
mod library;
mod live;
mod old_saver;
mod report;
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
use std::sync::mpsc;
#[cfg(windows)]
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
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

enum AppEvent {
    Menu(tray_icon::menu::MenuEvent),
    Done(Done),
    /// A second launch asked for Settings.
    OpenSettings,
    /// The update check's answer.
    UpdateChecked(Result<update::Release, String>),
    /// How installing an update goes.
    Installing(InstallStep),
    /// More of Content's landscape tiles have been drawn; `true` when the
    /// last one is.
    LandsDrawn(bool),
    /// The live metronome: a frame may be due. It arrives even while one
    /// of the app's windows is dragged or resized (Windows runs its own
    /// message loop then, and winit's timers stop; user events still come).
    #[cfg(windows)]
    LiveFrame,
}

/// Installing an update, step by step (from its thread).
enum InstallStep {
    /// Downloading this version: the share done, 0 to 100.
    Progress(String, u32),
    /// Downloaded, checked and ready, or why not.
    Ready(Result<install::Prepared, String>),
}

struct App {
    settings: Settings,
    settings_path: PathBuf,
    settings_stamp: Option<SystemTime>,
    data_dir: PathBuf,
    jobs: mpsc::Sender<Job>,
    /// For threads that report back (the update check).
    proxy: EventLoopProxy<AppEvent>,
    /// Content's landscape tiles are being drawn (`land_thumbs`).
    drawing_lands: bool,
    /// What the last update check found, and whether one is running (and
    /// was asked for by hand: then its answer shows even when it's "up to
    /// date").
    update: update::UpdateState,
    checking_updates: Option<bool>,
    /// How this copy was installed (so whether it can update itself).
    install: install::Install,
    /// An update being installed: what it's doing.
    installing: Option<String>,
    /// Remove Clairlune was asked for: once the old wallpaper is back, the
    /// helper deletes the rest and the app quits.
    removing: bool,
    /// The program file as it was at start; and whether a new copy was put
    /// in its place (Homebrew, a DMG copy dragged over): seen once, then
    /// again a moment later, it restarts into it.
    exe_stamp: Option<install::ExeStamp>,
    exe_replaced: Option<install::ExeStamp>,
    /// Lighter on Retina screens as the wallpaper on screen was drawn.
    lighter: bool,
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
    /// Windows: the live metronome's frame interval in microseconds (0:
    /// idle, parked), and its thread, to wake it.
    #[cfg(windows)]
    metronome: (Arc<AtomicU64>, std::thread::Thread),
    /// Hears display changes and Explorer restarts (Windows).
    #[cfg(windows)]
    watcher: Option<clairlune_platform::windows_live::SystemWatcher>,
    /// Hears Space changes: a picture is set on the current Space only (macOS).
    #[cfg(target_os = "macos")]
    spaces: clairlune_platform::macos::SpaceWatcher,
    /// The monitor layout the wallpaper was made for, and when a change was
    /// noticed (it settles before anything is redone).
    monitor_layout: Option<Vec<MonitorKey>>,
    monitors_changed_at: Option<Instant>,
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
    /// The taskbar's colour when last looked at (the tray mark follows it).
    taskbar_light: Option<bool>,
    /// The monitors the tray's "New wallpaper on" lists, for a tray made
    /// again after being hidden.
    tray_monitors: Vec<clairlune_platform::Monitor>,
    /// The monitors the wallpaper is on (for Settings: each one's own
    /// settings).
    screens: Vec<clairlune_platform::Monitor>,
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
    /// Settings' page state: a monitor picked on Content, the problem
    /// report shown, Remove Clairlune asking, scrolled down this far.
    Page(Option<&'static str>, bool, bool, f32),
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
            (Theme::Light, Section::About, "settings-light-about.png"),
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
        // Each monitor's own settings (two monitors, made up for the test).
        steps.push((ms(19000), UiStep::Open(Theme::Dark, Section::Display)));
        steps.push((ms(19300), UiStep::Page(None, false, false, 300.0)));
        steps.push((ms(19800), UiStep::Screenshot("settings-dark-display.png")));
        steps.push((ms(20300), UiStep::Close));
        // A monitor with its own choices, and the problem report.
        steps.push((ms(20600), UiStep::Open(Theme::Light, Section::Content)));
        steps.push((
            ms(20900),
            UiStep::Page(Some("ui-test-2"), false, false, 0.0),
        ));
        steps.push((
            ms(21400),
            UiStep::Screenshot("settings-light-content-monitor.png"),
        ));
        steps.push((ms(21900), UiStep::Close));
        steps.push((ms(22200), UiStep::Open(Theme::Dark, Section::General)));
        steps.push((ms(22500), UiStep::Page(None, true, false, 420.0)));
        steps.push((ms(23000), UiStep::Screenshot("settings-dark-report.png")));
        steps.push((ms(23500), UiStep::Close));
        steps.push((ms(23800), UiStep::Open(Theme::Light, Section::General)));
        steps.push((ms(24100), UiStep::Page(None, false, true, 380.0)));
        steps.push((ms(24600), UiStep::Screenshot("settings-light-remove.png")));
        steps.push((ms(25100), UiStep::Close));
        steps.push((ms(25400), UiStep::Exit));
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
    if let Err(e) = cmd.arg(path).spawn() {
        tracing::warn!(target: "clairlune", path = %path.display(), error = %e, "could not open");
    }
}

/// Opens a web page in the default browser.
fn open_url(url: &str) {
    if let Err(e) = clairlune_platform::open_url(url) {
        // The link may be long (a problem report): its start says enough.
        let start: String = url.chars().take(80).collect();
        tracing::warn!(target: "clairlune", url = %start, error = %e, "could not open");
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
        let restore = matches!(job, Job::Restore | Job::RestoreToRemove);
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

    /// Keeps the metronome at the live frame rate while live plays
    /// (Windows; elsewhere the event loop's own timer is enough).
    fn set_metronome(&self, playing: bool) {
        #[cfg(windows)]
        {
            let us = if playing {
                self.live.frame_interval().as_micros() as u64
            } else {
                0
            };
            let (interval, thread) = &self.metronome;
            if interval.swap(us, Ordering::Relaxed) == 0 && us != 0 {
                thread.unpark();
            }
        }
        #[cfg(not(windows))]
        let _ = playing;
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
            exact: false,
            settings: Box::new(self.settings.clone()),
            at,
            scrolling,
        });
    }

    /// The wallpaper a typed-in seed makes: from the archive or the library
    /// when it's there (exactly as it was shown), otherwise made again from
    /// the seed (the same scenes for the same Content choices and theme).
    fn wallpaper_from_seed(&mut self, seed: u64) {
        if self.timeline_mode() && !self.timeline_generate {
            self.manual_until = self.timeline_next;
        }
        let archived = self.archive().and_then(|a| {
            a.entries()
                .into_iter()
                .filter(|e| e.seed == seed)
                .max_by_key(|e| e.created)
                .map(|e| a.dir().join(e.name))
        });
        let saved = self.library().and_then(|lib| {
            lib.items()
                .into_iter()
                .find(|i| i.seed == seed)
                .map(|i| (lib.item_dir(&i.id), i))
        });
        if let Some(dir) = archived {
            self.current_theme = None;
            self.current_buddy = None;
            self.chosen = true;
            self.apply_folder(dir, None, None, false, false);
        } else if let Some((dir, item)) = saved {
            let (theme, buddy) = Self::library_choices(&item, &self.settings);
            self.current_theme = theme;
            self.current_buddy = buddy;
            self.chosen = true;
            self.apply_folder(dir, theme, buddy, false, false);
        } else {
            self.chosen = true;
            self.current_theme = None;
            self.current_buddy = None;
            let scrolling = self.live.scrolling_ok();
            self.submit(Job::New {
                seed,
                exact: true,
                settings: Box::new(self.settings.clone()),
                at: None,
                scrolling,
            });
        }
        self.reschedule();
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
                    exact: false,
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
            t.show_update(self.update.newer(), self.install.in_place());
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
        // Permissions may have been given or taken away meanwhile.
        if self.settings_window.is_some() {
            self.check_permissions();
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
            Action::About => {
                self.last_section = ui::view::Section::About;
                match &mut self.settings_window {
                    Some(w) => {
                        w.set_section(ui::view::Section::About);
                        w.focus();
                    }
                    None => self.open_settings(event_loop, None),
                }
            }
            Action::OpenUpdate => self.install_update(),
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
        self.check_permissions();
        self.status.ask_start_at_login = !self.settings.general.asked_start_at_login
            && self.status.start_at_login == Some(false)
            && self.ui_test.is_none();
        self.status.note = None;
        match ui::UiWindow::open(event_loop, ui::Kind::Settings, theme, self.last_section) {
            Ok(w) => {
                self.settings_window = Some(w);
                self.refresh_window_thumb();
                self.land_thumbs(theme);
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
                        sign: self.settings.export.sign,
                        dest,
                        source,
                        theme: apply::resolve_theme(&self.settings.theme),
                        buddy: self.settings.buddy.show,
                    });
                } else {
                    self.submit(Job::Export {
                        sign: self.settings.export.sign,
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
                        sign: self.settings.export.sign,
                        dest,
                        source,
                        theme: theme.unwrap_or_else(|| apply::resolve_theme(&self.settings.theme)),
                        buddy: buddy.unwrap_or(self.settings.buddy.show),
                    });
                } else {
                    self.submit(Job::Export {
                        sign: self.settings.export.sign,
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

    /// Loads the current wallpaper into the open Settings window: its
    /// thumbnail, and the main monitor's still behind the glass.
    fn refresh_window_thumb(&mut self) {
        let (Some(w), Some(dir)) = (&mut self.settings_window, &self.current) else {
            return;
        };
        let read = |name: &str| {
            std::fs::read(dir.join(name))
                .ok()
                .and_then(|b| ui::decode_png_rgba(&b))
        };
        if let Some((size, rgba)) = read("thumb.png") {
            w.set_thumb(size, &rgba);
        }
        if let Some((size, rgba)) = read("wallpaper-0.png").or_else(|| read("thumb.png")) {
            w.set_backdrop(size, &rgba);
        }
    }

    /// Points Settings at its landscape tiles for the theme it shows, and
    /// draws the missing ones on a thread of their own (once per generator
    /// version and theme; a second or two the first time).
    fn land_thumbs(&mut self, theme: Theme) {
        let dir = self.data_dir.join("thumbs").join(format!(
            "g{}-{}",
            clairlune_scene::GENERATOR_VERSION,
            theme.as_str()
        ));
        if let Some(w) = &mut self.settings_window {
            w.set_land_dir(dir.clone());
        }
        if self.drawing_lands {
            return;
        }
        let ids: Vec<&'static str> = clairlune_scene::archetype::ARCHETYPE_INFO
            .iter()
            .filter(|a| a.implemented)
            .map(|a| a.id)
            .filter(|id| !dir.join(format!("{id}.png")).exists())
            .collect();
        if ids.is_empty() {
            return;
        }
        self.drawing_lands = true;
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new()
            .name("clairlune-lands".into())
            .spawn(move || {
                for (i, id) in ids.iter().enumerate() {
                    if let Err(e) = apply::land_thumb(&dir, id, theme) {
                        tracing::warn!(target: "clairlune", landscape = id, error = %e, "landscape tile");
                    }
                    // Show them in batches as they come.
                    let last = i + 1 == ids.len();
                    if i % 6 == 5 || last {
                        let _ = proxy.send_event(AppEvent::LandsDrawn(last));
                    }
                }
            });
        if spawned.is_err() {
            self.drawing_lands = false;
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
        {
            if self.spaces.take_changed() && !self.busy {
                // The stills were set on the Space that was active then.
                self.live.reset_stills();
            }
            noticed |= self.spaces.take_screens_changed();
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
        // Lighter on Retina screens changes the scenes' size: the wallpaper
        // on screen is drawn again.
        if self.settings.live.lighter_on_retina != self.lighter {
            self.lighter = self.settings.live.lighter_on_retina;
            if self.current.is_some() && !self.restored() {
                self.reapply(false);
            }
        }
        let live = self.settings.display.mode == settings::WallpaperMode::Live;
        self.live.set_fps(self.settings.live.smoothness_fps);
        self.live.set_battery_policy(
            self.settings.live.battery_saver_on_battery,
            self.settings.live.pause_on_battery,
        );
        self.live
            .set_cover_pause(self.settings.live.pause_when_covered);
        self.live.set_still_monitors(self.settings.still_monitors());
        self.live.set_enabled(live);
        if let Some(t) = &self.tray {
            t.show_live(live);
        }
    }

    fn status(&self) -> ui::view::Status {
        let mut s = self.status.clone();
        s.monitors = self
            .screens
            .iter()
            .zip(tray::monitor_labels(&self.screens))
            .map(|(m, label)| (m.id.clone(), label))
            .collect();
        s.checking_updates = self.checking_updates.is_some();
        s.update_in_place = self.install.in_place();
        s.update_how = self.install.how().map(str::to_string);
        s.installing = self.installing.clone();
        s.report = report::details(&self.report_facts(), report::LOG_LINES);
        s.removal = Some(self.install.removal().map(str::to_string));
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
                self.check_permissions();
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
                if self.settings_window.is_some() {
                    self.land_thumbs(theme);
                }
                self.check();
            }
            UiAction::NewWallpaper => self.new_wallpaper_by_hand(),
            UiAction::FromSeed(seed) => self.wallpaper_from_seed(seed),
            UiAction::OpenLink(url) => open_url(url),
            UiAction::Resume => {
                self.settings.schedule.paused = false;
                if let Some(t) = &self.tray {
                    t.show_paused(false);
                }
                self.save_settings();
                self.reschedule();
            }
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
            UiAction::InstallUpdate => self.install_update(),
            UiAction::RemoveEverything => self.remove_everything(),
            UiAction::ReportProblem => {
                let url = report::issue_url(&self.report_facts());
                tracing::info!(target: "clairlune", "opening a problem report on GitHub");
                open_url(&url);
            }
            UiAction::OpenLoginItems => clairlune_platform::open_login_items_settings(),
            UiAction::SetStartAtLogin(on) => {
                let result = std::env::current_exe()
                    .map_err(|e| e.to_string())
                    .and_then(|exe| {
                        clairlune_platform::set_start_at_login(&exe, on)
                            .map_err(|e| e.to_string())
                            .map(|()| clairlune_platform::start_at_login(&exe))
                    });
                match result {
                    Ok(state) => {
                        self.status.start_at_login = state;
                        self.check_permissions();
                        // Switched off in Login Items: only the user can turn
                        // it on, there; take them to it.
                        if on && cfg!(target_os = "macos") && self.login_needs_approval() {
                            tracing::info!(target: "clairlune", "start at login needs approval in Login Items");
                            clairlune_platform::open_login_items_settings();
                            self.status.note = Some(
                                "Switch Clairlune on in System Settings > General > Login Items, which is open now, so it starts with your Mac."
                                    .into(),
                            );
                        }
                    }
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
                UiStep::Page(monitor, report, remove, scroll) => {
                    if let Some(w) = &mut self.settings_window {
                        let ps = w.page_state();
                        ps.content_monitor = monitor.map(str::to_string);
                        ps.report_open = report;
                        ps.remove_confirm = remove;
                        ps.scroll_to = Some(scroll);
                    }
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

    /// Downloads the newer version and installs it in place, on a thread;
    /// where this install can't update itself, opens the release page.
    fn install_update(&mut self) {
        if self.installing.is_some() {
            return;
        }
        if !self.install.in_place() {
            self.open_update();
            return;
        }
        let install = self.install.clone();
        let proxy = self.proxy.clone();
        self.installing = Some("Getting the new version\u{2026}".into());
        tracing::info!(target: "clairlune", install = %install.name(), "installing an update");
        let started = std::thread::Builder::new()
            .name("clairlune-install".into())
            .spawn(move || {
                let ready = (|| {
                    let release = update::fetch()?;
                    if !update::is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
                        return Err("this is already the newest version".to_string());
                    }
                    let name = install
                        .asset_name(&release.version)
                        .ok_or("nothing to download for this install")?;
                    let asset = release
                        .assets
                        .iter()
                        .find(|a| a.name == name)
                        .ok_or_else(|| format!("the release has no {name}"))?;
                    let v = release.version.clone();
                    let p = proxy.clone();
                    install::prepare(&install, asset, &release.version, &|share| {
                        let _ = p.send_event(AppEvent::Installing(InstallStep::Progress(
                            v.clone(),
                            share,
                        )));
                    })
                })();
                let _ = proxy.send_event(AppEvent::Installing(InstallStep::Ready(ready)));
            });
        if started.is_err() {
            self.installing = None;
        }
        self.redraw_settings();
    }

    fn on_installing(&mut self, step: InstallStep, event_loop: &ActiveEventLoop) {
        match step {
            InstallStep::Progress(version, share) => {
                self.installing = Some(if share >= 100 {
                    format!("Checking {version}\u{2026}")
                } else {
                    format!("Downloading {version}: {share}%")
                });
            }
            InstallStep::Ready(Ok(p)) => {
                // Back as it was: Settings again if it's open, else quietly.
                let relaunch: &[&str] = if self.settings_window.is_some() {
                    &[]
                } else {
                    &["--background"]
                };
                match install::finish(&p, std::process::id(), Some(relaunch)) {
                    Ok(()) => {
                        tracing::info!(target: "clairlune", version = %p.version, "update ready; quitting to install it");
                        self.installing = Some(format!("Installing {}\u{2026}", p.version));
                        event_loop.exit();
                    }
                    Err(e) => {
                        tracing::warn!(target: "clairlune", error = %e, "update not installed");
                        self.installing = None;
                        self.status.update_note = Some(format!("Couldn't install it: {e}"));
                    }
                }
            }
            InstallStep::Ready(Err(e)) => {
                tracing::warn!(target: "clairlune", error = %e, "update not installed");
                self.installing = None;
                self.status.update_note = Some(format!("Couldn't update: {e}"));
            }
        }
        self.redraw_settings();
    }

    /// Remove Clairlune: stops everything, then puts the old wallpaper back
    /// (on the worker); [`finish_removal`](Self::finish_removal) does the
    /// rest when that's done.
    fn remove_everything(&mut self) {
        if self.removing {
            return;
        }
        tracing::info!(target: "clairlune", install = %self.install.name(), "removing Clairlune");
        self.removing = true;
        self.next_change = None;
        self.live.show(Vec::new());
        if let Ok(exe) = std::env::current_exe()
            && let Err(e) = clairlune_platform::set_start_at_login(&exe, false)
        {
            tracing::warn!(target: "clairlune", error = %e, "start at login not turned off");
        }
        self.status.note = Some("Removing Clairlune\u{2026}".into());
        self.submit(Job::RestoreToRemove);
        self.redraw_settings();
    }

    /// The old wallpaper is back (or there was none to put back): the
    /// helper deletes Clairlune's folders and the app once it has quit.
    fn finish_removal(&mut self, event_loop: &ActiveEventLoop) {
        let mut dirs = vec![self.data_dir.clone()];
        if let Some(config) = self.settings_path.parent() {
            dirs.push(config.to_path_buf());
        }
        match install::remove_after_exit(&self.install, std::process::id(), &dirs) {
            Ok(()) => {
                tracing::info!(target: "clairlune", "removal under way; quitting");
                event_loop.exit();
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "removal not started");
                self.removing = false;
                self.status.note = Some(format!("Couldn't remove Clairlune: {e}"));
                self.redraw_settings();
            }
        }
    }

    /// A new copy of Clairlune was put where this one runs from (Homebrew
    /// upgraded it, a new DMG copy was dragged over): once the change has
    /// settled, restart into it, the way it runs now (Settings again if
    /// it's open, else quietly). Returns whether a change is being watched.
    fn watch_exe(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if self.ui_test.is_some() || self.installing.is_some() || self.removing {
            return false;
        }
        let Some(start) = &self.exe_stamp else {
            return false;
        };
        let now = install::exe_stamp();
        match (&self.exe_replaced, now) {
            // Mid-copy: look again.
            (_, None) => true,
            (_, Some(n)) if n == *start => {
                self.exe_replaced = None;
                false
            }
            // The same new copy as a moment ago: settled.
            (Some(seen), Some(n)) if *seen == n => {
                let args: &[&str] = if self.settings_window.is_some() {
                    &[]
                } else {
                    &[clairlune_platform::BACKGROUND_ARG]
                };
                match install::restart_after_exit(std::process::id(), args) {
                    Ok(()) => {
                        tracing::info!(target: "clairlune", "a new copy of Clairlune is in place; restarting into it");
                        event_loop.exit();
                    }
                    Err(e) => {
                        tracing::warn!(target: "clairlune", error = %e, "couldn't restart into the new copy");
                        self.exe_stamp = Some(n);
                        self.exe_replaced = None;
                    }
                }
                false
            }
            (_, Some(n)) => {
                tracing::info!(target: "clairlune", "the program file changed on disk; checking it settles");
                self.exe_replaced = Some(n);
                true
            }
        }
    }

    /// What the features in use need the user to allow in the system's
    /// settings (macOS, Windows), for Settings > General: start at login
    /// switched on in Login Items or Startup apps.
    fn check_permissions(&mut self) {
        use ui::view::{Permission, UiAction};
        let mut list = Vec::new();
        if cfg!(any(target_os = "macos", windows)) && self.status.start_at_login == Some(true) {
            let off = std::env::current_exe()
                .is_ok_and(|exe| clairlune_platform::start_at_login_needs_approval(&exe));
            list.push(Permission {
                name: "Start at login",
                granted: !off,
                open: UiAction::OpenLoginItems,
            });
        }
        // The UI test shows one still to allow.
        if self.ui_test.is_some() {
            list = vec![Permission {
                name: "Start at login",
                granted: false,
                open: UiAction::OpenLoginItems,
            }];
        }
        if list != self.status.permissions {
            self.status.permissions = list;
            self.redraw_settings();
        }
    }

    /// Whether start at login is switched off where only the user can
    /// switch it on (Login Items, Startup apps).
    fn login_needs_approval(&self) -> bool {
        cfg!(any(target_os = "macos", windows))
            && self.status.start_at_login == Some(true)
            && std::env::current_exe()
                .is_ok_and(|exe| clairlune_platform::start_at_login_needs_approval(&exe))
    }

    fn redraw_settings(&self) {
        if let Some(w) = &self.settings_window {
            w.request_redraw();
        }
    }

    /// What a problem report carries.
    fn report_facts(&self) -> report::Facts {
        use settings::WallpaperMode;
        let d = &self.settings.display;
        let wallpaper = match d.mode {
            WallpaperMode::Live => {
                let still = d.monitors.iter().filter(|m| m.still).count();
                format!(
                    "Live at {} fps, {}{}",
                    self.settings.live.smoothness_fps,
                    if d.span {
                        "spread across"
                    } else {
                        "one scene each"
                    },
                    if still > 0 {
                        format!(
                            ", {still} monitor{} kept still",
                            if still == 1 { "" } else { "s" }
                        )
                    } else {
                        String::new()
                    }
                )
            }
            WallpaperMode::Still => "Still frame".into(),
        };
        let showing = match self.status.seed {
            Some(seed) if !self.status.what.is_empty() => {
                format!("{}, seed {seed}", self.status.what)
            }
            _ => self.status.what.clone(),
        };
        report::Facts {
            install: self.install.name(),
            monitors: tray::monitor_labels(&self.screens),
            wallpaper,
            live: self.live.status.clone(),
            showing,
            crash_reports: self.status.crash_reports,
            log: diagnostics::recent_lines(report::LOG_LINES),
        }
    }

    /// The newer version's release page, in the browser.
    fn open_update(&self) {
        let url = self.update.url.as_deref().unwrap_or(update::RELEASES_PAGE);
        open_url(url);
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
            t.show_update(self.update.newer(), self.install.in_place());
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
            self.settings.display.mode == settings::WallpaperMode::Live,
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
            sign: self.settings.export.sign,
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
            sign: self.settings.export.sign,
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
                // One scene per monitor: each can get a new one. (A span is
                // one scene.)
                let span = live.iter().any(|s| s.view.is_some());
                self.screens = live.iter().map(|s| s.monitor.clone()).collect();
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
            // Handled where events arrive (it ends the app).
            Done::ReadyToRemove(_) => {}
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
                self.status.note = Some(
                    "Your wallpaper is back. Changes are paused until you resume them.".to_string(),
                );
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
            old_saver::remove(&self.data_dir);
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
        // Back from the system's settings: see what's allowed now.
        if matches!(event, WindowEvent::Focused(true))
            && self.settings_window.as_ref().is_some_and(|w| w.id() == id)
        {
            self.check_permissions();
        }
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
            AppEvent::LandsDrawn(done) => {
                if done {
                    self.drawing_lands = false;
                }
                if let Some(w) = &mut self.settings_window {
                    w.refresh_lands();
                }
            }
            AppEvent::Menu(e) => {
                if let Some(action) = self.tray.as_ref().and_then(|t| t.action(e.id())) {
                    tracing::debug!(target: "clairlune", ?action, "menu");
                    self.on_action(action, event_loop);
                }
            }
            AppEvent::Done(Done::ReadyToRemove(error)) => {
                self.busy = false;
                if let Some(e) = error {
                    tracing::info!(target: "clairlune", error = %e, "no earlier wallpaper put back");
                }
                self.finish_removal(event_loop);
            }
            AppEvent::Done(d) => self.on_done(d),
            AppEvent::OpenSettings => self.open_settings(event_loop, None),
            AppEvent::UpdateChecked(r) => self.on_update_checked(r),
            AppEvent::Installing(step) => self.on_installing(step, event_loop),
            #[cfg(windows)]
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
            if self.watch_exe(event_loop) {
                // Look again soon, once a copy being put in place settles.
                self.next_check = now + Duration::from_secs(3);
            }
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
/// builds.
fn init_logging(data_dir: &Path) {
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
    // Written as it comes, on the thread that logs: a few lines a minute
    // don't need a writer thread of their own.
    let file_layer = appender
        .ok()
        .map(|a| fmt::layer().with_writer(a).with_ansi(false));
    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));
    tracing_subscriber::registry()
        .with(filter)
        .with(diagnostics::Tail)
        .with(file_layer)
        .with(stderr_layer)
        .init();
    diagnostics::install_crash_reports(data_dir);
}

/// `clairlune --remove-everything` (for CI): Remove Clairlune as Settings
/// does it, without the app: the old wallpaper back, start at login off,
/// then the helper deletes the folders and the app.
/// Returns the exit code.
fn remove_now(data_dir: &Path) -> i32 {
    let install = install::detect();
    eprintln!("install: {}", install.name());
    let host = clairlune_platform::detect().ok();
    match clairlune_platform::StillStore::default_location()
        .map_err(|e| e.to_string())
        .and_then(|store| apply::restore(host.as_deref(), &store))
    {
        Ok(_) => eprintln!("the wallpaper from before is back"),
        Err(e) => eprintln!("no wallpaper put back: {e}"),
    }
    if let Ok(exe) = std::env::current_exe()
        && let Err(e) = clairlune_platform::set_start_at_login(&exe, false)
    {
        eprintln!("start at login not turned off: {e}");
    }
    let mut dirs = vec![data_dir.to_path_buf()];
    if let Some(config) = clairlune_platform::app_config_dir() {
        dirs.push(config);
    }
    match install::remove_after_exit(&install, std::process::id(), &dirs) {
        Ok(()) => {
            eprintln!("removing {} once this exits", dirs.len());
            0
        }
        Err(e) => {
            eprintln!("couldn't start the removal: {e}");
            1
        }
    }
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
    init_logging(&data_dir);
    // CI's check of in-place updates: install the latest release over this
    // copy and exit (install::install_latest_now).
    if args.iter().any(|a| a == "--install-latest") {
        #[cfg(windows)]
        clairlune_platform::attach_parent_console();
        std::process::exit(install::install_latest_now());
    }
    // CI's check of Remove Clairlune: the same steps, without the app.
    if args.iter().any(|a| a == "--remove-everything") {
        #[cfg(windows)]
        clairlune_platform::attach_parent_console();
        std::process::exit(remove_now(&data_dir));
    }
    // Asked once, off the main thread, for problem reports.
    let _ = std::thread::Builder::new()
        .name("clairlune-system".into())
        .spawn(|| {
            let _ = report::system();
        });
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
    #[allow(unused_mut)]
    let mut settings = Settings::load(&settings_path);
    // Live became the default on the Mac in 0.17: a copy still on the old
    // default moves to it, once.
    #[cfg(target_os = "macos")]
    let moved_to_live = {
        let was_moved = settings.general.mac_live_moved;
        let moved = ui_test_out.is_none() && settings.move_to_live_once();
        if settings.general.mac_live_moved != was_moved
            && let Err(e) = settings.save(&settings_path)
        {
            tracing::warn!(target: "clairlune", error = %e, "settings not saved");
        }
        if moved {
            tracing::info!(target: "clairlune", "moved to Live, the default on the Mac now");
        }
        moved
    };
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
    live.set_still_monitors(settings.still_monitors());
    #[cfg(windows)]
    let watcher = match clairlune_platform::windows_live::SystemWatcher::new() {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::warn!(target: "clairlune", error = %e, "no display-change watcher");
            None
        }
    };
    // The live metronome (Windows): a LiveFrame event every frame while
    // live plays; parked otherwise.
    #[cfg(windows)]
    let metronome = {
        let interval = Arc::new(AtomicU64::new(0));
        let (m, p) = (interval.clone(), metronome_proxy);
        let spawned = std::thread::Builder::new()
            .name("clairlune-live-metronome".into())
            .spawn(move || {
                loop {
                    let us = m.load(Ordering::Relaxed);
                    if us == 0 {
                        std::thread::park();
                        continue;
                    }
                    std::thread::sleep(Duration::from_micros(us));
                    if p.send_event(AppEvent::LiveFrame).is_err() {
                        return; // the event loop is gone
                    }
                }
            });
        match spawned {
            Ok(handle) => (interval, handle.thread().clone()),
            // No metronome: winit's timers still drive frames.
            Err(_) => (interval, std::thread::current()),
        }
    };
    #[cfg(not(windows))]
    drop(metronome_proxy);
    let lighter = settings.live.lighter_on_retina;
    let mut app = App {
        live,
        #[cfg(windows)]
        metronome,
        #[cfg(windows)]
        watcher,
        #[cfg(target_os = "macos")]
        spaces: clairlune_platform::macos::SpaceWatcher::new(),
        monitor_layout: None,
        monitors_changed_at: None,
        settings_stamp: file_stamp(&settings_path),
        settings,
        settings_path,
        data_dir,
        jobs,
        proxy: app_proxy,
        drawing_lands: false,
        update: update_state,
        checking_updates: None,
        install: install::detect(),
        installing: None,
        removing: false,
        exe_stamp: install::exe_stamp(),
        exe_replaced: None,
        lighter,
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
        taskbar_light: clairlune_platform::taskbar_is_light(),
        tray_monitors: Vec::new(),
        screens: Vec::new(),
    };
    #[cfg(target_os = "macos")]
    if moved_to_live {
        app.status.note = Some(
            "Clairlune plays live on the Mac now: Clawd moves, fires flicker, snow falls. Settings > Display switches back to a still frame."
                .into(),
        );
    }
    if app.ui_test.is_some() {
        let m = |id: &str, x: i32, w: u32, h: u32, primary: bool| clairlune_platform::Monitor {
            id: id.into(),
            x,
            y: 0,
            width: w,
            height: h,
            primary,
        };
        app.screens = vec![
            m("ui-test-1", 0, 2560, 1440, true),
            m("ui-test-2", 2560, 1920, 1080, false),
        ];
        app.settings.display.mode = settings::WallpaperMode::Live;
        // The right one: its own choices, and the still frame.
        app.settings
            .edit_monitor("ui-test-2", "Right (1920x1080)", |m| {
                m.still = true;
                m.content = Some(settings::Content {
                    archetypes: vec!["lake".into(), "islands".into()],
                    ..settings::Content::default()
                });
            });
    }
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
