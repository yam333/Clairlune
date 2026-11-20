//! User settings: `<config_dir>/clairlune/settings.toml` (PLAN.md section 8).
//!
//! Every field has a default, so a missing file, a missing key or a file from
//! an older version all load. Unknown keys (from a newer version) are ignored
//! rather than rejected, and kept out of the way on save.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub rotation: Rotation,
    pub schedule: Schedule,
    pub theme: ThemeSettings,
    pub display: DisplaySettings,
    pub live: LiveSettings,
    pub content: Content,
    pub buddy: Buddy,
    pub archive: ArchiveSettings,
    pub export: Export,
    pub general: General,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Detailed logging until this Unix time (0: off).
    pub detailed_logging_until: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    /// Minutes between new wallpapers; 0 means only on demand.
    pub change_every_minutes: u32,
    /// A new wallpaper when the app starts (at login).
    pub change_at_login: bool,
    /// The schedule is paused (tray toggle).
    pub paused: bool,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            change_every_minutes: 60,
            change_at_login: true,
            paused: false,
        }
    }
}

/// Live: the scene plays behind the desktop icons. Still: one image, set
/// through the system, and no work between changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperMode {
    #[default]
    Live,
    Still,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub mode: WallpaperMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveSettings {
    /// Frames per second: 6, 12, 24, 30 or 60 (PLAN.md section 5.3).
    pub smoothness_fps: u32,
    /// On battery, drop to Battery saver (6 fps).
    pub battery_saver_on_battery: bool,
    /// On battery, stop animating (the frame stays).
    pub pause_on_battery: bool,
    /// When an app is maximized or full screen over a monitor.
    pub pause_when_covered: CoverPause,
}

/// What a maximized or full-screen app pauses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverPause {
    /// The monitor it covers.
    #[default]
    ThatMonitor,
    /// Every monitor.
    AllMonitors,
    /// Nothing: keep playing.
    Off,
}

impl Default for LiveSettings {
    fn default() -> Self {
        LiveSettings {
            smoothness_fps: 12,
            battery_saver_on_battery: true,
            pause_on_battery: false,
            pause_when_covered: CoverPause::ThatMonitor,
        }
    }
}

/// Generate new scenes on a schedule, or follow the daily timeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationMode {
    #[default]
    Generate,
    Timeline,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rotation {
    pub mode: RotationMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    FollowSystem,
    Light,
    Dark,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    pub mode: ThemeMode,
}

/// Content filters. Empty lists mean "everything".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Content {
    pub archetypes: Vec<String>,
    pub activities: Vec<String>,
    pub weather: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Buddy {
    /// Show Clawd in wallpapers.
    pub show: bool,
}

impl Default for Buddy {
    fn default() -> Self {
        Buddy { show: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchiveSettings {
    /// Days to keep unpinned entries; 0 keeps them forever.
    pub keep_days: u32,
    /// Size cap in megabytes for unpinned entries.
    pub max_mb: u64,
}

impl Default for ArchiveSettings {
    fn default() -> Self {
        ArchiveSettings {
            keep_days: 7,
            max_mb: 1024,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Export {
    /// Folder for exports; empty means `Pictures/Clairlune`.
    pub folder: String,
}

impl Settings {
    /// Loads settings, falling back to defaults when the file is missing.
    /// A file that doesn't parse is kept aside as `settings.toml.bad` so the
    /// user's edits aren't lost, and defaults are used.
    pub fn load(path: &Path) -> Settings {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(target: "clairlune", path = %path.display(), error = %e, "settings unreadable; using defaults");
                }
                return Settings::default();
            }
        };
        match toml::from_str(&text) {
            Ok(s) => s,
            Err(e) => {
                let aside = path.with_extension("toml.bad");
                let _ = std::fs::rename(path, &aside);
                tracing::warn!(target: "clairlune", path = %path.display(), aside = %aside.display(), error = %e, "settings did not parse; kept aside, using defaults");
                Settings::default()
            }
        }
    }

    /// Saves atomically (write a temporary file, then rename over).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("toml.part");
        std::fs::write(
            &tmp,
            format!("# Clairlune settings. Edit here or in the app.\n\n{text}"),
        )?;
        std::fs::rename(&tmp, path)
    }

    /// The scene filters for composition.
    pub fn content_context(&self) -> clairlune_scene::ContentContext {
        let mut ctx = clairlune_scene::ContentContext::default();
        if !self.content.archetypes.is_empty() {
            ctx.enabled_archetypes = self.content.archetypes.clone();
        }
        if !self.content.activities.is_empty() {
            ctx.enabled_activities = self.content.activities.clone();
        }
        if !self.content.weather.is_empty() {
            ctx.enabled_weather = self.content.weather.clone();
        }
        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("clairlune-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("settings.toml")
    }

    #[test]
    fn missing_file_gives_defaults() {
        let s = Settings::load(Path::new("definitely/not/here/settings.toml"));
        assert_eq!(s, Settings::default());
        assert_eq!(s.schedule.change_every_minutes, 60);
        assert_eq!(s.theme.mode, ThemeMode::FollowSystem);
        assert!(s.buddy.show);
        assert_eq!(s.display.mode, WallpaperMode::Live);
        assert_eq!(s.live.smoothness_fps, 12);
    }

    #[test]
    fn round_trips_and_tolerates_partial_and_unknown_keys() {
        let p = temp("rt");
        let mut s = Settings::default();
        s.schedule.change_every_minutes = 15;
        s.theme.mode = ThemeMode::Dark;
        s.content.archetypes = vec!["shore".into()];
        s.display.mode = WallpaperMode::Still;
        s.live.smoothness_fps = 24;
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            text.contains("[display]") && text.contains("mode = \"still\""),
            "{text}"
        );

        std::fs::write(
            &p,
            "[schedule]\nchange_every_minutes = 5\n[future]\nthing = 1\n",
        )
        .unwrap();
        let s = Settings::load(&p);
        assert_eq!(s.schedule.change_every_minutes, 5);
        assert!(s.schedule.change_at_login);
        assert_eq!(s.archive.keep_days, 7);
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_broken_file_is_kept_aside() {
        let p = temp("bad");
        std::fs::write(&p, "this is = = not toml").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
        assert!(p.with_extension("toml.bad").exists());
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }
}
