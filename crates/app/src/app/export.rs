//! Saving the wallpaper as a PNG or a GIF, and where to.

use crate::*;

impl App {
    /// Saves the wallpaper on screen (not the newest archive entry: that's
    /// another one after Previous or a Library pick).
    pub(crate) fn export_png(&mut self) {
        let dest = self.export_dir();
        self.submit(Job::Export {
            sign: self.settings.export.sign,
            dest,
            source: self.current.clone(),
            still: self.shown_still.clone(),
        });
    }

    pub(crate) fn export_gif(&mut self) {
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

    /// Opens the system's folder picker for where saved images go: a sheet
    /// on Settings on macOS, a dialog on Windows. It's waited for on a
    /// thread of its own; the answer comes back as an event.
    pub(crate) fn choose_export_folder(&mut self) {
        if self.picking_folder {
            return;
        }
        let mut dialog = rfd::AsyncFileDialog::new().set_title("Where saved images go");
        let current = self.export_dir();
        if current.is_dir() {
            dialog = dialog.set_directory(&current);
        }
        if let Some(w) = &self.settings_window {
            dialog = dialog.set_parent(w.window());
        }
        let picking = dialog.pick_folder();
        let proxy = self.proxy.clone();
        let started = std::thread::Builder::new()
            .name("clairlune-folder".into())
            .spawn(move || {
                let picked = pollster::block_on(picking).map(|f| f.path().to_path_buf());
                let _ = proxy.send_event(AppEvent::FolderPicked(picked));
            });
        self.picking_folder = started.is_ok();
    }

    /// The export folder: the setting, or `Pictures/Clairlune`.
    pub(crate) fn export_dir(&self) -> PathBuf {
        if self.settings.export.folder.trim().is_empty() {
            clairlune_platform::pictures_dir()
                .unwrap_or_else(|| self.data_dir.clone())
                .join(APP_NAME)
        } else {
            PathBuf::from(self.settings.export.folder.trim())
        }
    }
}
