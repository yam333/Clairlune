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
mod icon;
mod ipc;
mod settings;
mod timefmt;
mod tokens;
#[cfg(any(windows, target_os = "macos"))]
mod tray;
mod ui;

use apply::{Done, Job};
use clairlune_scene::{APP_NAME, Theme};
use settings::Settings;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

/// How often to look for settings edits and system theme changes.
const CHECK_EVERY: Duration = Duration::from_secs(20);

enum AppEvent {
    #[cfg(any(windows, target_os = "macos"))]
    Menu(tray_icon::menu::MenuEvent),
    Done(Done),
    /// A second launch asked for Settings.
    OpenSettings,
}

struct App {
    settings: Settings,
    settings_path: PathBuf,
    settings_stamp: Option<SystemTime>,
    data_dir: PathBuf,
    jobs: mpsc::Sender<Job>,
    busy: bool,
    /// When the schedule next changes the wallpaper (None: only on demand).
    next_change: Option<Instant>,
    next_check: Instant,
    /// The theme the current wallpaper was rendered for.
    shown_theme: Option<Theme>,
    /// The archive entry on screen.
    current: Option<String>,
    /// What's on screen, for the Settings window.
    status: ui::view::Status,
    #[cfg(any(windows, target_os = "macos"))]
    tray: Option<tray::Tray>,
    /// The Settings window, while open, and the section it last showed.
    settings_window: Option<ui::SettingsWindow>,
    last_section: ui::view::Section,
    /// `--ui-test`: scripted open/close cycles with screenshots, then exit.
    ui_test: Option<UiTest>,
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
    Screenshot(&'static str),
    Close,
    Exit,
}

impl UiTest {
    fn new(out: PathBuf) -> Self {
        use ui::view::Section;
        let ms = Duration::from_millis;
        let mut steps = Vec::new();
        let cycles = [
            (Theme::Dark, Section::Schedule, "settings-dark-schedule.png"),
            (Theme::Light, Section::Content, "settings-light-content.png"),
            (Theme::Dark, Section::General, "settings-dark-general.png"),
        ];
        for (i, (theme, section, shot)) in cycles.into_iter().enumerate() {
            let t0 = 1500 * i as u64;
            steps.push((ms(t0), UiStep::Open(theme, section)));
            steps.push((ms(t0 + 700), UiStep::Screenshot(shot)));
            steps.push((ms(t0 + 1200), UiStep::Close));
        }
        steps.push((ms(4800), UiStep::Exit));
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

impl App {
    fn interval(&self) -> Option<Duration> {
        let m = self.settings.schedule.change_every_minutes;
        (m > 0 && !self.settings.schedule.paused).then(|| Duration::from_secs(u64::from(m) * 60))
    }

    fn reschedule(&mut self) {
        self.next_change = self.interval().map(|d| Instant::now() + d);
    }

    fn submit(&mut self, job: Job) {
        if self.busy && !matches!(job, Job::Export { .. }) {
            return;
        }
        if !matches!(job, Job::Export { .. }) {
            self.busy = true;
            #[cfg(any(windows, target_os = "macos"))]
            if let Some(t) = &self.tray {
                t.set_busy(true);
            }
        }
        if self.jobs.send(job).is_err() {
            tracing::error!(target: "clairlune", "worker thread is gone");
        }
    }

    fn new_wallpaper(&mut self) {
        let seed = random_seed();
        self.submit(Job::New {
            seed,
            settings: Box::new(self.settings.clone()),
        });
    }

    fn archive(&self) -> Option<archive::Archive> {
        archive::Archive::open(self.data_dir.join("archive")).ok()
    }

    /// Re-applies saved recipes: the previous entry, or the current one again
    /// (for a theme change).
    fn reapply(&mut self, previous: bool) {
        let Some(archive) = self.archive() else {
            return;
        };
        let target = match (&self.current, previous) {
            (Some(cur), true) => archive.before(cur),
            (None, true) => archive.entries().into_iter().rev().nth(1),
            (Some(cur), false) => archive.entries().into_iter().find(|e| &e.name == cur),
            (None, false) => archive.latest(),
        };
        let Some(entry) = target else {
            return;
        };
        match archive.recipes(&entry) {
            Ok(recipes) => self.submit(Job::Recipes {
                recipes: Box::new(recipes),
                settings: Box::new(self.settings.clone()),
                archive: previous,
            }),
            Err(e) => {
                tracing::warn!(target: "clairlune", entry = %entry.name, error = %e, "unreadable archive entry")
            }
        }
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            tracing::warn!(target: "clairlune", error = %e, "could not save settings");
        }
        self.settings_stamp = file_stamp(&self.settings_path);
    }

    /// Picks up edits to settings.toml and system theme changes.
    fn check(&mut self) {
        let stamp = file_stamp(&self.settings_path);
        if stamp != self.settings_stamp {
            self.settings_stamp = stamp;
            let new = Settings::load(&self.settings_path);
            if new != self.settings {
                tracing::info!(target: "clairlune", "settings changed on disk");
                let schedule_changed = new.schedule != self.settings.schedule;
                self.settings = new;
                #[cfg(any(windows, target_os = "macos"))]
                if let Some(t) = &self.tray {
                    t.show_theme(self.settings.theme.mode);
                    t.show_paused(self.settings.schedule.paused);
                }
                if schedule_changed {
                    self.reschedule();
                }
            }
        }
        let theme = apply::resolve_theme(self.settings.theme.mode);
        if self.shown_theme.is_some_and(|t| t != theme) && !self.busy {
            tracing::info!(target: "clairlune", theme = theme.as_str(), "theme changed; re-rendering");
            self.reapply(false);
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn on_action(&mut self, action: tray::Action, event_loop: &ActiveEventLoop) {
        use tray::Action;
        match action {
            Action::NewWallpaper => {
                self.new_wallpaper();
                self.reschedule();
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
            Action::TogglePause => {
                self.settings.schedule.paused = !self.settings.schedule.paused;
                if let Some(t) = &self.tray {
                    t.show_paused(self.settings.schedule.paused);
                }
                self.save_settings();
                self.reschedule();
            }
            Action::SavePng => {
                let dest = self.export_dir();
                self.submit(Job::Export { dest });
            }
            Action::OpenArchive => open(&self.data_dir.join("archive"), false),
            Action::OpenSettings => self.open_settings(event_loop, None),
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
        let theme = theme.unwrap_or_else(|| apply::resolve_theme(self.settings.theme.mode));
        self.status.start_at_login = std::env::current_exe()
            .ok()
            .and_then(|exe| clairlune_platform::start_at_login(&exe));
        self.status.note = None;
        match ui::SettingsWindow::open(event_loop, theme, self.last_section) {
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

    /// Loads the current wallpaper's thumbnail into the open Settings window.
    fn refresh_window_thumb(&mut self) {
        let (Some(w), Some(name)) = (&mut self.settings_window, &self.current) else {
            return;
        };
        let path = self.data_dir.join("archive").join(name).join("thumb.png");
        if let Some((size, rgba)) = std::fs::read(&path)
            .ok()
            .and_then(|b| ui::decode_png_rgba(&b))
        {
            w.set_thumb(size, &rgba);
        }
    }

    fn status(&self) -> ui::view::Status {
        let mut s = self.status.clone();
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
                #[cfg(any(windows, target_os = "macos"))]
                if let Some(t) = &self.tray {
                    t.show_theme(self.settings.theme.mode);
                    t.show_paused(self.settings.schedule.paused);
                }
                self.reschedule();
                let theme = apply::resolve_theme(self.settings.theme.mode);
                if let Some(w) = &mut self.settings_window {
                    w.set_theme(theme);
                }
                self.check();
            }
            UiAction::NewWallpaper => {
                self.new_wallpaper();
                self.reschedule();
            }
            UiAction::Previous => self.reapply(true),
            UiAction::OpenArchive => open(&self.data_dir.join("archive"), false),
            UiAction::OpenLogs => open(&self.data_dir.join("logs"), false),
            UiAction::SavePng => {
                let dest = self.export_dir();
                self.submit(Job::Export { dest });
            }
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
                #[cfg(any(windows, target_os = "macos"))]
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
                    }
                    tracing::info!(target: "clairlune", ms = opened.elapsed().as_millis() as u64, "ui test: settings opened");
                }
                UiStep::Screenshot(name) => {
                    let out = self.ui_test.as_ref().map(|t| t.out.join(name));
                    if let (Some(w), Some(out)) = (&mut self.settings_window, out) {
                        w.screenshot_to = Some(out);
                        w.request_redraw();
                    }
                }
                UiStep::Close => {
                    self.settings_window = None;
                    tracing::info!(target: "clairlune", "ui test: settings closed");
                }
                UiStep::Exit => {
                    tracing::info!(target: "clairlune", "ui test passed: settings opened and closed 3 times on one loop");
                    event_loop.exit();
                }
            }
        }
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
                entry_name,
                seed,
                theme,
                archetype,
                activity,
                monitors,
                ms,
            } => {
                self.busy = false;
                self.current = Some(entry_name);
                self.shown_theme = Some(theme);
                let what = match activity {
                    Some(a) => {
                        format!("{} in {}", a.replace('_', " "), archetype.replace('_', " "))
                    }
                    None => archetype.replace('_', " "),
                };
                let text = format!("{APP_NAME}: {what} (seed {seed})");
                tracing::debug!(target: "clairlune", monitors, ms, "{text}");
                self.status.what = what;
                self.status.seed = Some(seed);
                self.status.theme = Some(theme);
                self.refresh_window_thumb();
                #[cfg(any(windows, target_os = "macos"))]
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
            Done::Restored => {
                self.busy = false;
                self.current = None;
                self.status.what.clear();
                self.status.seed = None;
                self.status.theme = None;
                self.status.note =
                    Some("Your wallpaper is back. The schedule is paused.".to_string());
                if let Some(w) = &mut self.settings_window {
                    w.clear_thumb();
                }
                #[cfg(any(windows, target_os = "macos"))]
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(APP_NAME);
                }
            }
            Done::Failed(e) => {
                self.busy = false;
                tracing::debug!(target: "clairlune", error = %e, "shown in the tray tooltip");
                self.status.note = Some(capitalise_first(&e));
                #[cfg(any(windows, target_os = "macos"))]
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(&format!("{APP_NAME}: {e}"));
                }
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause != StartCause::Init {
            return;
        }
        // macOS wants the tray created once the loop runs.
        #[cfg(any(windows, target_os = "macos"))]
        {
            match tray::Tray::new(self.settings.theme.mode, self.settings.schedule.paused) {
                Ok(t) => self.tray = Some(t),
                Err(e) => tracing::error!(target: "clairlune", error = %e, "no tray icon"),
            }
        }
        let latest = self.archive().and_then(|a| a.latest());
        self.current = latest.as_ref().map(|e| e.name.clone());
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
            return; // UI test: leave the wallpaper alone
        }
        self.shown_theme = latest.and_then(|e| match e.theme.as_str() {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        });
        if self.settings.schedule.change_at_login || self.current.is_none() {
            self.new_wallpaper();
        }
        self.reschedule();
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let status = self.status();
        let mut actions = Vec::new();
        let Some(w) = self.settings_window.as_mut().filter(|w| w.id() == id) else {
            return;
        };
        let before = self.settings.clone();
        let outcome = w.on_event(&event, &mut self.settings, &status, &mut actions);
        self.last_section = w.section();
        if let ui::Outcome::Close = outcome {
            self.settings_window = None;
            tracing::debug!(target: "clairlune", "settings closed");
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
            #[cfg(any(windows, target_os = "macos"))]
            AppEvent::Menu(e) => {
                if let Some(action) = self.tray.as_ref().and_then(|t| t.action(e.id())) {
                    tracing::debug!(target: "clairlune", ?action, "menu");
                    self.on_action(action, event_loop);
                }
            }
            AppEvent::Done(d) => self.on_done(d),
            AppEvent::OpenSettings => self.open_settings(event_loop, None),
        }
        let _ = event_loop;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.run_ui_test(event_loop);
        let now = Instant::now();
        if self.next_change.is_some_and(|t| now >= t) && !self.busy {
            self.new_wallpaper();
            self.reschedule();
        }
        if now >= self.next_check {
            self.check();
            self.next_check = now + CHECK_EVERY;
        }
        let mut wake = match self.next_change {
            Some(t) => t.min(self.next_check),
            None => self.next_check,
        };
        if let Some(t) = self.ui_test.as_ref().and_then(|t| t.wake()) {
            wake = wake.min(t);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        tracing::info!(target: "clairlune", "quitting");
    }
}

/// Logs to a daily file in `<data>/logs/` (seven kept), and to stderr in debug
/// builds. The guard flushes the file writer when dropped.
fn init_logging(data_dir: &Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, fmt};
    let filter =
        EnvFilter::try_from_env("CLAIRLUNE_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    // The appender scans the folder for old files to prune, so it must exist.
    let _ = std::fs::create_dir_all(data_dir.join("logs"));
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
        .with(file_layer)
        .with(stderr_layer)
        .init();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(target: "clairlune", panic = %info, backtrace = %std::backtrace::Backtrace::force_capture(), "panic");
        default_hook(info);
    }));
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

fn main() {
    clairlune_platform::init_dpi_awareness();
    // `--ui-test <dir>`: the scripted Settings check, in a scratch data folder,
    // leaving the wallpaper alone.
    let args: Vec<String> = std::env::args().collect();
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
    let _instance = match single_instance(&data_dir) {
        Ok(f) => f,
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
    tracing::info!(
        target: "clairlune",
        version = env!("CARGO_PKG_VERSION"),
        generator_version = clairlune_scene::GENERATOR_VERSION,
        os = std::env::consts::OS,
        settings = %settings_path.display(),
        data = %data_dir.display(),
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
    #[cfg(any(windows, target_os = "macos"))]
    {
        let p = proxy.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(move |e| {
            let _ = p.send_event(AppEvent::Menu(e));
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
    let mut app = App {
        settings_stamp: file_stamp(&settings_path),
        settings,
        settings_path,
        data_dir,
        jobs,
        busy: false,
        next_change: None,
        next_check: Instant::now() + CHECK_EVERY,
        shown_theme: None,
        current: None,
        #[cfg(any(windows, target_os = "macos"))]
        tray: None,
        settings_window: None,
        last_section: ui::view::Section::Schedule,
        status: ui::view::Status::default(),
        ui_test: ui_test_out.map(UiTest::new),
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
    fn theme_mode_resolves() {
        assert_eq!(apply::resolve_theme(ThemeMode::Light), Theme::Light);
        assert_eq!(apply::resolve_theme(ThemeMode::Dark), Theme::Dark);
        let _ = apply::resolve_theme(ThemeMode::FollowSystem);
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
