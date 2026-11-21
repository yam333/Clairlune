//! Linux: each desktop sets its wallpaper differently (PLAN.md section 2), so
//! the host drives the desktop's own tools: `gsettings` (GNOME, Cinnamon,
//! MATE), Plasma's D-Bus scripting (KDE), `xfconf-query` (XFCE), `swaymsg`
//! (Sway). Other Wayland compositors with the wlr layer-shell protocol
//! (Hyprland, river, Wayfire, labwc...) have no common tool, so Clairlune
//! shows the still in a background surface of its own (`wlr`), held while
//! the app runs. One image for all monitors (per-monitor images come later),
//! sized to the largest output `xrandr` reports.
//!
//! The decisions (which desktop, which commands, the screen size) are plain
//! functions with tests; only running the commands touches the system.

use crate::{Capabilities, Monitor, PlatformError, Snapshot, WallpaperHost};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The id of the one "monitor" this host manages: the whole desktop.
const DESKTOP_ID: &str = "linux-desktop";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Desktop {
    Gnome,
    Cinnamon,
    Mate,
    Kde,
    Xfce,
    Sway,
    /// Another Wayland compositor: our own background surface (`wlr`).
    Wlroots,
}

impl Desktop {
    fn name(self) -> &'static str {
        match self {
            Desktop::Gnome => "linux-gnome",
            Desktop::Cinnamon => "linux-cinnamon",
            Desktop::Mate => "linux-mate",
            Desktop::Kde => "linux-kde",
            Desktop::Xfce => "linux-xfce",
            Desktop::Sway => "linux-sway",
            Desktop::Wlroots => "linux-wlroots",
        }
    }
}

/// The desktop from `XDG_CURRENT_DESKTOP` (a `:`-separated list such as
/// `ubuntu:GNOME`), whether a Sway socket is set, and whether this is a
/// Wayland session (any other compositor: our own surface, if it has the
/// layer shell).
pub(crate) fn detect_desktop(current: &str, sway: bool, wayland: bool) -> Option<Desktop> {
    let parts: Vec<String> = current.split(':').map(|p| p.to_ascii_lowercase()).collect();
    let has = |name: &str| parts.iter().any(|p| p == name);
    if has("kde") {
        Some(Desktop::Kde)
    } else if has("x-cinnamon") || has("cinnamon") {
        Some(Desktop::Cinnamon)
    } else if has("mate") {
        Some(Desktop::Mate)
    } else if has("xfce") {
        Some(Desktop::Xfce)
    } else if has("gnome") || has("unity") || has("budgie") || has("pantheon") {
        Some(Desktop::Gnome)
    } else if has("sway") || sway {
        Some(Desktop::Sway)
    } else if wayland {
        Some(Desktop::Wlroots)
    } else {
        None
    }
}

/// A `file://` URI with everything but unreserved characters and `/`
/// percent-encoded.
pub(crate) fn file_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The path in a `file://` URI (as `gsettings get` prints it, quoted).
pub(crate) fn path_from_uri(value: &str) -> Option<PathBuf> {
    let v = value.trim().trim_matches('\'').trim_matches('"');
    let rest = v.strip_prefix("file://").unwrap_or(v);
    if rest.is_empty() {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 3 <= bytes.len()
            && let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8_lossy(&out).into_owned()))
}

/// A plain path as `gsettings get` (quoted) or `xfconf-query` prints it:
/// not a URI, so nothing is decoded.
pub(crate) fn plain_path(value: &str) -> Option<PathBuf> {
    let v = value.trim();
    let v = v
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
        .unwrap_or(v);
    (!v.is_empty()).then(|| PathBuf::from(v))
}

/// The `gsettings` keys that make up the wallpaper on a desktop, saved and
/// put back word for word.
pub(crate) fn gsettings_keys(desktop: Desktop) -> &'static [(&'static str, &'static str)] {
    match desktop {
        Desktop::Gnome => &[
            ("org.gnome.desktop.background", "picture-uri"),
            ("org.gnome.desktop.background", "picture-uri-dark"),
            ("org.gnome.desktop.background", "picture-options"),
        ],
        Desktop::Cinnamon => &[
            ("org.cinnamon.desktop.background", "picture-uri"),
            ("org.cinnamon.desktop.background", "picture-options"),
        ],
        Desktop::Mate => &[
            ("org.mate.background", "picture-filename"),
            ("org.mate.background", "picture-options"),
        ],
        _ => &[],
    }
}

/// The commands that put saved `gsettings` values back.
pub(crate) fn restore_commands(settings: &[(String, String)]) -> Vec<Vec<String>> {
    settings
        .iter()
        .filter_map(|(key, value)| {
            let (schema, key) = key.split_once(' ')?;
            Some(argv(&["gsettings", "set", schema, key, value]))
        })
        .collect()
}

/// Plasma's desktop image from its applet config: the `Image=` of the first
/// `[Containments][N][Wallpaper][org.kde.image][General]` group.
pub(crate) fn plasma_image(appletsrc: &str) -> Option<PathBuf> {
    let mut in_group = false;
    for line in appletsrc.lines().map(str::trim) {
        if line.starts_with('[') {
            in_group = line.starts_with("[Containments]")
                && line.ends_with("[Wallpaper][org.kde.image][General]");
        } else if in_group && let Some(v) = line.strip_prefix("Image=") {
            return path_from_uri(v);
        }
    }
    None
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

/// The commands that set `path` as the wallpaper on `desktop`.
pub(crate) fn set_commands(
    desktop: Desktop,
    path: &Path,
    xfce_props: &[String],
) -> Vec<Vec<String>> {
    let uri = file_uri(path);
    let file = path.to_string_lossy().into_owned();
    match desktop {
        // GNOME keeps a separate dark-mode wallpaper; set both.
        Desktop::Gnome => vec![
            argv(&[
                "gsettings",
                "set",
                "org.gnome.desktop.background",
                "picture-uri",
                &uri,
            ]),
            argv(&[
                "gsettings",
                "set",
                "org.gnome.desktop.background",
                "picture-uri-dark",
                &uri,
            ]),
            argv(&[
                "gsettings",
                "set",
                "org.gnome.desktop.background",
                "picture-options",
                "zoom",
            ]),
        ],
        Desktop::Cinnamon => vec![
            argv(&[
                "gsettings",
                "set",
                "org.cinnamon.desktop.background",
                "picture-uri",
                &uri,
            ]),
            argv(&[
                "gsettings",
                "set",
                "org.cinnamon.desktop.background",
                "picture-options",
                "zoom",
            ]),
        ],
        Desktop::Mate => vec![
            argv(&[
                "gsettings",
                "set",
                "org.mate.background",
                "picture-filename",
                &file,
            ]),
            argv(&[
                "gsettings",
                "set",
                "org.mate.background",
                "picture-options",
                "zoom",
            ]),
        ],
        Desktop::Kde => {
            let script = format!(
                "var ds = desktops(); for (var i = 0; i < ds.length; i++) {{ var d = ds[i]; \
                 d.wallpaperPlugin = 'org.kde.image'; \
                 d.currentConfigGroup = Array('Wallpaper', 'org.kde.image', 'General'); \
                 d.writeConfig('Image', '{}'); }}",
                uri.replace('\'', "%27")
            );
            vec![vec![
                "dbus-send".into(),
                "--session".into(),
                "--dest=org.kde.plasmashell".into(),
                "--type=method_call".into(),
                "/PlasmaShell".into(),
                "org.kde.PlasmaShell.evaluateScript".into(),
                format!("string:{script}"),
            ]]
        }
        Desktop::Xfce => xfce_props
            .iter()
            .map(|p| argv(&["xfconf-query", "-c", "xfce4-desktop", "-p", p, "-s", &file]))
            .collect(),
        Desktop::Sway => vec![argv(&["swaymsg", "output", "*", "bg", &file, "fill"])],
        // Shown by our own surface (`wlr::show_still`), not a command.
        Desktop::Wlroots => Vec::new(),
    }
}

/// XFCE keeps one `last-image` property per monitor and workspace.
pub(crate) fn xfce_image_props(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|l| l.ends_with("/last-image"))
        .map(str::to_string)
        .collect()
}

/// The size to render at: the primary output from `xrandr --current`, else
/// the largest connected one.
pub(crate) fn screen_size(xrandr: &str) -> Option<(u32, u32)> {
    let mut best: Option<(bool, u32, u32)> = None;
    for line in xrandr.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.get(1) != Some(&"connected") {
            continue;
        }
        let primary = words.contains(&"primary");
        let Some(geom) = words.iter().find(|w| w.contains('x') && w.contains('+')) else {
            continue; // connected but off
        };
        let (wh, _) = geom.split_once('+')?;
        let (w, h) = wh.split_once('x')?;
        let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) else {
            continue;
        };
        let better = match best {
            None => true,
            Some((bp, bw, bh)) => (primary && !bp) || (primary == bp && w * h > bw * bh),
        };
        if better {
            best = Some((primary, w, h));
        }
    }
    best.map(|(_, w, h)| (w, h))
}

fn run(cmd: &[String]) -> Result<String, PlatformError> {
    let out = Command::new(&cmd[0])
        .args(&cmd[1..])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PlatformError::Unsupported(format!("`{}` is not installed", cmd[0]))
            } else {
                PlatformError::Os(format!("{}: {e}", cmd[0]))
            }
        })?;
    if !out.status.success() {
        return Err(PlatformError::Os(format!(
            "{} failed: {}",
            cmd[0],
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub(crate) struct LinuxHost {
    desktop: Desktop,
}

impl LinuxHost {
    pub(crate) fn new() -> Result<Self, PlatformError> {
        let current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let sway = std::env::var_os("SWAYSOCK").is_some();
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let desktop = detect_desktop(&current, sway, wayland).ok_or_else(|| {
            PlatformError::Unsupported(format!(
                "desktop `{current}` isn't supported yet (GNOME, Cinnamon, MATE, KDE Plasma, XFCE, Sway and Wayland compositors with the wlr layer shell are)"
            ))
        })?;
        if desktop == Desktop::Wlroots {
            crate::wlr::available().map_err(|e| {
                PlatformError::Unsupported(format!("this compositor (`{current}`): {e}"))
            })?;
        }
        Ok(LinuxHost { desktop })
    }

    fn xfce_props(&self) -> Vec<String> {
        run(&argv(&["xfconf-query", "-c", "xfce4-desktop", "-l"]))
            .map(|l| xfce_image_props(&l))
            .unwrap_or_default()
    }
}

/// The whole desktop as one monitor, at the size of its largest output: the
/// compositor's own sizes on Wayland (XWayland's `xrandr` may report scaled
/// ones, or be missing), else `xrandr`, else 1920x1080.
pub(crate) fn desktop_monitor() -> Monitor {
    let wayland = std::env::var_os("WAYLAND_DISPLAY")
        .and_then(|_| crate::wlr::output_sizes().ok())
        .and_then(|sizes| sizes.first().copied());
    let (width, height) = wayland
        .or_else(|| {
            run(&argv(&["xrandr", "--current"]))
                .ok()
                .and_then(|o| screen_size(&o))
        })
        .unwrap_or((1920, 1080));
    Monitor {
        id: DESKTOP_ID.into(),
        x: 0,
        y: 0,
        width,
        height,
        primary: true,
    }
}

impl WallpaperHost for LinuxHost {
    fn name(&self) -> &'static str {
        self.desktop.name()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            per_monitor: false,
            live: false,
        }
    }

    fn monitors(&self) -> Result<Vec<Monitor>, PlatformError> {
        Ok(vec![desktop_monitor()])
    }

    fn current(&self, _monitor: &Monitor) -> Result<Option<PathBuf>, PlatformError> {
        let get = |schema: &str, key: &str| {
            run(&argv(&["gsettings", "get", schema, key])).map(|v| path_from_uri(&v))
        };
        match self.desktop {
            Desktop::Gnome => get("org.gnome.desktop.background", "picture-uri"),
            Desktop::Cinnamon => get("org.cinnamon.desktop.background", "picture-uri"),
            // Plain paths, not URIs: nothing to decode.
            Desktop::Mate => run(&argv(&[
                "gsettings",
                "get",
                "org.mate.background",
                "picture-filename",
            ]))
            .map(|v| plain_path(&v)),
            Desktop::Xfce => match self.xfce_props().first() {
                Some(p) => run(&argv(&["xfconf-query", "-c", "xfce4-desktop", "-p", p]))
                    .map(|v| plain_path(&v)),
                None => Ok(None),
            },
            Desktop::Kde => Ok(crate::config_dir()
                .and_then(|d| {
                    std::fs::read_to_string(d.join("plasma-org.kde.plasma.desktop-appletsrc")).ok()
                })
                .and_then(|t| plasma_image(&t))),
            // Sway doesn't report it; restore can't bring back what it never
            // saw. Our own surface hides what is under it.
            Desktop::Sway | Desktop::Wlroots => Ok(None),
        }
    }

    fn set_static(&self, _monitor: &Monitor, path: &Path) -> Result<(), PlatformError> {
        if !path.is_absolute() {
            return Err(PlatformError::Os(format!(
                "wallpaper path must be absolute: {}",
                path.display()
            )));
        }
        if self.desktop == Desktop::Wlroots {
            // Held by a thread of this process: it lasts while the app runs.
            return crate::wlr::show_still(path).map_err(PlatformError::Os);
        }
        let props = if self.desktop == Desktop::Xfce {
            self.xfce_props()
        } else {
            Vec::new()
        };
        if self.desktop == Desktop::Xfce && props.is_empty() {
            return Err(PlatformError::Os(
                "XFCE lists no wallpaper setting yet: pick any wallpaper once in Desktop settings"
                    .into(),
            ));
        }
        for cmd in set_commands(self.desktop, path, &props) {
            run(&cmd)?;
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<Snapshot, PlatformError> {
        let m = &self.monitors()?[0];
        // Every key as it is (GNOME's dark-mode picture, the picture
        // options), so restore puts back all of it, not one image for both.
        let settings = gsettings_keys(self.desktop)
            .iter()
            .filter_map(|(schema, key)| {
                run(&argv(&["gsettings", "get", schema, key]))
                    .ok()
                    .map(|v| (format!("{schema} {key}"), v.trim().to_string()))
            })
            .collect();
        Ok(Snapshot {
            monitors: vec![(m.id.clone(), self.current(m)?)],
            position: None,
            settings,
        })
    }

    fn restore(&self, snapshot: &Snapshot) -> Result<(), PlatformError> {
        let m = &self.monitors()?[0];
        if !snapshot.settings.is_empty() && !gsettings_keys(self.desktop).is_empty() {
            for cmd in restore_commands(&snapshot.settings) {
                run(&cmd)?;
            }
            // Saved by a version that kept only the light picture.
            let has = |k: &str| snapshot.settings.iter().any(|(key, _)| key.ends_with(k));
            if self.desktop == Desktop::Gnome
                && !has(" picture-uri-dark")
                && let Some((_, v)) = snapshot
                    .settings
                    .iter()
                    .find(|(k, _)| k.ends_with(" picture-uri"))
            {
                run(&argv(&[
                    "gsettings",
                    "set",
                    "org.gnome.desktop.background",
                    "picture-uri-dark",
                    v,
                ]))?;
            }
            return Ok(());
        }
        match snapshot.monitors.first().and_then(|(_, p)| p.as_deref()) {
            Some(path) => self.set_static(m, path),
            // Take our surface away: the compositor's own background shows.
            None if self.desktop == Desktop::Wlroots => {
                crate::wlr::hide_still();
                Ok(())
            }
            // Nothing was seen to put back (Sway doesn't tell, and Plasma's
            // config had no image): say so, and leave Clairlune's still
            // there rather than a desktop pointing at a deleted file.
            None if matches!(self.desktop, Desktop::Sway | Desktop::Kde) => {
                Err(PlatformError::Unsupported(format!(
                    "{} didn't say what the wallpaper was before Clairlune, so it can't be put back; set yours again in the desktop's settings",
                    if self.desktop == Desktop::Sway {
                        "Sway"
                    } else {
                        "Plasma"
                    }
                )))
            }
            // A solid colour: no image to set.
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paths_are_not_decoded() {
        assert_eq!(
            plain_path("'/home/me/a%41b.png'\n").as_deref(),
            Some(Path::new("/home/me/a%41b.png"))
        );
        assert_eq!(
            plain_path("/home/me/it's.png").as_deref(),
            Some(Path::new("/home/me/it's.png"))
        );
        assert_eq!(plain_path("''"), None);
        // A URI's escape at the very end decodes too.
        assert_eq!(
            path_from_uri("'file:///tmp/a%41'").as_deref(),
            Some(Path::new("/tmp/aA"))
        );
    }

    #[test]
    fn saved_gsettings_go_back_word_for_word() {
        let saved = vec![
            (
                "org.gnome.desktop.background picture-uri".to_string(),
                "'file:///usr/share/backgrounds/light.jpg'".to_string(),
            ),
            (
                "org.gnome.desktop.background picture-uri-dark".to_string(),
                "'file:///usr/share/backgrounds/dark.jpg'".to_string(),
            ),
            (
                "org.gnome.desktop.background picture-options".to_string(),
                "'centered'".to_string(),
            ),
        ];
        let cmds = restore_commands(&saved);
        assert_eq!(cmds.len(), 3);
        assert_eq!(
            cmds[1],
            argv(&[
                "gsettings",
                "set",
                "org.gnome.desktop.background",
                "picture-uri-dark",
                "'file:///usr/share/backgrounds/dark.jpg'"
            ])
        );
        assert_eq!(cmds[2][4], "'centered'");
        assert_eq!(gsettings_keys(Desktop::Gnome).len(), 3);
        assert!(gsettings_keys(Desktop::Kde).is_empty());
    }

    #[test]
    fn plasma_image_from_its_config() {
        let rc = "[Containments][1][General]\nImage=nope\n\n\
                  [Containments][1][Wallpaper][org.kde.image][General]\n\
                  Image=file:///usr/share/wallpapers/Next/\nSlidePaths=/x\n";
        assert_eq!(
            plasma_image(rc).as_deref(),
            Some(Path::new("/usr/share/wallpapers/Next/"))
        );
        assert_eq!(plasma_image("[Containments][1][General]\nImage=a\n"), None);
    }

    #[test]
    fn desktops_from_the_environment() {
        assert_eq!(
            detect_desktop("ubuntu:GNOME", false, false),
            Some(Desktop::Gnome)
        );
        assert_eq!(detect_desktop("KDE", false, false), Some(Desktop::Kde));
        assert_eq!(
            detect_desktop("X-Cinnamon", false, false),
            Some(Desktop::Cinnamon)
        );
        assert_eq!(detect_desktop("MATE", false, false), Some(Desktop::Mate));
        assert_eq!(detect_desktop("XFCE", false, false), Some(Desktop::Xfce));
        assert_eq!(
            detect_desktop("Budgie:GNOME", false, false),
            Some(Desktop::Gnome)
        );
        assert_eq!(detect_desktop("", true, false), Some(Desktop::Sway));
        assert_eq!(detect_desktop("Hyprland", false, false), None);
        // Any other Wayland compositor: our own background surface.
        assert_eq!(
            detect_desktop("Hyprland", false, true),
            Some(Desktop::Wlroots)
        );
        assert_eq!(detect_desktop("river", false, true), Some(Desktop::Wlroots));
        // Known desktops keep their own tools on Wayland too.
        assert_eq!(detect_desktop("GNOME", false, true), Some(Desktop::Gnome));
        assert_eq!(detect_desktop("sway", false, true), Some(Desktop::Sway));
    }

    #[test]
    fn uris_round_trip() {
        let p = Path::new("/home/a b/.local/share/clairlune/current/x%y.png");
        let uri = file_uri(p);
        assert_eq!(
            uri,
            "file:///home/a%20b/.local/share/clairlune/current/x%25y.png"
        );
        assert_eq!(path_from_uri(&format!("'{uri}'")).as_deref(), Some(p));
        assert_eq!(path_from_uri("''"), None);
    }

    #[test]
    fn gnome_sets_light_and_dark() {
        let cmds = set_commands(Desktop::Gnome, Path::new("/tmp/w.png"), &[]);
        let keys: Vec<&str> = cmds.iter().map(|c| c[3].as_str()).collect();
        assert_eq!(keys, ["picture-uri", "picture-uri-dark", "picture-options"]);
        assert_eq!(cmds[0][4], "file:///tmp/w.png");
    }

    #[test]
    fn xfce_sets_every_monitor() {
        let listing = "/backdrop/screen0/monitorDP-1/workspace0/last-image\n/backdrop/screen0/monitorDP-1/workspace0/image-style\n/backdrop/screen0/monitorHDMI-1/workspace0/last-image\n";
        let props = xfce_image_props(listing);
        assert_eq!(props.len(), 2);
        let cmds = set_commands(Desktop::Xfce, Path::new("/tmp/w.png"), &props);
        assert_eq!(cmds.len(), 2);
        assert_eq!(
            cmds[1][4],
            "/backdrop/screen0/monitorHDMI-1/workspace0/last-image"
        );
        assert_eq!(cmds[1][6], "/tmp/w.png");
    }

    #[test]
    fn kde_script_names_the_image() {
        let cmds = set_commands(Desktop::Kde, Path::new("/tmp/w.png"), &[]);
        assert_eq!(cmds.len(), 1);
        assert!(
            cmds[0]
                .last()
                .unwrap()
                .contains("writeConfig('Image', 'file:///tmp/w.png')")
        );
    }

    #[test]
    fn screen_size_prefers_the_primary_output() {
        let x = "Screen 0: minimum 320 x 200, current 4480 x 1440, maximum 16384 x 16384\n\
                 HDMI-1 connected 1920x1080+2560+180 (normal left inverted right x axis y axis) 527mm x 296mm\n\
                 DP-1 connected primary 2560x1440+0+0 (normal left inverted right x axis y axis) 597mm x 336mm\n\
                 DP-2 disconnected (normal left inverted right x axis y axis)\n";
        assert_eq!(screen_size(x), Some((2560, 1440)));
        let no_primary =
            "HDMI-1 connected 1920x1080+0+0 (normal)\nDP-1 connected 3840x2160+1920+0 (normal)\n";
        assert_eq!(screen_size(no_primary), Some((3840, 2160)));
        assert_eq!(screen_size("eDP-1 connected (normal)\n"), None);
    }
}
