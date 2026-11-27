//! What the menu and the windows ask for.

use crate::*;

impl App {
    pub(crate) fn on_action(&mut self, action: tray::Action, event_loop: &ActiveEventLoop) {
        use tray::Action;
        match action {
            Action::NewWallpaper => self.new_wallpaper_by_hand(),
            Action::NewWallpaperOn(i) => {
                if let Some(id) = self.tray.as_ref().and_then(|t| t.monitor_id(i)) {
                    self.new_wallpaper_on(id);
                }
            }
            Action::Previous => self.reapply(true),
            Action::TogglePause => {
                if self.settings.schedule.paused {
                    self.resume_changes();
                } else {
                    self.settings.schedule.paused = true;
                    self.save_settings();
                    self.reschedule();
                }
                self.redraw_windows();
            }
            Action::Theme(mode) => {
                self.settings.theme.mode = mode;
                if let Some(t) = &self.tray {
                    t.show_theme(mode);
                }
                self.save_settings();
                self.check();
            }
            Action::ToggleLive => {
                let currently_live = self
                    .current_live
                    .unwrap_or(self.settings.display.mode == settings::WallpaperMode::Live);
                self.current_live = None;
                self.settings.display.mode = if currently_live {
                    settings::WallpaperMode::Still
                } else {
                    settings::WallpaperMode::Live
                };
                self.save_settings();
                self.follow_display_settings();
                self.redraw_windows();
            }
            Action::SavePng => {
                self.export_png();
            }
            Action::SaveGif => self.export_gif(),
            Action::SaveToLibrary => self.save_to_library(),
            Action::OpenAscii => self.open_ascii(event_loop),
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

    pub(crate) fn on_ui_action(
        &mut self,
        action: ui::view::UiAction,
        event_loop: &ActiveEventLoop,
    ) {
        use ui::view::UiAction;
        match action {
            UiAction::SettingsChanged => {
                self.save_settings();
                // Spread across turned on or off, a monitor moved, another
                // made main: the wallpaper on screen is drawn again for it.
                let key = span::settings_key(&self.settings.display);
                if key != self.span_key {
                    self.span_key = key;
                    if self.current.is_some() && !self.restored() && !self.busy {
                        self.reapply(false);
                    }
                }
                self.sync_tray();
                self.check_permissions();
                if let Some(t) = &self.tray {
                    t.show_theme(self.settings.theme.mode);
                }
                self.follow_display_settings();
                self.reschedule();
                let theme = apply::resolve_theme(&self.settings.theme);
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
            UiAction::ChooseExportFolder => self.choose_export_folder(),
            UiAction::Resume => self.resume_changes(),
            UiAction::EditTimeline => self.open_timeline(event_loop),
            UiAction::TimelineChanged => self.timeline_edited(),
            UiAction::Previous => self.reapply(true),
            UiAction::OpenArchive => self.open_archive(event_loop),
            UiAction::OpenArchiveFolder => open(&self.data_dir.join("archive"), false),
            a @ (UiAction::ApplyArchive(_)
            | UiAction::PinArchive { .. }
            | UiAction::SaveArchiveToLibrary(_)
            | UiAction::ExportArchive { .. }
            | UiAction::CopyArchivePng(_)
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
            UiAction::CopyPng => {
                self.copy_png_to_clipboard(
                    ui::Kind::Settings,
                    self.current.clone(),
                    self.shown_still.clone(),
                );
            }
            UiAction::SaveToLibrary => self.save_to_library(),
            UiAction::OpenLibrary => self.open_library(event_loop),
            a @ (UiAction::ApplyLibrary(_)
            | UiAction::UpdateLibrary(_)
            | UiAction::DeleteLibrary { .. }
            | UiAction::ExportLibrary { .. }
            | UiAction::CopyLibraryPng(_)) => self.on_library_action(a),
            UiAction::OpenAscii => self.open_ascii(event_loop),
            UiAction::ChooseAsciiImage => self.choose_ascii_image(),
            UiAction::LoadAsciiFile(path) => self.load_ascii_file(&path),
            UiAction::PasteAsciiImage => self.paste_ascii_image(),
            UiAction::SaveAsciiTxt { name, text } => self.save_ascii_txt(&name, &text),
            UiAction::ExportAsciiPng {
                name,
                width,
                height,
                rgba,
            } => self.export_ascii_png(&name, width, height, &rgba),
            UiAction::CopyAsciiPng {
                width,
                height,
                rgba,
            } => self.copy_rgba_to_clipboard(
                ui::Kind::Ascii,
                [width as usize, height as usize],
                &rgba,
                "Copied the ASCII PNG to the clipboard.",
            ),
            UiAction::ApplyAsciiWallpaper {
                name,
                image,
                config,
            } => {
                use clairlune_render::img2ascii::AsciiColorMode;
                // Pause automatic changes so the user's custom ASCII wallpaper
                // stays on screen until they resume changes or ask for a new one.
                self.settings.schedule.paused = true;
                self.save_settings();
                self.reschedule();
                self.sync_tray();
                self.chosen = true;
                self.current_buddy = Some(false);
                self.current_theme = match config.color_mode {
                    AsciiColorMode::Black => Some(Theme::Dark),
                    AsciiColorMode::White => Some(Theme::Light),
                    _ => None,
                };
                let want_live = if config.weather != "none" {
                    Some(true)
                } else {
                    None
                };
                self.set_current_live(want_live);
                self.status.note = Some(
                    "Applied your ASCII wallpaper. Scheduled changes are paused until you resume them."
                        .into(),
                );
                self.submit(Job::Photo {
                    photo: Box::new(apply::PhotoSource {
                        name,
                        image,
                        config: *config,
                    }),
                    to_library: false,
                    library_dir: self.data_dir.join("library"),
                    settings: Box::new(self.settings.clone()),
                });
            }
            UiAction::SaveAsciiToLibrary {
                name,
                image,
                config,
            } => {
                self.submit(Job::Photo {
                    photo: Box::new(apply::PhotoSource {
                        name,
                        image,
                        config: *config,
                    }),
                    to_library: true,
                    library_dir: self.data_dir.join("library"),
                    settings: Box::new(self.settings.clone()),
                });
            }
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
                self.submit(Job::Restore);
            }
        }
        if let Some(w) = &self.settings_window {
            w.request_redraw();
        }
        let _ = event_loop;
    }
}
