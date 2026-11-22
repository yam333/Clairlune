//! The macOS screen saver (`crates/saver`): the app ships it in its bundle
//! (`Clairlune.app/Contents/Resources/Clairlune.saver`), installs it in
//! `~/Library/Screen Savers` when Settings asks, and writes the scene on
//! screen for it (`screensaver.json`) each time the wallpaper changes.
//! macOS 13 lets the app pick it (the screen saver's own defaults); macOS
//! 14 and later let only the user pick the screen saver, so turning it on
//! opens System Settings at Screen Saver there.

use clairlune_scene::saver::{SAVER_SCENE_FILE, SaverScene};
use std::path::{Path, PathBuf};

const BUNDLE: &str = "Clairlune.saver";

/// The saver this app carries (a build from the DMG or Homebrew; not
/// `cargo install`).
pub fn shipped() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let saver = exe.parent()?.parent()?.join("Resources").join(BUNDLE);
    saver.join("Contents").is_dir().then_some(saver)
}

/// Where it goes: `~/Library/Screen Savers/Clairlune.saver`.
pub fn installed_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
    Some(
        PathBuf::from(home)
            .join("Library")
            .join("Screen Savers")
            .join(BUNDLE),
    )
}

/// The installed saver's version (from its `Resources/version`), if any.
pub fn installed_version() -> Option<String> {
    let path = installed_path()?
        .join("Contents")
        .join("Resources")
        .join("version");
    std::fs::read_to_string(path)
        .ok()
        .map(|v| v.trim().to_string())
}

/// Copies the shipped saver into place (replacing an older one), without
/// the quarantine flag the download left on the app: the user already
/// opened the app that carries it.
pub fn install() -> Result<(), String> {
    let from = shipped().ok_or("this copy of Clairlune has no screen saver")?;
    let to = installed_path().ok_or("no home folder")?;
    if to.exists() {
        std::fs::remove_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
    }
    copy_dir(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    let _ = std::process::Command::new("xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(&to)
        .status();
    Ok(())
}

/// Takes the saver away again.
pub fn uninstall() -> Result<(), String> {
    let Some(path) = installed_path() else {
        return Ok(());
    };
    match std::fs::remove_dir_all(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {e}", path.display()))
        }
        _ => Ok(()),
    }
}

/// Writes what the saver plays: in the app's data folder, and in the
/// installed saver's Resources (in case the screen saver host's sandbox
/// keeps it from the first).
pub fn write_scene(data_dir: &Path, scene: &SaverScene) -> Result<(), String> {
    let text = serde_json::to_string(scene).map_err(|e| e.to_string())?;
    let mut targets = vec![data_dir.join(SAVER_SCENE_FILE)];
    if let Some(saver) = installed_path().filter(|p| p.exists()) {
        targets.push(
            saver
                .join("Contents")
                .join("Resources")
                .join(SAVER_SCENE_FILE),
        );
    }
    for path in targets {
        let tmp = path.with_extension("json.part");
        std::fs::write(&tmp, &text)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// The macOS major version (13 for Ventura), from `sw_vers`.
fn macos_major() -> Option<u32> {
    let out = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    parse_major(&String::from_utf8_lossy(&out.stdout))
}

fn parse_major(version: &str) -> Option<u32> {
    version.trim().split('.').next()?.parse().ok()
}

/// Picks the installed saver as the screen saver, where macOS lets an app
/// do that (macOS 13 and earlier); `false` where only the user can.
pub fn select() -> bool {
    if !macos_major().is_some_and(|m| m <= 13) {
        return false;
    }
    let Some(path) = installed_path() else {
        return false;
    };
    std::process::Command::new("defaults")
        .args([
            "-currentHost",
            "write",
            "com.apple.screensaver",
            "moduleDict",
            "-dict",
        ])
        .args(["moduleName", "Clairlune", "path"])
        .arg(&path)
        .args(["type", "0"])
        .status()
        .is_ok_and(|s| s.success())
}

/// Starts the screen saver now (the one picked in System Settings). macOS
/// locks behind it, and it plays until the display turns off; Apple menu >
/// Lock Screen, by contrast, shows only a still picture for a wallpaper
/// that isn't Apple's own, and no third-party saver runs once the screen is
/// locked (mac-lock.yml checks both).
pub fn start_now() {
    let _ = std::process::Command::new("open")
        .args(["-b", "com.apple.ScreenSaver.Engine"])
        .status();
}

/// Whether Clairlune is the screen saver picked in System Settings, or
/// `None` when it can't tell. macOS 14 and later keep the choice in the
/// wallpaper store (an Idle choice naming Clairlune.saver); macOS 13 in the
/// screen saver's own defaults. Read only.
pub fn picked() -> Option<bool> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let store = home
        .join("Library")
        .join("Application Support")
        .join("com.apple.wallpaper")
        .join("Store")
        .join("Index.plist");
    if store.exists() {
        let out = std::process::Command::new("plutil")
            .args(["-convert", "xml1", "-o", "-"])
            .arg(&store)
            .output()
            .ok()?;
        return out
            .status
            .success()
            .then(|| idle_uses_clairlune(&String::from_utf8_lossy(&out.stdout)));
    }
    let out = std::process::Command::new("defaults")
        .args([
            "-currentHost",
            "read",
            "com.apple.screensaver",
            "moduleDict",
        ])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).contains(BUNDLE))
}

/// In the wallpaper store (as XML): whether an Idle (screen saver) choice
/// names Clairlune.saver. Each choice's configuration is a small property
/// list of its own, in base64 under <data>.
fn idle_uses_clairlune(xml: &str) -> bool {
    const IDLE: &str = "<key>Idle</key>";
    const DESKTOP: &str = "<key>Desktop</key>";
    const DATA: &str = "<data>";
    let mut idle = false;
    let mut i = 0;
    loop {
        let next = [IDLE, DESKTOP, DATA]
            .into_iter()
            .filter_map(|m| xml[i..].find(m).map(|p| (p + i, m)))
            .min();
        let Some((at, mark)) = next else {
            return false;
        };
        i = at + mark.len();
        match mark {
            IDLE => idle = true,
            DESKTOP => idle = false,
            _ => {
                let end = xml[i..].find("</data>").map_or(xml.len(), |e| i + e);
                let bytes = base64(&xml[i..end]);
                if idle && bytes.windows(BUNDLE.len()).any(|w| w == BUNDLE.as_bytes()) {
                    return true;
                }
                i = end;
            }
        }
    }
}

/// Standard base64, ignoring whitespace (as plutil writes <data>).
fn base64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// How soon after the screen saver starts macOS asks for the password
/// (System Settings > Lock Screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordAfter {
    Immediately,
    /// After this many seconds.
    Seconds(u32),
    /// Never: the screen saver doesn't lock at all.
    Never,
}

/// What macOS has set, from `sysadminctl -screenLock status` (no admin
/// rights needed to read it).
pub fn password_after() -> Option<PasswordAfter> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("sysadminctl")
        .args(["-screenLock", "status"])
        .output()
        .ok()?;
    // It writes to stderr, through NSLog.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    parse_password_after(&text)
}

fn parse_password_after(text: &str) -> Option<PasswordAfter> {
    let text = text.to_lowercase();
    if text.contains("screenlock is off") {
        return Some(PasswordAfter::Never);
    }
    let rest = text.split("screenlock delay is ").nth(1)?;
    if rest.starts_with("immediate") {
        return Some(PasswordAfter::Immediately);
    }
    let secs = rest.split_whitespace().next()?.parse().ok()?;
    Some(if secs == 0 {
        PasswordAfter::Immediately
    } else {
        PasswordAfter::Seconds(secs)
    })
}

/// System Settings at Lock Screen (when to ask for the password, when the
/// display turns off).
pub fn open_lock_screen_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.Lock-Screen-Settings.extension")
        .status();
}

/// System Settings at Desktop & Dock, where Hot Corners are.
pub fn open_hot_corners() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.Desktop-Settings.extension")
        .status();
}

/// System Settings at Screen Saver, to pick Clairlune.
pub fn open_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.ScreenSaver-Settings.extension")
        .status();
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picked_screen_saver_is_found_in_the_store() {
        // base64 of a configuration naming the saver, and of another one.
        let ours = "YnBsaXN0MDDRAQJWbW9kdWxl0QMEWHJlbGF0aXZlXxA4ZmlsZTovLy9Vc2Vycy9hL0xpYnJhcnkvU2NyZWVuJTIwU2F2ZXJzL0NsYWlybHVuZS5zYXZlci8ICxIVHgAAAAAAAAEBAAAAAAAAAAUAAAAAAAAAAAAAAAAAAABZ";
        let other = "YnBsaXN0MDDRAQJWbW9kdWxl0QMEWHJlbGF0aXZlXxAzZmlsZTovLy9TeXN0ZW0vTGlicmFyeS9TY3JlZW4lMjBTYXZlcnMvSGVsbG8uc2F2ZXIvCAsSFR4AAAAAAAABAQAAAAAAAAAFAAAAAAAAAAAAAAAAAAAAVA==";
        let xml = |desktop: &str, idle: &str| {
            format!(
                "<dict><key>AllSpacesAndDisplays</key><dict><key>Desktop</key><dict><key>Content</key><dict><key>Choices</key><array><dict><key>Configuration</key><data>{desktop}</data></dict></array></dict></dict><key>Idle</key><dict><key>Content</key><dict><key>Choices</key><array><dict><key>Configuration</key><data>\n\t{idle}\n\t</data></dict></array></dict></dict></dict></dict>"
            )
        };
        assert!(idle_uses_clairlune(&xml(other, ours)));
        assert!(!idle_uses_clairlune(&xml(ours, other)));
        assert!(!idle_uses_clairlune("<dict></dict>"));
        assert_eq!(base64("aGk="), b"hi");
    }

    /// On a Mac: what `picked` and `password_after` say here; with
    /// CLAIRLUNE_EXPECT_PICKED=1 or 0 set, it must match (mac-lock.yml).
    #[test]
    #[ignore]
    fn picked_here() {
        let picked = picked();
        eprintln!("picked: {picked:?}, password after: {:?}", password_after());
        if let Ok(want) = std::env::var("CLAIRLUNE_EXPECT_PICKED") {
            assert_eq!(picked, Some(want == "1"));
        }
    }

    #[test]
    fn the_password_delay_is_read() {
        let log = "2026-11-22 09:29:03.584 sysadminctl[5625:20478] ";
        assert_eq!(
            parse_password_after(&format!("{log}screenLock delay is 300 seconds")),
            Some(PasswordAfter::Seconds(300))
        );
        assert_eq!(
            parse_password_after(&format!("{log}screenLock delay is immediate")),
            Some(PasswordAfter::Immediately)
        );
        assert_eq!(
            parse_password_after(&format!("{log}screenLock is off")),
            Some(PasswordAfter::Never)
        );
        assert_eq!(parse_password_after("something else"), None);
    }

    #[test]
    fn the_major_version_is_read() {
        assert_eq!(
            parse_major(
                "13.6.1
"
            ),
            Some(13)
        );
        assert_eq!(parse_major("15.1"), Some(15));
        assert_eq!(parse_major(""), None);
    }

    #[test]
    fn a_folder_is_copied_whole() {
        let base =
            std::env::temp_dir().join(format!("clairlune-saver-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let from = base.join("from.saver");
        std::fs::create_dir_all(from.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(from.join("Contents/Resources")).unwrap();
        std::fs::write(from.join("Contents/Info.plist"), "plist").unwrap();
        std::fs::write(from.join("Contents/MacOS/Clairlune"), "exe").unwrap();
        std::fs::write(from.join("Contents/Resources/version"), "1.2.3\n").unwrap();
        let to = base.join("to.saver");
        copy_dir(&from, &to).unwrap();
        assert_eq!(
            std::fs::read_to_string(to.join("Contents/MacOS/Clairlune")).unwrap(),
            "exe"
        );
        assert_eq!(
            std::fs::read_to_string(to.join("Contents/Resources/version")).unwrap(),
            "1.2.3\n"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }
}
