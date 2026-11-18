//! The `Character` trait and its two implementations: the M1 placeholder pixel
//! map, and sheets exported from the project's own Aseprite files.

use crate::embedded::SHEETS;
use crate::sheet::{Animation, Frame, MAX_CHARACTER_COLORS, ORIGIN, SpriteSheet};
use clairlune_scene::activity::Catalog;
use clairlune_scene::anim::DEFAULT_ONE_SHOT_MS;
use clairlune_scene::placeholder::CLAWD_PLACEHOLDER;
use std::collections::BTreeMap;

/// Name of the sheet holding the shared base animations (idle, walk, reacts).
pub const BASE_SHEET: &str = "_base";

/// Everything the renderer and the state machine need from a character.
pub trait Character {
    /// Animation for an activity's state, or `None` if there is no art for it.
    fn animation(&self, activity: &str, state: &str) -> Option<&Animation>;

    /// Play length of a one-shot state: the sum of its frame durations.
    fn one_shot_ms(&self, activity: &str, state: &str) -> u64 {
        self.animation(activity, state)
            .map(Animation::duration_ms)
            .filter(|&d| d > 0)
            .unwrap_or(DEFAULT_ONE_SHOT_MS)
    }
}

/// The prototype's 15x9 pixel map as a single-frame animation for every state.
#[derive(Clone, Debug)]
pub struct Placeholder {
    anim: Animation,
}

impl Default for Placeholder {
    fn default() -> Self {
        Self::new()
    }
}

impl Placeholder {
    pub fn new() -> Self {
        let m = &CLAWD_PLACEHOLDER;
        let pixels = (0..m.height)
            .flat_map(|y| (0..m.width).map(move |x| m.pixel(x, y)))
            .collect();
        let mut anchors = BTreeMap::new();
        anchors.insert(ORIGIN.to_string(), ((m.width / 2) as i32, m.height as i32));
        let frame = Frame {
            width: m.width,
            height: m.height,
            pixels,
            duration_ms: DEFAULT_ONE_SHOT_MS as u32,
            anchors,
        };
        Self {
            anim: Animation {
                name: "placeholder".into(),
                frames: vec![frame],
            },
        }
    }
}

impl Character for Placeholder {
    fn animation(&self, _activity: &str, _state: &str) -> Option<&Animation> {
        Some(&self.anim)
    }
}

/// Sheets per activity (`<activity>.json`) plus the shared `_base` sheet. Any
/// activity without art falls back to the placeholder, so missing art is never a
/// crash (PLAN.md section 7.1).
#[derive(Clone, Debug)]
pub struct SheetCharacter {
    sheets: BTreeMap<String, SpriteSheet>,
    placeholder: Placeholder,
}

impl SheetCharacter {
    /// The sheets embedded from `assets/clawd/`.
    pub fn builtin() -> Result<Self, Vec<String>> {
        Self::from_sources(SHEETS)
    }

    /// Parses `(name, json, png)` triples. Returns every error at once.
    pub fn from_sources(sources: &[(&str, &str, &[u8])]) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let mut sheets = BTreeMap::new();
        let mut colors = Vec::new();
        for (name, json, png) in sources {
            match SpriteSheet::parse(json, png) {
                Ok(sheet) => {
                    for c in &sheet.colors {
                        if !colors.contains(c) {
                            colors.push(*c);
                        }
                    }
                    sheets.insert(name.to_string(), sheet);
                }
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        if colors.len() > MAX_CHARACTER_COLORS {
            errors.push(format!(
                "all sheets together use {} colours; the character may use at most {MAX_CHARACTER_COLORS}",
                colors.len()
            ));
        }
        if errors.is_empty() {
            Ok(Self {
                sheets,
                placeholder: Placeholder::new(),
            })
        } else {
            Err(errors)
        }
    }

    pub fn has_sheet(&self, activity: &str) -> bool {
        self.sheets.contains_key(activity)
    }

    /// Activities still drawn with the placeholder.
    pub fn missing_art<'c>(&self, catalog: &'c Catalog) -> Vec<&'c str> {
        catalog
            .activities
            .iter()
            .map(|a| a.id.as_str())
            .filter(|id| !self.has_sheet(id))
            .collect()
    }

    /// Every state of an activity that has a sheet must have a tag, in its own
    /// sheet or the base sheet (PLAN.md section 7.2, step 2).
    pub fn validate(&self, catalog: &Catalog) -> Vec<String> {
        let mut errors = Vec::new();
        for a in &catalog.activities {
            let Some(sheet) = self.sheets.get(&a.id) else {
                continue;
            };
            for state in a.animation.states.keys() {
                let in_base = self
                    .sheets
                    .get(BASE_SHEET)
                    .is_some_and(|b| b.animations.contains_key(state));
                if !sheet.animations.contains_key(state) && !in_base {
                    errors.push(format!(
                        "{}: state `{state}` has no tag in {}.aseprite or {BASE_SHEET}.aseprite",
                        a.id, a.id
                    ));
                }
            }
        }
        errors
    }
}

impl Character for SheetCharacter {
    fn animation(&self, activity: &str, state: &str) -> Option<&Animation> {
        self.sheets
            .get(activity)
            .and_then(|s| s.animations.get(state))
            .or_else(|| {
                self.sheets
                    .get(BASE_SHEET)
                    .and_then(|s| s.animations.get(state))
            })
            .or_else(|| self.placeholder.animation(activity, state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::tests::fixture;
    use clairlune_scene::activity::builtin_catalog;

    #[test]
    fn placeholder_matches_the_pixel_map() {
        let p = Placeholder::new();
        let a = p.animation("anything", "any").unwrap();
        let f = &a.frames[0];
        assert_eq!((f.width, f.height), (15, 9));
        assert_eq!(f.origin(), (7, 9));
        assert_eq!(p.one_shot_ms("x", "y"), DEFAULT_ONE_SHOT_MS);
    }

    #[test]
    fn builtin_sheets_are_valid_and_complete() {
        let c = SheetCharacter::builtin().expect("embedded sheets parse");
        let errors = c.validate(builtin_catalog());
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        // Every activity can be drawn, with art or the placeholder.
        for a in &builtin_catalog().activities {
            for s in a.animation.states.keys() {
                assert!(c.animation(&a.id, s).is_some());
            }
        }
    }

    #[test]
    fn validation_reports_states_without_tags() {
        let (json, png) = fixture(2, 0, 0);
        let c = SheetCharacter::from_sources(&[("warm_hands", &json, &png)]).unwrap();
        let errors = c.validate(builtin_catalog()).join("\n");
        assert!(errors.contains("state `warm` has no tag"), "{errors}");
        assert!(
            errors.contains("state `look_around` has no tag"),
            "{errors}"
        );
        // `idle` exists in the fixture, so it isn't reported, and it is used.
        assert!(!errors.contains("`idle`"));
        assert_eq!(c.animation("warm_hands", "idle").unwrap().name, "idle");
        // Unknown states fall back to the placeholder instead of failing.
        assert_eq!(
            c.animation("warm_hands", "warm").unwrap().name,
            "placeholder"
        );
        assert!(c.missing_art(builtin_catalog()).contains(&"stroll"));
    }
}
