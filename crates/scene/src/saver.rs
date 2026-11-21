//! What the macOS screen saver plays: the wallpaper on screen, as the app
//! last made it. The app writes this next to its data (and into the saver
//! it installed); the saver reads it when the screen saver starts.

use crate::{SceneRecipe, Theme};
use serde::{Deserialize, Serialize};

/// The file's name, in the app's data folder and in the saver's Resources.
pub const SAVER_SCENE_FILE: &str = "screensaver.json";

/// The scenes on screen, one per monitor, and how they are shown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaverScene {
    pub theme: Theme,
    pub buddy: bool,
    /// Frames a second the saver plays at.
    pub fps: u32,
    pub monitors: Vec<SaverMonitor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaverMonitor {
    pub width: u32,
    pub height: u32,
    pub recipe: SceneRecipe,
}

impl SaverScene {
    /// The scene for a screen of this size: the monitor of that size, else
    /// the one closest in shape, else none.
    pub fn for_size(&self, width: u32, height: u32) -> Option<&SaverMonitor> {
        if let Some(m) = self
            .monitors
            .iter()
            .find(|m| (m.width, m.height) == (width, height))
        {
            return Some(m);
        }
        let aspect = |w: u32, h: u32| f64::from(w) / f64::from(h.max(1));
        let want = aspect(width, height);
        self.monitors.iter().min_by(|a, b| {
            let da = (aspect(a.width, a.height) - want).abs();
            let db = (aspect(b.width, b.height) - want).abs();
            da.total_cmp(&db)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Aspect, ContentContext, compose};

    #[test]
    fn a_screen_gets_its_own_scene_or_the_closest_in_shape() {
        let wide = compose(1, Aspect::from_size(2560, 1080), &ContentContext::default());
        let laptop = compose(2, Aspect::from_size(3024, 1964), &ContentContext::default());
        let s = SaverScene {
            theme: Theme::Dark,
            buddy: true,
            fps: 12,
            monitors: vec![
                SaverMonitor {
                    width: 2560,
                    height: 1080,
                    recipe: wide.clone(),
                },
                SaverMonitor {
                    width: 3024,
                    height: 1964,
                    recipe: laptop.clone(),
                },
            ],
        };
        assert_eq!(s.for_size(3024, 1964).unwrap().recipe, laptop);
        assert_eq!(s.for_size(3440, 1440).unwrap().recipe, wide);
        assert_eq!(s.for_size(1440, 900).unwrap().recipe, laptop);
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<SaverScene>(&text).unwrap(), s);
    }
}
