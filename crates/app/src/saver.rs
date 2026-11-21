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
