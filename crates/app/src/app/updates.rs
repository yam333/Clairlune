//! Updates (checking, installing, restarting into a new copy) and
//! removing Clairlune.

use crate::*;

impl App {
    /// Starts an update check on a thread; its answer comes back as an
    /// event. `by_hand`: asked for in Settings.
    pub(crate) fn check_updates(&mut self, by_hand: bool) {
        if self.checking_updates.is_some() || self.ui_test.is_some() {
            return;
        }
        self.checking_updates = Some(by_hand);
        let proxy = self.proxy.clone();
        let started = std::thread::Builder::new()
            .name("clairlune-update".into())
            .spawn(move || {
                let _ = proxy.send_event(AppEvent::UpdateChecked(update::fetch()));
            });
        if started.is_err() {
            self.checking_updates = None;
        }
    }

    pub(crate) fn on_update_checked(&mut self, result: Result<update::Release, String>) {
        let by_hand = self.checking_updates.take().unwrap_or(false);
        let now = timefmt::now_unix();
        match result {
            Ok(r) => {
                tracing::info!(target: "clairlune", latest = %r.version, "update check");
                self.update = update::UpdateState {
                    checked: now,
                    latest: Some(r.version),
                    url: Some(r.url),
                };
                self.status.update_note = Some(match self.update.newer() {
                    Some(v) => format!("Clairlune {v} is out"),
                    None => format!("Clairlune {} is up to date", env!("CARGO_PKG_VERSION")),
                });
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "update check failed");
                // Tried: the next try is in a few hours, not at once.
                self.update.checked = now.saturating_sub(update::EVERY_SECS - 3 * 3600);
                if by_hand {
                    self.status.update_note = Some(format!("Couldn't check: {e}"));
                }
            }
        }
        self.update.save(&self.data_dir);
        if let Some(t) = &self.tray {
            t.show_update(self.update.newer(), self.install.in_place());
        }
        if let Some(w) = &self.settings_window {
            w.request_redraw();
        }
    }

    /// Downloads the newer version and installs it in place, on a thread;
    /// where this install can't update itself, opens the release page.
    pub(crate) fn install_update(&mut self) {
        if self.installing.is_some() {
            return;
        }
        if !self.install.in_place() {
            self.open_update();
            return;
        }
        let install = self.install.clone();
        let proxy = self.proxy.clone();
        self.installing = Some("Getting the new version\u{2026}".into());
        tracing::info!(target: "clairlune", install = %install.name(), "installing an update");
        let started = std::thread::Builder::new()
            .name("clairlune-install".into())
            .spawn(move || {
                let ready = (|| {
                    let release = update::fetch()?;
                    if !update::is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
                        return Err("this is already the newest version".to_string());
                    }
                    let name = install
                        .asset_name(&release.version)
                        .ok_or("nothing to download for this install")?;
                    let asset = release
                        .assets
                        .iter()
                        .find(|a| a.name == name)
                        .ok_or_else(|| format!("the release has no {name}"))?;
                    let v = release.version.clone();
                    let p = proxy.clone();
                    install::prepare(&install, asset, &release.version, &|share| {
                        let _ = p.send_event(AppEvent::Installing(InstallStep::Progress(
                            v.clone(),
                            share,
                        )));
                    })
                })();
                let _ = proxy.send_event(AppEvent::Installing(InstallStep::Ready(ready)));
            });
        if started.is_err() {
            self.installing = None;
        }
        self.redraw_settings();
    }

    pub(crate) fn on_installing(&mut self, step: InstallStep, event_loop: &ActiveEventLoop) {
        match step {
            InstallStep::Progress(version, share) => {
                self.installing = Some(if share >= 100 {
                    format!("Checking {version}\u{2026}")
                } else {
                    format!("Downloading {version}: {share}%")
                });
            }
            InstallStep::Ready(Ok(p)) => {
                // Back as it was: Settings again if it's open, else quietly.
                let relaunch: &[&str] = if self.settings_window.is_some() {
                    &[]
                } else {
                    &["--background"]
                };
                match install::finish(&p, std::process::id(), Some(relaunch)) {
                    Ok(()) => {
                        tracing::info!(target: "clairlune", version = %p.version, "update ready; quitting to install it");
                        self.installing = Some(format!("Installing {}\u{2026}", p.version));
                        event_loop.exit();
                    }
                    Err(e) => {
                        tracing::warn!(target: "clairlune", error = %e, "update not installed");
                        self.installing = None;
                        self.status.update_note = Some(format!("Couldn't install it: {e}"));
                    }
                }
            }
            InstallStep::Ready(Err(e)) => {
                tracing::warn!(target: "clairlune", error = %e, "update not installed");
                self.installing = None;
                self.status.update_note = Some(format!("Couldn't update: {e}"));
            }
        }
        self.redraw_settings();
    }

    /// Remove Clairlune: stops everything, then puts the old wallpaper back
    /// (on the worker); [`finish_removal`](Self::finish_removal) does the
    /// rest when that's done.
    pub(crate) fn remove_everything(&mut self) {
        if self.removing {
            return;
        }
        tracing::info!(target: "clairlune", install = %self.install.name(), "removing Clairlune");
        self.removing = true;
        self.next_change = None;
        self.live.show(Vec::new());
        if let Ok(exe) = std::env::current_exe()
            && let Err(e) = clairlune_platform::set_start_at_login(&exe, false)
        {
            tracing::warn!(target: "clairlune", error = %e, "start at login not turned off");
        }
        self.status.note = Some("Removing Clairlune\u{2026}".into());
        self.submit(Job::RestoreToRemove);
        self.redraw_settings();
    }

    /// The old wallpaper is back (or there was none to put back): the
    /// helper deletes Clairlune's folders and the app once it has quit.
    pub(crate) fn finish_removal(&mut self, event_loop: &ActiveEventLoop) {
        let mut dirs = vec![self.data_dir.clone()];
        if let Some(config) = self.settings_path.parent() {
            dirs.push(config.to_path_buf());
        }
        match install::remove_after_exit(&self.install, std::process::id(), &dirs) {
            Ok(()) => {
                tracing::info!(target: "clairlune", "removal under way; quitting");
                event_loop.exit();
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "removal not started");
                self.removing = false;
                self.status.note = Some(format!("Couldn't remove Clairlune: {e}"));
                self.redraw_settings();
            }
        }
    }

    /// A new copy of Clairlune was put where this one runs from (Homebrew
    /// upgraded it, a new DMG copy was dragged over): once the change has
    /// settled, restart into it, the way it runs now (Settings again if
    /// it's open, else quietly). Returns whether a change is being watched.
    pub(crate) fn watch_exe(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if self.ui_test.is_some() || self.installing.is_some() || self.removing {
            return false;
        }
        let Some(start) = &self.exe_stamp else {
            return false;
        };
        let now = install::exe_stamp();
        match (&self.exe_replaced, now) {
            // Mid-copy: look again.
            (_, None) => true,
            (_, Some(n)) if n == *start => {
                self.exe_replaced = None;
                false
            }
            // The same new copy as a moment ago: settled.
            (Some(seen), Some(n)) if *seen == n => {
                let args: &[&str] = if self.settings_window.is_some() {
                    &[]
                } else {
                    &[clairlune_platform::BACKGROUND_ARG]
                };
                match install::restart_after_exit(std::process::id(), args) {
                    Ok(()) => {
                        tracing::info!(target: "clairlune", "a new copy of Clairlune is in place; restarting into it");
                        event_loop.exit();
                    }
                    Err(e) => {
                        tracing::warn!(target: "clairlune", error = %e, "couldn't restart into the new copy");
                        self.exe_stamp = Some(n);
                        self.exe_replaced = None;
                    }
                }
                false
            }
            (_, Some(n)) => {
                tracing::info!(target: "clairlune", "the program file changed on disk; checking it settles");
                self.exe_replaced = Some(n);
                true
            }
        }
    }

    /// The newer version's release page, in the browser.
    pub(crate) fn open_update(&self) {
        let url = self.update.url.as_deref().unwrap_or(update::RELEASES_PAGE);
        open_url(url);
    }
}
