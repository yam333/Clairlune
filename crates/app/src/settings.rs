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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Detailed logging until this Unix time (0: off).
    pub detailed_logging_until: u64,
    /// The tray icon (the menu bar icon on macOS) shows. Hidden, Clairlune
    /// runs on in the background; opening it again brings up Settings.
    pub show_tray_icon: bool,
    /// The first-run question, start quietly at sign-in, was answered.
    pub asked_start_at_login: bool,
}

impl Default for General {
    fn default() -> Self {
        General {
            detailed_logging_until: 0,
            show_tray_icon: true,
            asked_start_at_login: false,
        }
    }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperMode {
    Live,
    Still,
}

impl Default for WallpaperMode {
    /// Live where it has been tried (Windows); elsewhere Still until then,
    /// with Live one switch away in Settings.
    fn default() -> Self {
        if cfg!(windows) {
            WallpaperMode::Live
        } else {
            WallpaperMode::Still
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub mode: WallpaperMode,
    /// One scene spread across all monitors instead of one each.
    pub span: bool,
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
    /// Dark between two times of day, or from sunset to sunrise.
    Schedule,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    pub mode: ThemeMode,
    /// Schedule: dark from this local time ("HH:MM")...
    pub dark_from: String,
    /// ...until this one.
    pub light_from: String,
    /// Schedule: dark from sunset to sunrise at `location` instead.
    pub follow_sun: bool,
    /// "latitude, longitude" in decimal degrees (south and west negative).
    /// Sunrise and sunset are worked out on this computer.
    pub location: String,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        ThemeSettings {
            mode: ThemeMode::default(),
            dark_from: "19:00".into(),
            light_from: "07:00".into(),
            follow_sun: false,
            location: String::new(),
        }
    }
}

/// "HH:MM" as minutes after midnight.
pub fn parse_clock(text: &str) -> Option<u32> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Content filters. Empty lists mean "everything".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Content {
    pub archetypes: Vec<String>,
    pub activities: Vec<String>,
    pub weather: Vec<String>,
    /// How scenes may be drawn: `line`, `dither` (empty: both).
    pub looks: Vec<String>,
    pub contrast: Contrast,
}

/// Ink contrast for new scenes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contrast {
    Soft,
    #[default]
    Normal,
    High,
    /// A different one for each scene.
    Vary,
}

impl Contrast {
    pub fn as_str(self) -> &'static str {
        match self {
            Contrast::Soft => "soft",
            Contrast::Normal => "normal",
            Contrast::High => "high",
            Contrast::Vary => "vary",
        }
    }
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

    /// Loads settings edited while the app runs. A file that doesn't parse
    /// (a typo, or an editor half-way through saving) is an error: the app
    /// keeps what it has. A copy goes to `settings.toml.bad` so the edits
    /// survive the app saving over them.
    pub fn reload(path: &Path) -> Result<Settings, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => return Err(e.to_string()),
        };
        toml::from_str(&text).map_err(|e| {
            let _ = std::fs::write(path.with_extension("toml.bad"), &text);
            e.message().to_string()
        })
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
        if !self.content.looks.is_empty() {
            ctx.enabled_looks = self.content.looks.clone();
        }
        ctx.contrast = self.content.contrast.as_str().to_string();
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
        let mode = if cfg!(windows) {
            WallpaperMode::Live
        } else {
            WallpaperMode::Still
        };
        assert_eq!(s.display.mode, mode);
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
    fn a_typo_while_running_keeps_the_settings_and_the_file() {
        let p = temp("reload");
        std::fs::write(&p, "[schedule]\nchange_every_minutes = 5\n").unwrap();
        assert_eq!(
            Settings::reload(&p).unwrap().schedule.change_every_minutes,
            5
        );
        std::fs::write(&p, "[schedule]\nchange_every_minutes = = 5\n").unwrap();
        assert!(Settings::reload(&p).is_err());
        // Left where it is, with a copy aside.
        assert!(p.exists());
        assert!(p.with_extension("toml.bad").exists());
        std::fs::remove_file(&p).unwrap();
        assert_eq!(Settings::reload(&p).unwrap(), Settings::default());
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_tray_icon_shows_unless_hidden_and_the_question_waits() {
        let p = temp("general");
        std::fs::write(
            &p,
            "[general]
detailed_logging_until = 0
",
        )
        .unwrap();
        let s = Settings::load(&p);
        assert!(s.general.show_tray_icon);
        assert!(!s.general.asked_start_at_login);
        std::fs::write(
            &p,
            "[general]
show_tray_icon = false
",
        )
        .unwrap();
        assert!(!Settings::load(&p).general.show_tray_icon);
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
