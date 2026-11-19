//! The archive: every wallpaper set the app applied, kept for a while
//! (PLAN.md section 9).
//!
//! ```text
//! <data_dir>/archive/
//! ├── index.json
//! └── 2026-11-18T17-56-03Z_seed-1234567890/
//!     ├── recipe.json      one recipe per monitor
//!     ├── thumb.png        480 px wide, the primary monitor's scene
//!     └── wallpaper-0.png  one per monitor, primary first
//! ```
//!
//! Retention deletes for good (never to the Trash, which would fill up every
//! hour): first unpinned entries older than the retention period, then the
//! oldest unpinned ones while over the size cap. The newest entry (the
//! wallpaper on screen) and pinned entries are never swept.

use clairlune_scene::SceneRecipe;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One archived wallpaper set, as listed in `index.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Directory name: `<UTC stamp>_seed-<seed>`.
    pub name: String,
    /// Unix seconds.
    pub created: u64,
    /// The base seed (the primary monitor's).
    pub seed: u64,
    pub theme: String,
    /// The primary monitor's landscape and activity, for listings.
    pub archetype: String,
    pub activity: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    pub bytes: u64,
}

/// One monitor's scene in an entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorScene {
    pub monitor: String,
    pub width: u32,
    pub height: u32,
    pub recipe: SceneRecipe,
}

/// `recipe.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntryRecipes {
    pub base_seed: u64,
    pub theme: String,
    pub monitors: Vec<MonitorScene>,
}

/// What to archive: the recipes plus the rendered images.
pub struct NewEntry {
    pub created: u64,
    pub recipes: EntryRecipes,
    /// One PNG per monitor, in `recipes.monitors` order.
    pub pngs: Vec<Vec<u8>>,
    pub thumb_png: Vec<u8>,
}

#[derive(Default, Serialize, Deserialize)]
struct Index {
    /// Oldest first.
    entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sweep {
    pub removed: usize,
    pub freed: u64,
}

pub struct Archive {
    dir: PathBuf,
}

impl Archive {
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Archive { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    fn load_index(&self) -> Index {
        fs::read_to_string(self.index_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_index(&self, index: &Index) -> io::Result<()> {
        let tmp = self.dir.join("index.json.part");
        fs::write(&tmp, serde_json::to_string_pretty(index)?)?;
        fs::rename(tmp, self.index_path())
    }

    /// Entries, oldest first.
    pub fn entries(&self) -> Vec<Entry> {
        self.load_index().entries
    }

    /// The newest entry (the wallpaper on screen, when the app set it).
    pub fn latest(&self) -> Option<Entry> {
        self.entries().pop()
    }

    /// The entry before `name`, for "Previous wallpaper".
    pub fn before(&self, name: &str) -> Option<Entry> {
        let entries = self.entries();
        let i = entries.iter().position(|e| e.name == name)?;
        i.checked_sub(1).map(|j| entries[j].clone())
    }

    /// Writes an entry and lists it.
    pub fn add(&self, new: &NewEntry) -> io::Result<Entry> {
        let base = format!(
            "{}_seed-{}",
            crate::timefmt::stamp(new.created),
            new.recipes.base_seed
        );
        let mut name = base.clone();
        let mut n = 2;
        while self.dir.join(&name).exists() {
            name = format!("{base}-{n}");
            n += 1;
        }
        let dir = self.dir.join(&name);
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join("recipe.json"),
            serde_json::to_string_pretty(&new.recipes)?,
        )?;
        fs::write(dir.join("thumb.png"), &new.thumb_png)?;
        for (i, png) in new.pngs.iter().enumerate() {
            fs::write(dir.join(format!("wallpaper-{i}.png")), png)?;
        }
        let primary = new.recipes.monitors.first();
        let entry = Entry {
            name,
            created: new.created,
            seed: new.recipes.base_seed,
            theme: new.recipes.theme.clone(),
            archetype: primary
                .map(|m| m.recipe.archetype.clone())
                .unwrap_or_default(),
            activity: primary.and_then(|m| m.recipe.activity.clone()),
            pinned: false,
            bytes: dir_size(&dir),
        };
        let mut index = self.load_index();
        index.entries.push(entry.clone());
        self.save_index(&index)?;
        Ok(entry)
    }

    /// The recipes of an entry.
    pub fn recipes(&self, entry: &Entry) -> io::Result<EntryRecipes> {
        let text = fs::read_to_string(self.dir.join(&entry.name).join("recipe.json"))?;
        serde_json::from_str(&text).map_err(io::Error::other)
    }

    /// Applies retention: unpinned entries older than `keep_days` (0 keeps
    /// forever), then the oldest unpinned while their total exceeds `max_bytes`.
    pub fn sweep(&self, now: u64, keep_days: u32, max_bytes: u64) -> Sweep {
        let mut index = self.load_index();
        let newest = index.entries.last().map(|e| e.name.clone());
        let sweepable = |e: &Entry| !e.pinned && Some(&e.name) != newest.as_ref();
        let mut doomed: Vec<String> = Vec::new();
        if keep_days > 0 {
            let cutoff = now.saturating_sub(u64::from(keep_days) * 86_400);
            doomed.extend(
                index
                    .entries
                    .iter()
                    .filter(|e| sweepable(e) && e.created < cutoff)
                    .map(|e| e.name.clone()),
            );
        }
        let mut total: u64 = index
            .entries
            .iter()
            .filter(|e| !e.pinned && !doomed.contains(&e.name))
            .map(|e| e.bytes)
            .sum();
        for e in &index.entries {
            if total <= max_bytes {
                break;
            }
            if sweepable(e) && !doomed.contains(&e.name) {
                total -= e.bytes;
                doomed.push(e.name.clone());
            }
        }
        let mut report = Sweep::default();
        index.entries.retain(|e| {
            if !doomed.contains(&e.name) {
                return true;
            }
            match fs::remove_dir_all(self.dir.join(&e.name)) {
                Ok(()) => {
                    report.removed += 1;
                    report.freed += e.bytes;
                    false
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => false,
                // Locked (being viewed?): keep it listed and try next sweep.
                Err(_) => true,
            }
        });
        if report.removed > 0 {
            let _ = self.save_index(&index);
        }
        report
    }
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clairlune_scene::{Aspect, ContentContext, compose};

    fn temp(name: &str) -> Archive {
        let d =
            std::env::temp_dir().join(format!("clairlune-archive-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        Archive::open(d).unwrap()
    }

    fn entry(seed: u64, created: u64, bytes: usize) -> NewEntry {
        let recipe = compose(seed, Aspect::from_size(16, 9), &ContentContext::default());
        NewEntry {
            created,
            recipes: EntryRecipes {
                base_seed: seed,
                theme: "dark".into(),
                monitors: vec![MonitorScene {
                    monitor: "m0".into(),
                    width: 1920,
                    height: 1080,
                    recipe,
                }],
            },
            pngs: vec![vec![7; bytes]],
            thumb_png: vec![1; 10],
        }
    }

    const DAY: u64 = 86_400;

    #[test]
    fn add_lists_and_round_trips_recipes() {
        let a = temp("add");
        let e1 = a.add(&entry(1, 10 * DAY, 100)).unwrap();
        let e2 = a.add(&entry(2, 11 * DAY, 100)).unwrap();
        assert_eq!(a.entries(), vec![e1.clone(), e2.clone()]);
        assert_eq!(a.latest().unwrap().seed, 2);
        assert_eq!(a.before(&e2.name).unwrap().name, e1.name);
        assert!(a.before(&e1.name).is_none());
        let r = a.recipes(&e1).unwrap();
        assert_eq!(r.base_seed, 1);
        assert_eq!(
            r.monitors[0].recipe,
            entry(1, 0, 0).recipes.monitors[0].recipe
        );
        assert!(e1.bytes >= 110);
        fs::remove_dir_all(a.dir()).unwrap();
    }

    #[test]
    fn same_second_same_seed_gets_a_suffix() {
        let a = temp("dup");
        let x = a.add(&entry(5, DAY, 1)).unwrap();
        let y = a.add(&entry(5, DAY, 1)).unwrap();
        assert_ne!(x.name, y.name);
        assert!(y.name.ends_with("-2"));
        fs::remove_dir_all(a.dir()).unwrap();
    }

    #[test]
    fn sweep_by_age_keeps_pinned_and_newest() {
        let a = temp("age");
        let old = a.add(&entry(1, DAY, 10)).unwrap();
        let pinned = a.add(&entry(2, 2 * DAY, 10)).unwrap();
        let mut idx = a.load_index();
        idx.entries[1].pinned = true;
        a.save_index(&idx).unwrap();
        let newest = a.add(&entry(3, 3 * DAY, 10)).unwrap();
        // 30 days later with 7 days' retention: only the old unpinned one goes,
        // even though the newest is also past the cutoff.
        let s = a.sweep(33 * DAY, 7, u64::MAX);
        assert_eq!(s.removed, 1);
        let names: Vec<String> = a.entries().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec![pinned.name, newest.name]);
        assert!(!a.dir().join(&old.name).exists());
        fs::remove_dir_all(a.dir()).unwrap();
    }

    #[test]
    fn sweep_by_size_removes_oldest_first() {
        let a = temp("size");
        let e: Vec<Entry> = (0..4)
            .map(|i| a.add(&entry(i, DAY * (i + 1), 1000)).unwrap())
            .collect();
        // Room for the two newest entries (sizes differ a little with the
        // recipe): the two oldest go.
        let s = a.sweep(5 * DAY, 0, e[2].bytes + e[3].bytes);
        assert_eq!(s.removed, 2);
        assert_eq!(s.freed, e[0].bytes + e[1].bytes);
        let left: Vec<u64> = a.entries().into_iter().map(|e| e.seed).collect();
        assert_eq!(left, vec![2, 3]);
        fs::remove_dir_all(a.dir()).unwrap();
    }
}
