//! Where stills live while they are the wallpaper.
//!
//! macOS and GNOME don't reload an unchanged path even when the file changed,
//! so every still gets a new file name: `<monitor key>-<time>-<n>.png`. The
//! previous files for that monitor are deleted only after the new one is set
//! (PLAN.md section 2, "Unique file names").

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// The wallpaper as it was before Clairlune first set one, so it can be put
/// back: each monitor's image (or none) and the system's fit mode.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Monitor id and its image path; `None` for a solid colour.
    pub monitors: Vec<(String, Option<PathBuf>)>,
    /// The OS fit mode (on Windows a `DESKTOP_WALLPAPER_POSITION` value).
    pub position: Option<i32>,
}

const SNAPSHOT_FILE: &str = "previous-wallpaper.json";

/// Disambiguates files written within the same millisecond.
static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct StillStore {
    dir: PathBuf,
}

impl StillStore {
    /// A store in `dir` (created when missing).
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(StillStore { dir })
    }

    /// The default store: `<local data dir>/clairlune/current` (Local AppData on
    /// Windows, so large regenerated images never roam with the profile).
    pub fn default_location() -> std::io::Result<Self> {
        let base = crate::app_data_dir().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no data directory")
        })?;
        Self::new(base.join("current"))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Writes a still for a monitor under a new, absolute file name.
    pub fn write(&self, monitor_id: &str, png: &[u8]) -> std::io::Result<PathBuf> {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = self
            .dir
            .join(format!("{}-{ms}-{n}.png", monitor_key(monitor_id)));
        let tmp = path.with_extension("png.part");
        fs::write(&tmp, png)?;
        fs::rename(&tmp, &path)?;
        std::path::absolute(&path)
    }

    /// Saves the wallpaper `restore` returns to, unless it is Clairlune's own
    /// (every monitor showing a still from this store). So the user's wallpaper
    /// is remembered on the first takeover, and a wallpaper they pick later
    /// replaces it, but Clairlune never remembers itself.
    pub fn remember_if_users(&self, snapshot: &Snapshot) -> std::io::Result<bool> {
        let ours = !snapshot.monitors.is_empty()
            && snapshot
                .monitors
                .iter()
                .all(|(_, p)| p.as_deref().is_some_and(|p| p.starts_with(&self.dir)));
        if ours {
            return Ok(false);
        }
        let json = serde_json::to_string_pretty(snapshot).map_err(std::io::Error::other)?;
        let tmp = self.dir.join(format!("{SNAPSHOT_FILE}.part"));
        fs::write(&tmp, json)?;
        fs::rename(tmp, self.dir.join(SNAPSHOT_FILE))?;
        Ok(true)
    }

    /// The saved pre-Clairlune wallpaper, if any.
    pub fn original(&self) -> std::io::Result<Option<Snapshot>> {
        let path = self.dir.join(SNAPSHOT_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(std::io::Error::other),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Forgets the saved wallpaper (after restoring it).
    pub fn forget_original(&self) -> std::io::Result<()> {
        match fs::remove_file(self.dir.join(SNAPSHOT_FILE)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Deletes every still for every monitor (after restoring).
    pub fn clear_stills(&self) -> usize {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return 0;
        };
        entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .filter(|e| fs::remove_file(e.path()).is_ok())
            .count()
    }

    /// Deletes this monitor's other stills, keeping `keep`. Files that are
    /// still locked (the OS may hold the old one briefly) are left for next time.
    pub fn prune(&self, monitor_id: &str, keep: &Path) -> usize {
        let prefix = format!("{}-", monitor_key(monitor_id));
        let keep_name = keep.file_name();
        let mut removed = 0;
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return 0;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            let is_ours = name.to_string_lossy().starts_with(&prefix);
            if is_ours && Some(name.as_os_str()) != keep_name && fs::remove_file(e.path()).is_ok() {
                removed += 1;
            }
        }
        removed
    }
}

/// A short, file-name-safe key for a monitor id (device paths contain `\`,
/// `#`, `{` and the like).
pub fn monitor_key(monitor_id: &str) -> String {
    format!(
        "{:016x}",
        clairlune_scene::hash::hash_bytes(monitor_id.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> StillStore {
        let dir = std::env::temp_dir().join(format!(
            "clairlune-store-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        StillStore::new(dir).unwrap()
    }

    #[test]
    fn every_write_gets_a_new_name_and_prune_keeps_the_current_one() {
        let s = temp_store("names");
        let a = s.write(r"\\?\DISPLAY#ABC#1", b"one").unwrap();
        let b = s.write(r"\\?\DISPLAY#ABC#1", b"two").unwrap();
        let other = s.write(r"\\?\DISPLAY#XYZ#2", b"three").unwrap();
        assert_ne!(a, b);
        assert!(a.is_absolute());
        assert_eq!(s.prune(r"\\?\DISPLAY#ABC#1", &b), 1);
        assert!(!a.exists() && b.exists() && other.exists());
        assert_eq!(fs::read(&b).unwrap(), b"two");
        fs::remove_dir_all(s.dir()).unwrap();
    }

    #[test]
    fn remembers_the_users_wallpaper_never_its_own() {
        let s = temp_store("snap");
        let original = Snapshot {
            monitors: vec![
                ("m1".into(), Some(PathBuf::from(r"C:\pics\sea.jpg"))),
                ("m2".into(), None),
            ],
            position: Some(4),
        };
        let ours = Snapshot {
            monitors: vec![
                ("m1".into(), Some(s.dir().join("a.png"))),
                ("m2".into(), Some(s.dir().join("b.png"))),
            ],
            position: Some(4),
        };
        let later = Snapshot {
            monitors: vec![("m1".into(), Some(PathBuf::from(r"C:\pics\new.jpg")))],
            position: Some(2),
        };
        assert!(s.remember_if_users(&original).unwrap());
        assert!(!s.remember_if_users(&ours).unwrap());
        assert_eq!(s.original().unwrap(), Some(original));
        assert!(s.remember_if_users(&later).unwrap());
        assert_eq!(s.original().unwrap(), Some(later));
        s.forget_original().unwrap();
        assert_eq!(s.original().unwrap(), None);
        fs::remove_dir_all(s.dir()).unwrap();
    }

    #[test]
    fn monitor_keys_are_file_name_safe_and_stable() {
        let k = monitor_key(r"\\?\DISPLAY#DEL40F7#5&2b3e0f3&0&UID4352#{e6f07b5f}");
        assert_eq!(k.len(), 16);
        assert!(k.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(
            k,
            monitor_key(r"\\?\DISPLAY#DEL40F7#5&2b3e0f3&0&UID4352#{e6f07b5f}")
        );
    }
}
