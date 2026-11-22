//! Live mode in the app: the scenes on screen play behind
//! the desktop icons. Switching Live on or off never makes a new scene: the
//! still set with each wallpaper is the live frame at the hero time, so going
//! live starts from exactly what's there, and stopping leaves it.

use crate::apply::LiveScene;
use std::time::{Duration, Instant};

/// The live host: windows in the desktop layer (Windows), or desktop-level
/// windows on every screen (macOS).
#[cfg(windows)]
type Host = clairlune_live::desktop::Desktop;
#[cfg(target_os = "macos")]
type Host = clairlune_live::mac::MacDesktop;

/// How often to check that the desktop layer is still there (Explorer
/// restarts take it, and the windows in it, away).
#[cfg_attr(not(windows), allow(dead_code))]
const HEALTH_CHECK: Duration = Duration::from_secs(1);

pub struct Live {
    /// What's on screen, to play (and to play again after a restart).
    scenes: Vec<LiveScene>,
    /// Monitors (by id) kept on the still frame.
    still_monitors: Vec<String>,
    enabled: bool,
    fps: u32,
    /// What a maximized or full-screen app pauses.
    cover_pause: crate::settings::CoverPause,
    /// On battery: drop to 6 fps, or stop animating.
    battery_saver: bool,
    battery_pause: bool,
    on_battery: bool,
    desktop: Option<Host>,
    next: Option<Instant>,
    checked: Option<Instant>,
    /// For Settings: what plays, or why live isn't available.
    pub status: Option<String>,
    /// Frames drawn since the last usage line, their total and worst time.
    frames: u64,
    busy: Duration,
    worst: Duration,
}

/// What live playback did since it was last asked, for the usage line.
pub struct FrameUse {
    pub frames: u64,
    pub busy: Duration,
    pub worst: Duration,
    /// Monitors playing, and the frame rate now (0: not playing).
    pub monitors: usize,
    pub fps: u32,
}

impl Live {
    pub fn new(enabled: bool, fps: u32) -> Live {
        Live {
            scenes: Vec::new(),
            still_monitors: Vec::new(),
            enabled,
            fps: fps.clamp(1, 60),
            cover_pause: crate::settings::CoverPause::default(),
            battery_saver: true,
            battery_pause: false,
            on_battery: false,
            desktop: None,
            next: None,
            checked: None,
            status: None,
            frames: 0,
            busy: Duration::ZERO,
            worst: Duration::ZERO,
        }
    }

    /// What was drawn since the last call (and resets the count).
    pub fn take_use(&mut self) -> FrameUse {
        let playing = self.desktop.is_some();
        let out = FrameUse {
            frames: self.frames,
            busy: self.busy,
            worst: self.worst,
            monitors: if playing { self.playing().count() } else { 0 },
            fps: if playing { self.effective_fps() } else { 0 },
        };
        self.frames = 0;
        self.busy = Duration::ZERO;
        self.worst = Duration::ZERO;
        out
    }

    fn count(&mut self, busy: Duration) {
        self.frames += 1;
        self.busy += busy;
        self.worst = self.worst.max(busy);
    }

    /// Sets each monitor's still again (macOS: after a Space change, the
    /// new Space shows its own picture until then).
    #[cfg(target_os = "macos")]
    pub fn reset_stills(&self) {
        let Ok(host) = clairlune_platform::detect() else {
            return;
        };
        for s in &self.scenes {
            if let Some(still) = &s.still
                && let Err(e) = host.set_static(&s.monitor, still)
            {
                tracing::warn!(target: "clairlune", error = %e, "could not set the still again");
            }
        }
    }

    /// Starts playing again (Explorer restarted: the old windows are gone).
    #[cfg_attr(not(windows), allow(dead_code))]
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
        if on != self.enabled {
            self.enabled = on;
            self.restart();
        }
    }

    /// Monitors (by id) that keep the still frame while the others play.
    pub fn set_still_monitors(&mut self, ids: Vec<String>) {
        if ids != self.still_monitors {
            self.still_monitors = ids;
            self.restart();
        }
    }

    /// The scenes that play (not on a monitor kept still).
    fn playing(&self) -> impl Iterator<Item = &LiveScene> {
        self.scenes
            .iter()
            .filter(|s| !self.still_monitors.contains(&s.monitor.id))
    }

    pub fn set_fps(&mut self, fps: u32) {
        self.fps = fps.clamp(1, 60);
    }

    /// What to do on battery.
    pub fn set_battery_policy(&mut self, saver: bool, pause: bool) {
        self.battery_saver = saver;
        self.battery_pause = pause;
    }

    /// What a maximized or full-screen app pauses.
    pub fn set_cover_pause(&mut self, pause: crate::settings::CoverPause) {
        self.cover_pause = pause;
        if let Some(d) = &mut self.desktop {
            d.cover_policy = cover_policy(pause);
        }
    }

    /// The frame rate now: Battery saver's 6 fps on battery when asked, and
    /// no more than the presenter is worth (GDI: Balanced).
    fn effective_fps(&self) -> u32 {
        let cap = self.desktop.as_ref().map_or(60, |d| d.max_fps());
        let fps = self.fps.min(cap);
        if self.on_battery && self.battery_saver {
            fps.min(6)
        } else {
            fps
        }
    }

    /// The lowest frame rate a scrolling scene (a journey) still looks right
    /// at; below it (Battery saver) journeys aren't shown.
    pub const SCROLL_MIN_FPS: u32 = 12;

    /// Whether scrolling scenes look right now: a still frame shows them
    /// still, and live needs enough frames a second to scroll smoothly.
    pub fn scrolling_ok(&self) -> bool {
        !self.enabled || self.effective_fps() >= Self::SCROLL_MIN_FPS
    }

    /// How long one frame lasts now.
    pub fn frame_interval(&self) -> Duration {
        self.frame()
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

    fn start(&mut self) {
        use clairlune_live::Player;
        if self.playing().next().is_none() {
            self.status = Some("Every monitor shows the still frame (Settings, Display).".into());
            return;
        }
        let players: Option<Vec<_>> = self
            .playing()
            .map(|s| {
                match s.view {
                    Some(v) => Player::new_view(s.package.clone(), s.theme, s.buddy, v),
                    None => Player::new(s.package.clone(), s.theme, s.buddy),
                }
                .map(|p| (s.monitor.clone(), p))
            })
            .collect();
        let Some(players) = players else {
            self.status = Some("Live isn't available: a scene lacks its theme.".into());
            return;
        };
        match Host::start(players) {
            Ok(mut desktop) => {
                desktop.cover_policy = cover_policy(self.cover_pause);
                let n = self.playing().count();
                let capped = if desktop.max_fps() < self.fps {
                    format!(" (at most {} fps here)", desktop.max_fps())
                } else {
                    String::new()
                };
                self.status = Some(format!(
                    "Playing on {n} monitor{} at {} fps{capped}.",
                    if n == 1 { "" } else { "s" },
                    self.fps.min(desktop.max_fps())
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
        #[cfg(target_os = "macos")]
        if self.desktop.take().is_some() {
            tracing::info!(target: "clairlune", "live wallpaper stopped");
        }
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
                self.on_battery = power_source(self.on_battery);
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
                let drawn = desktop.tick();
                if let Ok(stats) = &drawn {
                    let busy = stats.busy;
                    self.count(busy);
                }
                if let Err(e) = drawn {
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
        // macOS: a covered, locked or sleeping screen reports its window
        // occluded, which the host checks every frame.
        #[cfg(target_os = "macos")]
        {
            let desktop = self.desktop.as_mut()?;
            if self.checked.is_none_or(|t| now >= t + HEALTH_CHECK) {
                self.checked = Some(now);
                self.on_battery = power_source(self.on_battery);
            }
            let next = self.next?;
            if self.on_battery && self.battery_pause {
                // Paused on battery: the frame on screen stays; look again
                // at the next check.
                return self.checked.map(|t| t + HEALTH_CHECK);
            }
            if now >= next {
                let drawn = desktop.tick();
                if let Ok(stats) = &drawn {
                    let busy = stats.busy;
                    self.count(busy);
                }
                if let Err(e) = drawn {
                    tracing::warn!(target: "clairlune", error = %e, "live frame failed; stopping");
                    self.stop();
                    self.status = Some(format!(
                        "Live stopped ({e}); the still frame is the wallpaper."
                    ));
                    return None;
                }
                self.next = Some((next + self.frame()).max(now));
            }
            self.next
        }
    }
}

/// On battery now (logged when it changes from `was`).
fn power_source(was: bool) -> bool {
    let now = clairlune_platform::on_battery();
    if now != was {
        tracing::info!(target: "clairlune", on_battery = now, "live: power source");
    }
    now
}

fn cover_policy(p: crate::settings::CoverPause) -> clairlune_live::CoverPolicy {
    use crate::settings::CoverPause;
    use clairlune_live::CoverPolicy;
    match p {
        CoverPause::ThatMonitor => CoverPolicy::ThatMonitor,
        CoverPause::AllMonitors => CoverPolicy::AllMonitors,
        CoverPause::Off => CoverPolicy::Off,
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop();
    }
}
