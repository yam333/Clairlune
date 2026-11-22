//! Handing work to the worker thread, and what comes back.

use crate::*;

impl App {
    pub(crate) fn submit(&mut self, job: Job) {
        let export = matches!(job, Job::Export { .. } | Job::ExportGif { .. });
        // Restore waits its turn behind a wallpaper being made, rather than
        // being dropped: the user asked for their own wallpaper back.
        let restore = matches!(job, Job::Restore | Job::RestoreToRemove);
        if self.busy && !export && !restore {
            return;
        }
        if !export {
            self.busy = true;
            if let Some(t) = &self.tray {
                t.set_busy(true);
            }
        }
        if self.jobs.send(job).is_err() {
            tracing::error!(target: "clairlune", "worker thread is gone");
        }
    }

    pub(crate) fn on_done(&mut self, done: Done) {
        match done {
            Done::Applied {
                source,
                seed,
                theme,
                wanted,
                archetype,
                activity,
                monitors,
                ms,
                live,
            } => {
                self.busy = false;
                self.set_restored(false);
                self.current = source;
                diagnostics::set_current(self.current.clone());
                self.shown_theme = Some(theme);
                self.asked_theme = Some(wanted);
                self.shown_journey = live.iter().any(|s| s.package.recipe.is_journey());
                self.shown_still = live
                    .iter()
                    .find(|s| s.monitor.primary)
                    .or(live.first())
                    .filter(|s| s.view.is_none())
                    .and_then(|s| s.still.clone());
                // One scene per monitor: each can get a new one. (A span is
                // one scene.)
                let span = live.iter().any(|s| s.view.is_some());
                self.screens = live.iter().map(|s| s.monitor.clone()).collect();
                self.tray_monitors = if span {
                    Vec::new()
                } else {
                    live.iter().map(|s| s.monitor.clone()).collect()
                };
                if let Some(t) = &self.tray {
                    t.set_monitors(&self.tray_monitors);
                }
                self.live.show(live);
                self.refresh_archive();
                self.trim_if_idle();
                let what = match activity {
                    Some(a) => {
                        format!("{} in {}", a.replace('_', " "), archetype.replace('_', " "))
                    }
                    None => archetype.replace('_', " "),
                };
                let mut text = format!("{APP_NAME}: {what} (seed {seed})");
                if self.timeline_mode()
                    && let Some(next) = self.timeline_next
                {
                    text.push_str(&format!(", next at {}", timeline::local_hhmm(next)));
                }
                tracing::debug!(target: "clairlune", monitors, ms, "{text}");
                self.status.what = what;
                self.status.seed = Some(seed);
                self.status.theme = Some(theme);
                self.refresh_window_thumb();
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(&text);
                }
            }
            Done::Exported(path) => {
                tracing::info!(target: "clairlune", path = %path.display(), "saved");
                self.status.note = Some(format!("Saved {}", path.display()));
                if let Some(dir) = path.parent() {
                    open(dir, false);
                }
            }
            Done::NoHost(why) => {
                self.status.note = Some(why);
                if let Some(w) = &self.settings_window {
                    w.request_redraw();
                }
            }
            // Handled where events arrive (it ends the app).
            Done::ReadyToRemove(_) => {}
            Done::Restored => {
                self.busy = false;
                self.set_restored(true);
                // Stop playing over it (with no scenes left, stopping
                // doesn't set Clairlune's stills again).
                self.live.show(Vec::new());
                self.shown_theme = None;
                self.shown_journey = false;
                self.shown_still = None;
                self.current = None;
                self.status.what.clear();
                self.status.seed = None;
                self.status.theme = None;
                self.status.note = Some(
                    "Your wallpaper is back. Changes are paused until you resume them.".to_string(),
                );
                if let Some(w) = &mut self.settings_window {
                    w.clear_thumb();
                }
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(APP_NAME);
                }
            }
            Done::Failed(e) => {
                self.busy = false;
                tracing::debug!(target: "clairlune", error = %e, "shown in the tray tooltip");
                self.status.note = Some(capitalise_first(&e));
                if let Some(t) = &self.tray {
                    t.set_busy(false);
                    t.set_tooltip(&format!("{APP_NAME}: {e}"));
                }
            }
        }
    }
}
