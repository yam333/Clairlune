//! Clairlune had a macOS screen saver until 0.17. It's gone: this takes
//! away what an earlier version left on the Mac, the screen saver in
//! `~/Library/Screen Savers` and the scene it played, so System Settings
//! doesn't offer a screen saver that no longer gets a scene.

use std::path::{Path, PathBuf};

const BUNDLE: &str = "Clairlune.saver";
const SCENE_FILE: &str = "screensaver.json";

/// Removes them, if they're there (nothing to do after the first time).
pub fn remove(data_dir: &Path) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from);
    remove_from(home.as_deref(), data_dir);
}

fn remove_from(home: Option<&Path>, data_dir: &Path) {
    let _ = std::fs::remove_file(data_dir.join(SCENE_FILE));
    let Some(home) = home else {
        return;
    };
    let saver = home.join("Library").join("Screen Savers").join(BUNDLE);
    if !saver.join("Contents").is_dir() {
        return;
    }
    match std::fs::remove_dir_all(&saver) {
        Ok(()) => {
            tracing::info!(target: "clairlune", "the screen saver from an earlier version removed")
        }
        Err(e) => {
            tracing::warn!(target: "clairlune", error = %e, "the old screen saver not removed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_saver_and_its_scene_go() {
        let root = std::env::temp_dir().join(format!("clairlune-old-saver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data = root.join("data");
        let saver = root.join("Library/Screen Savers").join(BUNDLE);
        let other = root.join("Library/Screen Savers/Other.saver");
        for d in [
            &data,
            &saver.join("Contents/MacOS"),
            &other.join("Contents"),
        ] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(data.join(SCENE_FILE), "{}").unwrap();
        std::fs::write(saver.join("Contents/MacOS/clairlune_saver"), "x").unwrap();
        std::fs::write(data.join("settings.toml"), "").unwrap();

        remove_from(Some(&root), &data);
        assert!(!saver.exists());
        assert!(!data.join(SCENE_FILE).exists());
        assert!(other.exists(), "another screen saver stays");
        assert!(data.join("settings.toml").exists(), "the app's data stays");

        // A second time, with nothing there, is fine.
        remove_from(Some(&root), &data);
        remove_from(None, &data);
        let _ = std::fs::remove_dir_all(&root);
    }
}
