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

    /// Copies a wallpaper's primary PNG to the clipboard (signed when asked,
    /// matching Save PNG).
    pub(crate) fn copy_png_to_clipboard(
        &mut self,
        prefer: ui::Kind,
        source: Option<PathBuf>,
        still: Option<PathBuf>,
    ) {
        let dir = source.or_else(|| {
            let archive = self.archive()?;
            let entry = archive.latest()?;
            Some(archive.dir().join(entry.name))
        });
        let src = still
            .filter(|p| p.is_file())
            .or_else(|| dir.map(|d| d.join("wallpaper-0.png")));
        let decoded = src
            .ok_or_else(|| "There's no wallpaper to copy yet.".to_string())
            .and_then(|path| {
                let bytes = std::fs::read(&path)
                    .map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
                let signed = self
                    .settings
                    .export
                    .sign
                    .then(|| apply::signed_png(&bytes))
                    .flatten();
                ui::decode_png_rgba(signed.as_deref().unwrap_or(&bytes))
                    .ok_or_else(|| "Couldn't decode the wallpaper image.".to_string())
            });
        match decoded {
            Ok((size, rgba)) => {
                self.copy_rgba_to_clipboard(
                    prefer,
                    size,
                    &rgba,
                    "Copied the wallpaper to the clipboard.",
                );
                return;
            }
            Err(e) => self.status.note = Some(e),
        }
        self.redraw_windows();
    }

    pub(crate) fn copy_rgba_to_clipboard(
        &mut self,
        prefer: ui::Kind,
        size: [usize; 2],
        rgba: &[u8],
        note: &str,
    ) {
        let mut windows: Vec<&mut ui::UiWindow> = [
            &mut self.settings_window,
            &mut self.library_window,
            &mut self.archive_window,
            &mut self.timeline_window,
            &mut self.ascii_window,
        ]
        .into_iter()
        .flatten()
        .collect();
        let idx = windows.iter().position(|w| w.kind() == prefer).unwrap_or(0);
        if let Some(w) = windows.get_mut(idx) {
            w.copy_image(size, rgba);
            self.status.note = Some(note.to_string());
        }
        self.redraw_windows();
    }

    /// Opens the system's file picker to choose an image for Image to ASCII.
    pub(crate) fn choose_ascii_image(&mut self) {
        if self.picking_ascii_image {
            return;
        }
        let mut dialog = rfd::AsyncFileDialog::new()
            .set_title("Choose an image")
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"],
            );
        if let Some(pics) = clairlune_platform::pictures_dir()
            && pics.is_dir()
        {
            dialog = dialog.set_directory(&pics);
        }
        if let Some(w) = &self.ascii_window {
            dialog = dialog.set_parent(w.window());
        }
        let picking = dialog.pick_file();
        let proxy = self.proxy.clone();
        let started = std::thread::Builder::new()
            .name("clairlune-ascii-pick".into())
            .spawn(move || {
                let picked = pollster::block_on(picking).map(|f| f.path().to_path_buf());
                let _ = proxy.send_event(AppEvent::AsciiImagePicked(picked));
            });
        self.picking_ascii_image = started.is_ok();
    }

    /// Decodes an image file (PNG, JPEG, WebP, GIF, BMP, TIFF) into the Studio.
    pub(crate) fn load_ascii_file(&mut self, path: &Path) {
        use clairlune_render::img2ascii::RgbaImage;
        let loaded = image::open(path)
            .map_err(|e| format!("Couldn't open {}: {e}", path.display()))
            .and_then(|dyn_img| {
                let rgba = dyn_img.into_rgba8();
                let (w, h) = rgba.dimensions();
                RgbaImage::new(w, h, rgba.into_raw())
            });
        match loaded {
            Ok(img) => {
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Image".into());
                self.status.note = None;
                if let Some(w) = &mut self.ascii_window {
                    w.set_ascii_image(stem, img);
                }
            }
            Err(e) => {
                self.status.note = Some(e);
                self.redraw_windows();
            }
        }
    }

    /// Pastes an image from the system clipboard into the Image to ASCII studio.
    pub(crate) fn paste_ascii_image(&mut self) {
        use clairlune_render::img2ascii::RgbaImage;
        let pasted = arboard::Clipboard::new()
            .and_then(|mut cb| cb.get_image())
            .map_err(|_| "There is no image on the clipboard to paste.".to_string())
            .and_then(|img| {
                RgbaImage::new(img.width as u32, img.height as u32, img.bytes.into_owned())
            });
        match pasted {
            Ok(img) => {
                self.status.note = None;
                if let Some(w) = &mut self.ascii_window {
                    w.set_ascii_image("Clipboard image".into(), img);
                }
            }
            Err(e) => {
                self.status.note = Some(e);
                self.redraw_windows();
            }
        }
    }

    /// Saves converted ASCII text as `<name>-ascii.txt` in the export folder.
    pub(crate) fn save_ascii_txt(&mut self, name: &str, text: &str) {
        let dest = self.export_dir();
        let res = std::fs::create_dir_all(&dest).and_then(|()| {
            let out = unique_export_path(&dest, &format!("{name}-ascii"), "txt");
            std::fs::write(&out, text.as_bytes()).map(|()| out)
        });
        match res {
            Ok(path) => {
                self.status.note = Some(format!("Saved {}", path.display()));
                open(&dest, false);
            }
            Err(e) => self.status.note = Some(format!("Couldn't save text file: {e}")),
        }
        self.redraw_windows();
    }

    /// Saves a rendered RGBA ASCII image as `<name>-ascii.png` in the export folder.
    pub(crate) fn export_ascii_png(&mut self, name: &str, width: u32, height: u32, rgba: &[u8]) {
        let dest = self.export_dir();
        let res =
            clairlune_render::img2ascii::encode_rgba_png(width, height, rgba).and_then(|png| {
                std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
                let out = unique_export_path(&dest, &format!("{name}-ascii"), "png");
                std::fs::write(&out, png).map_err(|e| e.to_string())?;
                Ok(out)
            });
        match res {
            Ok(path) => {
                self.status.note = Some(format!("Saved {}", path.display()));
                open(&dest, false);
            }
            Err(e) => self.status.note = Some(format!("Couldn't save PNG: {e}")),
        }
        self.redraw_windows();
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

fn unique_export_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 2..10_000 {
        let candidate = dir.join(format!("{stem}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    first
}
