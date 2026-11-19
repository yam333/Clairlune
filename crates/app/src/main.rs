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
mod settings;
mod timefmt;
mod tokens;
#[cfg(any(windows, target_os = "macos"))]
mod tray;

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
    #[cfg(any(windows, target_os = "macos"))]
    tray: Option<tray::Tray>,
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

    #[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))] // tray-only until the Linux tray lands
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
                let dest = if self.settings.export.folder.is_empty() {
                    clairlune_platform::pictures_dir()
                        .unwrap_or_else(|| self.data_dir.clone())
                        .join(APP_NAME)
                } else {
                    PathBuf::from(&self.settings.export.folder)
                };
                self.submit(Job::Export { dest });
            }
            Action::OpenArchive => open(&self.data_dir.join("archive"), false),
            Action::OpenSettings => {
                if !self.settings_path.exists() {
                    self.save_settings();
                }
                open(&self.settings_path, true);
            }
            Action::Quit => event_loop.exit(),
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
                #[cfg(any(windows, target_os = "macos"))]
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(&text);
                }
            }
            Done::Exported(path) => {
                tracing::info!(target: "clairlune", path = %path.display(), "saved");
                if let Some(dir) = path.parent() {
                    open(dir, false);
                }
            }
            Done::Failed(e) => {
                self.busy = false;
                tracing::debug!(target: "clairlune", error = %e, "shown in the tray tooltip");
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

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}

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
        }
        let _ = event_loop;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self.next_change.is_some_and(|t| now >= t) && !self.busy {
            self.new_wallpaper();
            self.reschedule();
        }
        if now >= self.next_check {
            self.check();
            self.next_check = now + CHECK_EVERY;
        }
        let wake = match self.next_change {
            Some(t) => t.min(self.next_check),
            None => self.next_check,
        };
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

fn main() {
    clairlune_platform::init_dpi_awareness();
    let Some(data_dir) = clairlune_platform::app_data_dir() else {
        eprintln!("{APP_NAME}: no data directory");
        return;
    };
    let _ = std::fs::create_dir_all(&data_dir);
    let _log = init_logging(&data_dir);
    let _instance = match single_instance(&data_dir) {
        Ok(f) => f,
        Err(e) => {
            tracing::info!(target: "clairlune", reason = %e, "another copy is running; exiting");
            return;
        }
    };
    let settings_path = clairlune_platform::app_config_dir()
        .unwrap_or_else(|| data_dir.clone())
        .join("settings.toml");
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

    let (jobs, rx) = mpsc::channel();
    let archive_dir = data_dir.join("archive");
    let worker = std::thread::Builder::new()
        .name("clairlune-worker".into())
        .spawn(move || {
            apply::worker(
                rx,
                move |d| {
                    let _ = proxy.send_event(AppEvent::Done(d));
                },
                archive_dir,
            )
        });
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
