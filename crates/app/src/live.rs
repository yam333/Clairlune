//! Live mode in the app (PLAN.md section 2): the scenes on screen play behind
//! the desktop icons. Switching Live on or off never makes a new scene: the
//! still set with each wallpaper is the live frame at the hero time, so going
//! live starts from exactly what's there, and stopping leaves it.

// Playback is Windows-only so far; elsewhere this only reports why.
#![cfg_attr(not(windows), allow(dead_code))]

use crate::apply::LiveScene;
use std::time::{Duration, Instant};

/// How often to check that the desktop layer is still there (Explorer
/// restarts take it, and the windows in it, away).
const HEALTH_CHECK: Duration = Duration::from_secs(1);

pub struct Live {
    /// What's on screen, to play (and to play again after a restart).
    scenes: Vec<LiveScene>,
    enabled: bool,
    fps: u32,
    /// On battery: drop to 6 fps, or stop animating.
    battery_saver: bool,
    battery_pause: bool,
    on_battery: bool,
    #[cfg(windows)]
    desktop: Option<clairlune_live::desktop::Desktop>,
    next: Option<Instant>,
    checked: Option<Instant>,
    /// For Settings: what plays, or why live isn't available.
    pub status: Option<String>,
}

/// Whether this build can play live wallpapers here, with the reason if not.
pub fn supported() -> Result<(), String> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err("Live wallpapers run on Windows so far; here the still frame is used.".into())
    }
}

impl Live {
    pub fn new(enabled: bool, fps: u32) -> Live {
        Live {
            scenes: Vec::new(),
            enabled: enabled && supported().is_ok(),
            fps: fps.clamp(1, 60),
            battery_saver: true,
            battery_pause: false,
            on_battery: false,
            #[cfg(windows)]
            desktop: None,
            next: None,
            checked: None,
            status: None,
        }
    }

    /// Starts playing again (Explorer restarted: the old windows are gone).
    pub fn refresh(&mut self) {
        self.restart();
    }

    /// A wallpaper was applied: play its scenes.
    pub fn show(&mut self, scenes: Vec<LiveScene>) {
        self.scenes = scenes;
        self.restart();
    }

    /// Live or Still frame, from the setting.
    pub fn set_enabled(&mut self, on: bool) {
        let on = on && supported().is_ok();
        if on != self.enabled {
            self.enabled = on;
            self.restart();
        }
    }

    pub fn set_fps(&mut self, fps: u32) {
        self.fps = fps.clamp(1, 60);
    }

    /// What to do on battery (PLAN.md section 5.3).
    pub fn set_battery_policy(&mut self, saver: bool, pause: bool) {
        self.battery_saver = saver;
        self.battery_pause = pause;
    }

    /// The frame rate now: Battery saver's 6 fps on battery when asked.
    fn effective_fps(&self) -> u32 {
        if self.on_battery && self.battery_saver {
            self.fps.min(6)
        } else {
            self.fps
        }
    }

    fn frame(&self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.effective_fps()))
    }

    fn restart(&mut self) {
        self.stop();
        if !self.enabled || self.scenes.is_empty() {
            return;
        }
        self.start();
    }

    #[cfg(not(windows))]
    fn start(&mut self) {
        self.status = supported().err();
    }

    #[cfg(windows)]
    fn start(&mut self) {
        use clairlune_live::Player;
        let players: Option<Vec<_>> = self
            .scenes
            .iter()
            .map(|s| {
                Player::new(s.package.clone(), s.theme, s.buddy).map(|p| (s.monitor.clone(), p))
            })
            .collect();
        let Some(players) = players else {
            self.status = Some("Live isn't available: a scene lacks its theme.".into());
            return;
        };
        match clairlune_live::desktop::Desktop::start(players) {
            Ok(desktop) => {
                let n = self.scenes.len();
                self.status = Some(format!(
                    "Playing on {n} monitor{} at {} fps.",
                    if n == 1 { "" } else { "s" },
                    self.fps
                ));
                self.desktop = Some(desktop);
                self.next = Some(Instant::now());
                self.checked = Some(Instant::now());
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", error = %e, "live wallpaper didn't start");
                self.status = Some(format!(
                    "Live isn't available here ({e}); the still frame is the wallpaper."
                ));
            }
        }
    }

    /// Stops playing. On the classic desktop a stale layer can keep the last
    /// live frame, so the stills are set again.
    fn stop(&mut self) {
        self.next = None;
        #[cfg(windows)]
        if let Some(desktop) = self.desktop.take() {
            let classic = desktop.layout() == "classic";
            drop(desktop);
            if classic && let Ok(host) = clairlune_platform::detect() {
                for s in &self.scenes {
                    if let Some(still) = &s.still {
                        let _ = host.set_static(&s.monitor, still);
                    }
                }
            }
            tracing::info!(target: "clairlune", "live wallpaper stopped");
        }
        if !self.enabled {
            self.status = None;
        }
    }

    /// Draws a frame when one is due; returns when to come back.
    pub fn tick(&mut self, now: Instant) -> Option<Instant> {
        #[cfg(windows)]
        {
            let desktop = self.desktop.as_mut()?;
            if self.checked.is_none_or(|t| now >= t + HEALTH_CHECK) {
                self.checked = Some(now);
                let battery = clairlune_platform::windows_live::on_battery();
                if battery != self.on_battery {
                    tracing::info!(target: "clairlune", on_battery = battery, "live: power source");
                    self.on_battery = battery;
                }
                if !desktop.healthy() {
                    tracing::info!(target: "clairlune", "the desktop layer changed (Explorer restarted?); starting live again");
                    self.restart();
                    return self.next;
                }
            }
            let next = self.next?;
            if self.on_battery && self.battery_pause {
                // Paused on battery: the frame on screen stays; look again
                // at the next health check.
                return self.checked.map(|t| t + HEALTH_CHECK);
            }
            if now >= next {
                if let Err(e) = desktop.tick() {
                    tracing::warn!(target: "clairlune", error = %e, "live frame failed; stopping");
                    self.stop();
                    self.status = Some(format!(
                        "Live stopped ({e}); the still frame is the wallpaper."
                    ));
                    return None;
                }
                // Keep the rhythm, but never try to catch up after a stall.
                self.next = Some((next + self.frame()).max(now));
            }
            self.next
        }
        #[cfg(not(windows))]
        {
            let _ = now;
            None
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop();
    }
}
