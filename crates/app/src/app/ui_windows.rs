//! Opening the windows (Settings, Library, Archive, Timeline) and keeping
//! what they show up to date.

use crate::*;

impl App {
    fn saved_bounds(&self, kind: ui::Kind) -> Option<ui::WindowBounds> {
        self.window_bounds
            .get(&kind)
            .copied()
            .filter(|_| self.ui_test.is_none())
    }

    pub(crate) fn open_timeline(&mut self, event_loop: &ActiveEventLoop) {
        self.refresh_library();
        if let Some(w) = &self.timeline_window {
            w.focus();
            w.request_redraw();
            return;
        }
        let theme = apply::resolve_theme(&self.settings.theme);
        let bounds = self.saved_bounds(ui::Kind::Timeline);
        match ui::UiWindow::open(
            event_loop,
            ui::Kind::Timeline,
            theme,
            self.last_section,
            bounds,
        ) {
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

    /// Opens Settings (or brings it forward). Falls back to the settings file
    /// in a text editor if no window can be made (no GPU, for example).
    pub(crate) fn open_settings(&mut self, event_loop: &ActiveEventLoop, theme: Option<Theme>) {
        if let Some(w) = &self.settings_window {
            w.focus();
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
        let bounds = self.saved_bounds(ui::Kind::Settings);
        match ui::UiWindow::open(
            event_loop,
            ui::Kind::Settings,
            theme,
            self.last_section,
            bounds,
        ) {
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
    pub(crate) fn library(&self) -> Option<library::Library> {
        library::Library::open(self.data_dir.join("library")).ok()
    }

    pub(crate) fn refresh_library(&mut self) {
        self.library_items = self.library().map(|l| l.items()).unwrap_or_default();
        self.library_uses = self
            .library_items
            .iter()
            .map(|i| (i.id.clone(), self.timeline.uses(&i.id)))
            .filter(|(_, u)| !u.is_empty())
            .collect();
        self.redraw_windows();
    }

    pub(crate) fn redraw_windows(&self) {
        for w in [
            &self.settings_window,
            &self.library_window,
            &self.timeline_window,
            &self.archive_window,
            &self.ascii_window,
        ]
        .into_iter()
        .flatten()
        {
            w.request_redraw();
        }
    }

    pub(crate) fn open_ascii(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.ascii_window {
            w.focus();
            w.request_redraw();
            return;
        }
        self.status.note = None;
        let theme = apply::resolve_theme(&self.settings.theme);
        let bounds = self.saved_bounds(ui::Kind::Ascii);
        match ui::UiWindow::open(
            event_loop,
            ui::Kind::Ascii,
            theme,
            self.last_section,
            bounds,
        ) {
            Ok(w) => self.ascii_window = Some(w),
            Err(e) => tracing::warn!(target: "clairlune", error = %e, "no image-to-ascii window"),
        }
    }

    pub(crate) fn open_library(&mut self, event_loop: &ActiveEventLoop) {
        self.refresh_library();
        if let Some(w) = &self.library_window {
            w.focus();
            w.request_redraw();
            return;
        }
        let theme = apply::resolve_theme(&self.settings.theme);
        let bounds = self.saved_bounds(ui::Kind::Library);
        match ui::UiWindow::open(
            event_loop,
            ui::Kind::Library,
            theme,
            self.last_section,
            bounds,
        ) {
            Ok(w) => self.library_window = Some(w),
            Err(e) => tracing::warn!(target: "clairlune", error = %e, "no library window"),
        }
    }

    /// Copies the archive entry on screen into the library.
    pub(crate) fn save_to_library(&mut self) {
        let dir = self.current.clone();
        self.save_dir_to_library(dir);
    }

    /// Copies an archive entry into the library.
    pub(crate) fn save_dir_to_library(&mut self, dir: Option<PathBuf>) {
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

    pub(crate) fn refresh_archive(&mut self) {
        self.archive_items = self.archive().map(|a| a.entries()).unwrap_or_default();
        if let Some(w) = &self.archive_window {
            w.request_redraw();
        }
    }

    pub(crate) fn open_archive(&mut self, event_loop: &ActiveEventLoop) {
        self.refresh_archive();
        if let Some(w) = &self.archive_window {
            w.focus();
            w.request_redraw();
            return;
        }
        let theme = apply::resolve_theme(&self.settings.theme);
        let bounds = self.saved_bounds(ui::Kind::Archive);
        match ui::UiWindow::open(
            event_loop,
            ui::Kind::Archive,
            theme,
            self.last_section,
            bounds,
        ) {
            Ok(w) => self.archive_window = Some(w),
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "no archive window; opening the folder");
                open(&self.data_dir.join("archive"), false);
            }
        }
    }

    /// The Archive window's requests.
    pub(crate) fn on_archive_action(&mut self, action: ui::view::UiAction) {
        use ui::view::UiAction;
        let Some(archive) = self.archive() else {
            return;
        };
        let dir_of = |name: &str| archive.dir().join(name);
        match action {
            UiAction::ApplyArchive(name) => {
                let dir = dir_of(&name);
                let (th, bd, lv) = Self::photo_choices_for_dir(&dir);
                self.current_theme = th;
                self.current_buddy = bd;
                self.set_current_live(lv);
                self.chosen = true;
                if !self.apply_folder(dir, th, bd, false, false) {
                    self.status.note = Some("That archived wallpaper couldn't be read.".into());
                }
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
                let entry_theme = archive.get(&name).and_then(|e| match e.theme.as_str() {
                    "light" => Some(Theme::Light),
                    "dark" => Some(Theme::Dark),
                    _ => None,
                });
                if gif {
                    self.status.note = Some("Making the GIF...".to_string());
                    self.submit(Job::ExportGif {
                        sign: self.settings.export.sign,
                        dest,
                        source,
                        theme: entry_theme
                            .unwrap_or_else(|| apply::resolve_theme(&self.settings.theme)),
                        buddy: self.settings.buddy.show,
                    });
                } else {
                    self.submit(Job::Export {
                        sign: self.settings.export.sign,
                        dest,
                        source,
                        still: None,
                        theme: entry_theme,
                        buddy: Some(self.settings.buddy.show),
                    });
                }
            }
            UiAction::CopyArchivePng(name) => {
                let entry_theme = archive.get(&name).and_then(|e| match e.theme.as_str() {
                    "light" => Some(Theme::Light),
                    "dark" => Some(Theme::Dark),
                    _ => None,
                });
                self.copy_png_to_clipboard(
                    Some(dir_of(&name)),
                    None,
                    entry_theme,
                    Some(self.settings.buddy.show),
                );
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

    pub(crate) fn resolve_choices(
        theme: library::ThemeChoice,
        buddy: library::BuddyChoice,
        display: library::DisplayChoice,
    ) -> (Option<Theme>, Option<bool>, Option<bool>) {
        let theme = match theme {
            library::ThemeChoice::Follow => None,
            library::ThemeChoice::Light => Some(Theme::Light),
            library::ThemeChoice::Dark => Some(Theme::Dark),
        };
        let buddy = match buddy {
            library::BuddyChoice::Follow => None,
            library::BuddyChoice::On => Some(true),
            library::BuddyChoice::Off => Some(false),
        };
        let live = match display {
            library::DisplayChoice::Follow => None,
            library::DisplayChoice::Live => Some(true),
            library::DisplayChoice::Still => Some(false),
        };
        (theme, buddy, live)
    }

    pub(crate) fn item_choices(
        item: &library::Item,
        slot_theme: library::ThemeChoice,
        slot_buddy: library::BuddyChoice,
        slot_display: library::DisplayChoice,
    ) -> (Option<Theme>, Option<bool>, Option<bool>) {
        let theme = if slot_theme != library::ThemeChoice::Follow {
            slot_theme
        } else {
            item.theme
        };
        let buddy = if slot_buddy != library::BuddyChoice::Follow {
            slot_buddy
        } else {
            item.buddy
        };
        let display = if slot_display != library::DisplayChoice::Follow {
            slot_display
        } else {
            item.display
        };
        Self::resolve_choices(theme, buddy, display)
    }

    pub(crate) fn on_library_action(&mut self, action: ui::view::UiAction) {
        use ui::view::UiAction;
        let Some(lib) = self.library() else {
            return;
        };
        match action {
            UiAction::ApplyLibrary(id) => {
                if let Some(item) = lib.get(&id) {
                    if self.timeline_mode() && !self.timeline_generate {
                        self.manual_until = self.timeline_next;
                    }
                    let (theme, buddy, live) = Self::item_choices(
                        &item,
                        library::ThemeChoice::Follow,
                        library::BuddyChoice::Follow,
                        library::DisplayChoice::Follow,
                    );
                    self.current_theme = theme;
                    self.current_buddy = buddy;
                    self.set_current_live(live);
                    self.chosen = true;
                    if !self.apply_folder(lib.item_dir(&id), theme, buddy, false, false) {
                        self.status.note = Some("That saved wallpaper couldn't be read.".into());
                    }
                }
            }
            UiAction::UpdateLibrary(item) => {
                if let Err(e) = lib.update(&item) {
                    self.status.note = Some(format!("Couldn't save the change: {e}"));
                }
                // Showing this item: apply its new choices right away.
                if self.current.as_deref() == Some(lib.item_dir(&item.id).as_path()) {
                    let applied = self
                        .timeline_applied
                        .as_ref()
                        .filter(|_| self.timeline_mode());
                    let (theme, buddy, live) = Self::item_choices(
                        &item,
                        applied.map_or(library::ThemeChoice::Follow, |s| s.theme),
                        applied.map_or(library::BuddyChoice::Follow, |s| s.buddy),
                        applied.map_or(library::DisplayChoice::Follow, |s| s.display),
                    );
                    self.current_theme = theme;
                    self.current_buddy = buddy;
                    self.set_current_live(live);
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
                let item = lib.get(&id);
                let (theme, buddy, _) = item
                    .map(|i| {
                        Self::item_choices(
                            &i,
                            library::ThemeChoice::Follow,
                            library::BuddyChoice::Follow,
                            library::DisplayChoice::Follow,
                        )
                    })
                    .unwrap_or((None, None, None));
                let resolved_theme =
                    theme.unwrap_or_else(|| apply::resolve_theme(&self.settings.theme));
                let resolved_buddy = buddy.unwrap_or(self.settings.buddy.show);
                if gif {
                    self.status.note = Some("Making the GIF...".to_string());
                    self.submit(Job::ExportGif {
                        sign: self.settings.export.sign,
                        dest,
                        source,
                        theme: resolved_theme,
                        buddy: resolved_buddy,
                    });
                } else {
                    self.submit(Job::Export {
                        sign: self.settings.export.sign,
                        dest,
                        source,
                        still: None,
                        theme: Some(resolved_theme),
                        buddy: Some(resolved_buddy),
                    });
                }
            }
            UiAction::CopyLibraryPng(id) => {
                let item = lib.get(&id);
                let (theme, buddy, _) = item
                    .map(|i| {
                        Self::item_choices(
                            &i,
                            library::ThemeChoice::Follow,
                            library::BuddyChoice::Follow,
                            library::DisplayChoice::Follow,
                        )
                    })
                    .unwrap_or((None, None, None));
                let resolved_theme =
                    theme.unwrap_or_else(|| apply::resolve_theme(&self.settings.theme));
                let resolved_buddy = buddy.unwrap_or(self.settings.buddy.show);
                self.copy_png_to_clipboard(
                    Some(lib.item_dir(&id)),
                    None,
                    Some(resolved_theme),
                    Some(resolved_buddy),
                );
            }
            _ => {}
        }
        self.refresh_library();
    }

    /// Loads the current wallpaper into the open Settings window: its
    /// thumbnail, and the main monitor's still behind the glass.
    pub(crate) fn refresh_window_thumb(&mut self) {
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
    pub(crate) fn land_thumbs(&mut self, theme: Theme) {
        // Bumped only when the preview tiles themselves (palettes, letter sets
        // or tile sizing) change, so app updates keep cached tiles on disk.
        const THUMB_VERSION: u32 = 2;
        let dir = self.data_dir.join("thumbs").join(format!(
            "g{}-t{THUMB_VERSION}-{}",
            clairlune_scene::GENERATOR_VERSION,
            theme.as_str()
        ));
        if let Some(w) = &mut self.settings_window {
            w.set_land_dir(dir.clone());
        }
        // Earlier versions' tiles go.
        let current = format!("g{}-t{THUMB_VERSION}-", clairlune_scene::GENERATOR_VERSION);
        if let Ok(old) = std::fs::read_dir(self.data_dir.join("thumbs")) {
            for e in old.flatten() {
                if !e.file_name().to_string_lossy().starts_with(&current) {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        if self.drawing_lands {
            self.lands_pending = Some(theme);
            return;
        }
        self.lands_pending = None;
        // The landscapes, the palettes, the letters: (file name, what to draw).
        let lands = clairlune_scene::archetype::ARCHETYPE_INFO
            .iter()
            .filter(|a| a.implemented)
            .map(|a| (a.id.to_string(), Tile::Land(a.id)));
        let palettes = clairlune_scene::palette::PALETTES
            .iter()
            .map(|p| (format!("palette-{p}"), Tile::Palette(p)));
        let letters = clairlune_scene::glyphs::LETTERS
            .iter()
            .map(|l| (format!("letters-{l}"), Tile::Letters(l)));
        let ids: Vec<(String, Tile)> = lands
            .chain(palettes)
            .chain(letters)
            .filter(|(name, _)| !dir.join(format!("{name}.png")).exists())
            .collect();
        if ids.is_empty() {
            return;
        }
        self.drawing_lands = true;
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new()
            .name("clairlune-lands".into())
            .spawn(move || {
                for (i, (name, tile)) in ids.iter().enumerate() {
                    let drawn = match tile {
                        Tile::Land(id) => apply::land_thumb(&dir, id, theme),
                        Tile::Palette(p) => apply::palette_thumb(&dir, p, theme),
                        Tile::Letters(l) => apply::letters_thumb(&dir, l, theme),
                    };
                    if let Err(e) = drawn {
                        tracing::warn!(target: "clairlune", tile = %name, error = %e, "Content tile");
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

    pub(crate) fn redraw_settings(&self) {
        if let Some(w) = &self.settings_window {
            w.request_redraw();
        }
    }

    pub(crate) fn status(&self) -> ui::view::Status {
        let mut s = self.status.clone();
        s.monitors = self
            .screens
            .iter()
            .zip(tray::monitor_labels(&self.screens))
            .map(|(m, label)| (m.id.clone(), label))
            .collect();
        s.screens = self.screens.clone();
        s.span_shown = self.span_shown;
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
        s.export_dir = self.export_dir().display().to_string();
        s.timeline_next = self
            .timeline_next
            .filter(|_| self.timeline_mode())
            .map(|t| format!("Next slot at {}", timeline::local_hhmm(t)));
        s.timeline_problems = match self.library() {
            Some(lib) => self.timeline.problems(&|id| lib.get(id).is_some()),
            None => Vec::new(),
        };
        s.busy = self.busy;
        s.saving_library = self.saving_ascii_library;
        s.paused = self.settings.schedule.paused;
        s.live_active = !self.restored()
            && self
                .current_live
                .unwrap_or(self.settings.display.mode == settings::WallpaperMode::Live);
        s.next_in_minutes = self
            .next_change
            .map(|t| t.saturating_duration_since(Instant::now()).as_secs() / 60);
        s
    }
}

/// What a tile in Content shows.
enum Tile {
    Land(&'static str),
    Palette(&'static str),
    Letters(&'static str),
}
