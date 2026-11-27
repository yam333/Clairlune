//! The event loop's callbacks: windows, the menu, the worker, timers.

use crate::*;

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
        // Settings is its only face and closing it quits. And for the first
        // run's question.
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
            self.asked_theme = None;
            self.status.what.clear();
            self.status.seed = None;
        } else if self.timeline_mode() {
            // The timeline's active slot, right away (or keep playing a paused
            // live wallpaper across restarts).
            self.check_timeline();
            if paused
                && self.current.is_some()
                && self.settings.display.mode == settings::WallpaperMode::Live
            {
                self.reapply(false);
            }
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
            &mut self.ascii_window,
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
                        && self.ascii_window.is_none()
                    {
                        event_loop.exit();
                    }
                }
                ui::Kind::Library => self.library_window = None,
                ui::Kind::Timeline => self.timeline_window = None,
                ui::Kind::Archive => self.archive_window = None,
                ui::Kind::Ascii => self.ascii_window = None,
            }
            self.trim_if_idle();
        }
        if self.settings == before {
            actions.retain(|a| *a != ui::view::UiAction::SettingsChanged);
        } else if self.settings.display.mode != before.display.mode {
            self.current_live = None;
        }
        for a in actions {
            self.on_ui_action(a, event_loop);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::FolderPicked(picked) => {
                self.picking_folder = false;
                if let Some(dir) = picked {
                    self.settings.export.folder = dir.display().to_string();
                    self.save_settings();
                    self.redraw_windows();
                }
            }
            AppEvent::AsciiImagePicked(picked) => {
                self.picking_ascii_image = false;
                if let Some(path) = picked {
                    self.load_ascii_file(&path);
                }
            }
            AppEvent::LandsDrawn(done) => {
                if let Some(w) = &mut self.settings_window {
                    w.refresh_lands();
                }
                if done {
                    self.drawing_lands = false;
                    if let Some(theme) = self.lands_pending.take() {
                        self.land_thumbs(theme);
                    } else {
                        self.trim_if_idle();
                    }
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
        clairlune_platform::with_autoreleasepool(|| {
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
            #[cfg(target_os = "macos")]
            if self.live.menu_bars.appearance_due(now) {
                let dark = self
                    .tray
                    .as_ref()
                    .and_then(crate::tray::Tray::menu_bar_dark);
                self.live.menu_bars.see(now, dark);
            }
            let live_next = self.live.tick(Instant::now());
            self.set_metronome(live_next.is_some());
            let mut wake = self.next_check;
            if !self.busy {
                if let Some(t) = self.next_change {
                    wake = wake.min(t);
                }
                if let Some(t) = self.monitors_changed_at {
                    wake = wake.min(t + MONITOR_SETTLE);
                }
            }
            if let Some(t) = live_next {
                wake = wake.min(t);
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
            // Windows with an animation running get their next frame on time.
            let now = Instant::now();
            for w in [
                &mut self.settings_window,
                &mut self.library_window,
                &mut self.timeline_window,
                &mut self.archive_window,
                &mut self.ascii_window,
            ]
            .into_iter()
            .flatten()
            {
                if let Some(t) = w.redraw_due(now) {
                    wake = wake.min(t);
                }
            }
            // Coalesce sub-millisecond timer slop and ensure every WaitUntil
            // hands winit a distinct future Instant so macOS's CFRunLoopTimer
            // resets its fire date instead of repeating at 0.1 us.
            const MIN_WAIT: Duration = Duration::from_millis(1);
            let floor = Instant::now() + MIN_WAIT;
            if wake < floor {
                wake = floor;
            }
            if self.last_wake == Some(wake) {
                wake += Duration::from_nanos(1);
            }
            self.last_wake = Some(wake);
            event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
        });
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        tracing::info!(target: "clairlune", "quitting");
    }
}
