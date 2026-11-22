//! The periodic check, monitors, the tray, permissions, settings on disk
//! and the usage log.

use crate::*;

impl App {
    /// Keeps the metronome at the live frame rate while live plays
    /// (Windows; elsewhere the event loop's own timer is enough).
    pub(crate) fn set_metronome(&self, playing: bool) {
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
    pub(crate) fn set_restored(&self, on: bool) {
        let path = self.data_dir.join("restored");
        let _ = if on {
            std::fs::write(&path, b"")
        } else {
            std::fs::remove_file(&path)
        };
    }

    pub(crate) fn restored(&self) -> bool {
        self.data_dir.join("restored").exists()
    }

    pub(crate) fn save_settings(&mut self) {
        if let Err(e) = self.settings.save(&self.settings_path) {
            tracing::warn!(target: "clairlune", error = %e, "could not save settings");
        }
        self.settings_stamp = file_stamp(&self.settings_path);
    }

    /// Picks up edits to settings.toml and system theme changes.
    pub(crate) fn check(&mut self) {
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

    /// Notices monitor changes (system messages on Windows, a poll elsewhere)
    /// and, once the layout has settled and differs from the one the
    /// wallpaper was made for, makes the current scenes again at the new
    /// sizes: same seeds, saved scenes replayed where a monitor kept its size
    ///. Explorer restarts start live playback again.
    pub(crate) fn watch_monitors(&mut self, event_loop: &ActiveEventLoop, now: Instant) {
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
    pub(crate) fn follow_display_settings(&mut self) {
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

    /// With no window open, gives the memory a closed window or a finished
    /// render touched back to the system, so the app idles small.
    pub(crate) fn trim_if_idle(&self) {
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
    pub(crate) fn log_usage(&mut self, now: Instant) {
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

    /// What the features in use need the user to allow in the system's
    /// settings (macOS, Windows), for Settings > General: start at login
    /// switched on in Login Items or Startup apps.
    pub(crate) fn check_permissions(&mut self) {
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
    pub(crate) fn login_needs_approval(&self) -> bool {
        cfg!(any(target_os = "macos", windows))
            && self.status.start_at_login == Some(true)
            && std::env::current_exe()
                .is_ok_and(|exe| clairlune_platform::start_at_login_needs_approval(&exe))
    }

    /// What a problem report carries.
    pub(crate) fn report_facts(&self) -> report::Facts {
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

    /// The tray icon is wanted but isn't up (no tray on this desktop, or it
    /// failed): Settings is the only way in, and closing it quits.
    pub(crate) fn tray_missing(&self) -> bool {
        self.settings.general.show_tray_icon && self.tray.is_none()
    }

    /// Shows or takes away the tray icon (the menu bar icon on macOS) as
    /// the settings say. Hidden, the app runs on; opening it again (a
    /// second launch) brings up Settings.
    pub(crate) fn sync_tray(&mut self) {
        self.sync_tray_inner();
        if let Some(t) = &self.tray {
            t.show_update(self.update.newer(), self.install.in_place());
        }
    }

    pub(crate) fn sync_tray_inner(&mut self) {
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
}
