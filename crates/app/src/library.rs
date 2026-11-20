//! The library (PLAN.md section 9a): wallpapers kept on purpose. Saving copies
//! an archive entry (recipes, thumbnail, images and the frozen scenes), so the
//! archive copy can expire while the library copy stays. Nothing here is ever
//! deleted automatically.
//!
//! ```text
//! <data_dir>/library/<item_id>/
//! ├── item.json        id, name, created, tags, display, theme, buddy, ...
//! ├── recipe.json      as in the archive entry it came from
//! ├── thumb.png
//! ├── wallpaper-0.png  one per monitor
//! └── scene/0/         frozen scene package per monitor (plays exactly as saved)
//! ```

use crate::archive::{Entry, EntryRecipes};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `follow` plus explicit values, as everywhere in the plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    #[default]
    Follow,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuddyChoice {
    #[default]
    Follow,
    On,
    Off,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayChoice {
    #[default]
    Follow,
    Live,
    Still,
}

/// `item.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// Unix seconds.
    pub created: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub display: DisplayChoice,
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default)]
    pub buddy: BuddyChoice,
    /// The base seed and primary landscape, for listings.
    pub seed: u64,
    pub archetype: String,
    pub activity: Option<String>,
    /// The generator that made it (the frozen scene plays regardless).
    pub generator_version: u32,
}

pub struct Library {
    dir: PathBuf,
}

/// Copies a folder tree.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &target)?;
        } else {
            fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

/// `Shore #3f2a`: the landscape and a short form of the seed.
pub fn default_name(archetype: &str, seed: u64) -> String {
    let mut name = archetype.replace('_', " ");
    if let Some(first) = name.get(..1) {
        name = first.to_uppercase() + &name[1..];
    }
    format!("{name} #{:04x}", seed & 0xFFFF)
}

impl Library {
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Library { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// An item's folder.
    pub fn item_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    /// Items, newest first. Unreadable folders are skipped.
    pub fn items(&self) -> Vec<Item> {
        let mut items: Vec<Item> = fs::read_dir(&self.dir)
            .map(|it| {
                it.flatten()
                    .filter_map(|e| fs::read_to_string(e.path().join("item.json")).ok())
                    .filter_map(|t| serde_json::from_str(&t).ok())
                    .collect()
            })
            .unwrap_or_default();
        items.sort_by(|a, b| b.created.cmp(&a.created).then(a.id.cmp(&b.id)));
        items
    }

    pub fn get(&self, id: &str) -> Option<Item> {
        let t = fs::read_to_string(self.item_dir(id).join("item.json")).ok()?;
        serde_json::from_str(&t).ok()
    }

    /// Saves an archive entry into the library (a copy; the entry keeps its
    /// own lifetime). Saving the same entry twice returns the existing item.
    pub fn save_entry(&self, entry_dir: &Path, entry: &Entry, now: u64) -> io::Result<Item> {
        let id = format!(
            "lib_{:08x}",
            clairlune_scene::hash::hash_bytes(entry.name.as_bytes()) as u32
        );
        if let Some(existing) = self.get(&id) {
            return Ok(existing);
        }
        let dir = self.item_dir(&id);
        let tmp = self.dir.join(format!("{id}.part"));
        let _ = fs::remove_dir_all(&tmp);
        copy_tree(entry_dir, &tmp)?;
        let generator_version = fs::read_to_string(tmp.join("recipe.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<EntryRecipes>(&t).ok())
            .and_then(|r| r.monitors.first().map(|m| m.recipe.generator_version))
            .unwrap_or(clairlune_scene::GENERATOR_VERSION);
        let item = Item {
            id: id.clone(),
            name: default_name(&entry.archetype, entry.seed),
            created: now,
            tags: Vec::new(),
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
            seed: entry.seed,
            archetype: entry.archetype.clone(),
            activity: entry.activity.clone(),
            generator_version,
        };
        fs::write(tmp.join("item.json"), serde_json::to_string_pretty(&item)?)?;
        fs::rename(&tmp, &dir)?;
        Ok(item)
    }

    /// Rewrites an item's `item.json` (rename, tags, overrides).
    pub fn update(&self, item: &Item) -> io::Result<()> {
        let dir = self.item_dir(&item.id);
        if !dir.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such item"));
        }
        let tmp = dir.join("item.json.part");
        fs::write(&tmp, serde_json::to_string_pretty(item)?)?;
        fs::rename(tmp, dir.join("item.json"))
    }

    /// Deletes an item (user action; callers check timeline use first).
    pub fn delete(&self, id: &str) -> io::Result<()> {
        fs::remove_dir_all(self.item_dir(id))
    }

    /// The item's recipes (for re-rendering at another size).
    #[cfg(test)]
    pub fn recipes(&self, id: &str) -> io::Result<EntryRecipes> {
        let t = fs::read_to_string(self.item_dir(id).join("recipe.json"))?;
        serde_json::from_str(&t).map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Archive, MonitorScene, NewEntry};
    use clairlune_scene::{Aspect, ContentContext, compose};

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("clairlune-lib-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn archived(dir: &Path) -> (Archive, Entry) {
        let archive = Archive::open(dir.join("archive")).unwrap();
        let recipe = compose(11, Aspect::from_size(16, 9), &ContentContext::default());
        let pkg = clairlune_render::ScenePackage::capture(&recipe, 320, 180).unwrap();
        let entry = archive
            .add(&NewEntry {
                created: 100,
                recipes: EntryRecipes {
                    base_seed: 11,
                    theme: "dark".into(),
                    monitors: vec![MonitorScene {
                        monitor: "m".into(),
                        width: 320,
                        height: 180,
                        recipe,
                    }],
                },
                pngs: vec![vec![1, 2, 3]],
                thumb_png: vec![4, 5],
                packages: vec![pkg],
            })
            .unwrap();
        (archive, entry)
    }

    #[test]
    fn saving_copies_the_entry_and_survives_its_deletion() {
        let d = temp("save");
        let (archive, entry) = archived(&d);
        let lib = Library::open(d.join("library")).unwrap();
        let item = lib
            .save_entry(&archive.dir().join(&entry.name), &entry, 200)
            .unwrap();
        assert!(item.id.starts_with("lib_"));
        assert_eq!(item.seed, 11);
        assert_eq!(item.theme, ThemeChoice::Follow);
        // Saving again finds the same item.
        let again = lib
            .save_entry(&archive.dir().join(&entry.name), &entry, 300)
            .unwrap();
        assert_eq!(again, item);
        assert_eq!(lib.items().len(), 1);
        // The archive copy goes; the library copy and its scene stay.
        fs::remove_dir_all(archive.dir().join(&entry.name)).unwrap();
        let scene = lib.item_dir(&item.id).join("scene").join("0");
        assert!(clairlune_render::ScenePackage::load(&scene).is_ok());
        assert_eq!(lib.recipes(&item.id).unwrap().base_seed, 11);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rename_tags_overrides_and_delete() {
        let d = temp("edit");
        let (archive, entry) = archived(&d);
        let lib = Library::open(d.join("library")).unwrap();
        let mut item = lib
            .save_entry(&archive.dir().join(&entry.name), &entry, 200)
            .unwrap();
        item.name = "Evening shore".into();
        item.tags = vec!["evening".into()];
        item.theme = ThemeChoice::Dark;
        item.buddy = BuddyChoice::Off;
        lib.update(&item).unwrap();
        assert_eq!(lib.get(&item.id), Some(item.clone()));
        lib.delete(&item.id).unwrap();
        assert!(lib.items().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn default_names() {
        assert_eq!(default_name("night_sky", 0x1234_abcd), "Night sky #abcd");
        assert_eq!(default_name("cave", 7), "Cave #0007");
    }
}
