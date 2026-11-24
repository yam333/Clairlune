// No console window for the tray app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! The Clairlune desktop app: a tray icon that sets a new procedural wallpaper
//! on a schedule, one scene per monitor, keeping an archive of past ones.
//!
//! One event loop for the whole process: the tray, and
//! later the live wallpaper windows and the UI windows, all live on it. Work
//! that takes time (rendering, archiving) runs on a worker thread and reports
//! back as a user event.

mod app;
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
mod span;
mod sun;
mod timefmt;
mod timeline;
mod tokens;
mod tray;
mod ui;
mod update;

use app::ui_test::{UiTest, seed_ui_test};
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
    /// The folder picked for saved images (`None`: the picker was closed).
    FolderPicked(Option<PathBuf>),
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
    /// The folder picker is open.
    picking_folder: bool,
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
    /// What a spread scene depends on in Settings, as the wallpaper on
    /// screen was made (`span::settings_key`).
    span_key: String,
    /// The wallpaper on screen is one scene spread across the monitors.
    span_shown: bool,
    /// The monitors the wallpaper is on (for Settings: each one's own
    /// settings).
    screens: Vec<clairlune_platform::Monitor>,
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
    // The new version's icon, where the system remembers the old one's. (Off
    // the main thread: it waits for Explorer.)
    if ui_test_out.is_none() && install::first_run_of(env!("CARGO_PKG_VERSION"), &data_dir) {
        let _ = std::thread::Builder::new()
            .name("clairlune-icons".into())
            .spawn(clairlune_platform::refresh_icons);
    }

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
    let span_key = span::settings_key(&settings.display);
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
        picking_folder: false,
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
        span_key,
        span_shown: false,
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
        let m = |id: &str, x: i32, y: i32, w: u32, h: u32, primary: bool, scale: u32| {
            clairlune_platform::Monitor {
                id: id.into(),
                x,
                y,
                width: w,
                height: h,
                primary,
                scale,
            }
        };
        // A main monitor, one to its right, and a Retina laptop under the
        // main one (in points, as macOS arranges them).
        app.screens = vec![
            m("ui-test-1", 0, 0, 2560, 1440, true, 1),
            m("ui-test-2", 2560, 0, 1920, 1080, false, 1),
            m("ui-test-3", 524, 1440, 3024, 1964, false, 2),
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
