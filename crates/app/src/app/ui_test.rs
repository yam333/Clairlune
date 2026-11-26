//! `--ui-test`: every window opened with sample data in a scratch folder,
//! screenshots saved, then quit. It never touches the wallpaper.

use crate::*;

/// The M6 toolkit spike's pass criterion, scripted: open and close Settings
/// three times on the one event loop (alongside the tray), screenshot each
/// opening in a different theme and section, then quit.
pub(crate) struct UiTest {
    out: PathBuf,
    steps: Vec<(Duration, UiStep)>,
    started: Instant,
    next: usize,
}

#[derive(Clone, Copy)]
pub(crate) enum UiStep {
    Open(Theme, ui::view::Section),
    /// Settings with the first run's question over it.
    OpenAsking(Theme),
    /// The tray icon hidden or shown again, as the setting does it.
    Tray(bool),
    /// Spread across on or off (the arrangement shows on Display).
    Span(bool),
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
    pub(crate) fn new(out: PathBuf) -> Self {
        use ui::view::Section;
        let ms = Duration::from_millis;
        let mut steps = Vec::new();
        let cycles = [
            (Theme::Dark, Section::Rotation, "settings-dark-rotation.png"),
            (Theme::Light, Section::About, "settings-light-about.png"),
            (Theme::Dark, Section::Export, "settings-dark-export.png"),
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
            (Theme::Dark, 0, usize::MAX, "timeline-dark.png"),
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
        steps.push((ms(13400), UiStep::Open(Theme::Light, Section::Display)));
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
        // Spread across: the monitors' arrangement.
        steps.push((ms(25400), UiStep::Span(true)));
        steps.push((ms(25600), UiStep::Open(Theme::Dark, Section::Display)));
        steps.push((ms(25900), UiStep::Page(None, false, false, 320.0)));
        steps.push((
            ms(26400),
            UiStep::Screenshot("settings-dark-arrangement.png"),
        ));
        steps.push((ms(26800), UiStep::Close));
        steps.push((ms(27000), UiStep::Span(false)));
        // Content's palettes and letters, near the foot of the page.
        steps.push((ms(27300), UiStep::Open(Theme::Dark, Section::Content)));
        steps.push((ms(27600), UiStep::Page(None, false, false, 9000.0)));
        steps.push((ms(28300), UiStep::Screenshot("settings-dark-colours.png")));
        steps.push((ms(28800), UiStep::Close));
        steps.push((ms(29100), UiStep::Exit));
        UiTest {
            out,
            steps,
            started: Instant::now(),
            next: 0,
        }
    }

    pub(crate) fn due(&mut self) -> Option<UiStep> {
        let (at, step) = *self.steps.get(self.next)?;
        (self.started.elapsed() >= at).then(|| {
            self.next += 1;
            step
        })
    }

    pub(crate) fn wake(&self) -> Option<Instant> {
        self.steps.get(self.next).map(|(at, _)| self.started + *at)
    }
}

impl App {
    pub(crate) fn run_ui_test(&mut self, event_loop: &ActiveEventLoop) {
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
                UiStep::Span(on) => {
                    self.settings.display.span = on;
                    // Shown as if the wallpaper on screen were a spread.
                    self.span_shown = on;
                    self.redraw_windows();
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
                        // usize::MAX: nothing selected (the demo shows).
                        if index != usize::MAX {
                            w.select_slot(day, index);
                        }
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
}

/// `--ui-test`: three archived scenes in the scratch folder, two of them
/// saved to the library, and a weekly timeline using them, so the windows
/// show real content.
pub(crate) fn seed_ui_test(data_dir: &Path) -> Result<(), String> {
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
