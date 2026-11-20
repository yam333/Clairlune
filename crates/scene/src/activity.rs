//! Activities: what the character is doing (PLAN.md section 4.7).
//!
//! One TOML file per activity in `crates/scene/activities/`, embedded at compile
//! time. The engine never hardcodes a specific activity: adding a file (and its
//! sprites) is how content grows. Maps are `BTreeMap` so iteration order, and
//! therefore every seeded pick, is deterministic.

use crate::archetype::{ArchetypeInfo, SURFACES, TERRAINS, WEATHERS};
use crate::hash::StableHasher;
use crate::recipe::TimeOfDay;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Prop ids the engine can draw.
pub const PROPS: [&str; 12] = [
    "campfire",
    "log_seat",
    "telescope",
    "lantern",
    "spellbook",
    "camp_stove",
    "coffee_mug",
    "fallen_hat",
    "book_closed",
    "guitar_rest",
    "bucket",
    "kite_rest",
];
/// Effect ids the engine can draw.
pub const EFFECTS: [&str; 1] = ["embers"];
/// Particle presets an `[[effects]]` entry can attach to an anchor.
pub const PARTICLE_PRESETS: [&str; 3] = ["sparkles", "steam", "notes"];
/// Tether presets: a line from an anchor to a moving end (a float on the
/// water, a kite in the sky).
pub const TETHER_PRESETS: [&str; 2] = ["fishing_line", "kite_string"];
/// Which way the character faces, when it matters: `water` (the open water,
/// on the shore) or `open` (the middle of the screen, where the sky is
/// widest).
pub const FACINGS: [&str; 2] = ["water", "open"];
/// Anchor names the art exports (docs/clawd-style.md).
pub const ANCHORS: [&str; 6] = ["origin", "feet", "head", "hand_l", "hand_r", "item_tip"];
/// Generic prop attachment points.
pub const ATTACH: [&str; 4] = ["path", "left_of_clawd", "right_of_clawd", "either_side"];

/// Embedded activity files, sorted by file name.
const SOURCES: [(&str, &str); 9] = [
    ("fishing.toml", include_str!("../activities/fishing.toml")),
    ("fly_kite.toml", include_str!("../activities/fly_kite.toml")),
    (
        "make_coffee.toml",
        include_str!("../activities/make_coffee.toml"),
    ),
    (
        "play_guitar.toml",
        include_str!("../activities/play_guitar.toml"),
    ),
    (
        "read_lantern.toml",
        include_str!("../activities/read_lantern.toml"),
    ),
    ("stargaze.toml", include_str!("../activities/stargaze.toml")),
    ("stroll.toml", include_str!("../activities/stroll.toml")),
    (
        "warm_hands.toml",
        include_str!("../activities/warm_hands.toml"),
    ),
    (
        "wizard_magic.toml",
        include_str!("../activities/wizard_magic.toml"),
    ),
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub id: String,
    #[serde(default = "one")]
    pub weight: f64,
    pub requires: Requires,
    pub motion: Motion,
    #[serde(default)]
    pub props: BTreeMap<String, PropDef>,
    pub animation: AnimationDef,
    #[serde(default)]
    pub effects_by_state: BTreeMap<String, Vec<String>>,
    pub gif: Option<GifDef>,
    pub unattended: Unattended,
    #[serde(default)]
    pub character: CharacterDef,
    /// Effects attached to the character (docs/activity-system.md section 3).
    #[serde(default)]
    pub effects: Vec<EffectDef>,
}

/// Per-activity character settings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterDef {
    /// Extra cleared space above the character's box, in sprite pixels
    /// (a wizard hat, a raised wand).
    #[serde(default)]
    pub headroom: u32,
}

/// An effect attached to an anchor of the character's current frame.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EffectDef {
    /// `particles` or `tether`.
    #[serde(rename = "type")]
    pub kind: String,
    /// One of [`PARTICLE_PRESETS`] or [`TETHER_PRESETS`], by type.
    pub preset: String,
    /// Anchor name, e.g. `item_tip`.
    pub at: String,
    /// States in which the effect runs; empty means all.
    #[serde(default)]
    pub states: Vec<String>,
}

impl EffectDef {
    pub fn active_in(&self, state: &str) -> bool {
        self.states.is_empty() || self.states.iter().any(|s| s == state)
    }
}

fn one() -> f64 {
    1.0
}

/// Capability requirements. An empty list means "no constraint".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    #[serde(default)]
    pub terrain: Vec<String>,
    #[serde(default)]
    pub surface: Vec<String>,
    pub time_of_day: Vec<TimeOfDay>,
    #[serde(default)]
    pub weather: Vec<String>,
    /// Needs open sky overhead (stargazing): rules out caves and interiors.
    #[serde(default)]
    pub sky: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MotionKind {
    Stationary,
    Pace,
    Path,
    Bob,
    Fly,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    #[serde(rename = "type")]
    pub kind: MotionKind,
    pub path: Option<String>,
    pub speed_cells_per_s: Option<[f64; 2]>,
    pub at_path_end: Option<String>,
    /// One of [`FACINGS`].
    #[serde(default)]
    pub face: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PropDef {
    pub attach: String,
    #[serde(default)]
    pub effects: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AnimationDef {
    pub start: String,
    pub states: BTreeMap<String, StateDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateDef {
    #[serde(default, rename = "loop")]
    pub looping: bool,
    pub dwell_s: Option<[f64; 2]>,
    #[serde(default)]
    pub next: BTreeMap<String, f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GifDef {
    pub loop_script: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Unattended {
    pub props: Vec<String>,
}

/// Integer weight for seeded picks (weights in files are decimals).
pub fn int_weight(w: f64) -> u32 {
    (w * 1000.0).round().max(0.0) as u32
}

fn overlaps(need: &[String], offer: &[&str]) -> bool {
    need.is_empty() || need.iter().any(|n| offer.contains(&n.as_str()))
}

impl Activity {
    /// Whether this activity can take place in an archetype, given enabled weather.
    pub fn fits(&self, a: &ArchetypeInfo, enabled_weather: &[String]) -> bool {
        overlaps(&self.requires.terrain, a.terrain)
            && overlaps(&self.requires.surface, a.surface)
            && (!self.requires.sky || a.sky)
            && !self.weathers_in(a, enabled_weather).is_empty()
    }

    /// Weather that the activity, the archetype and the user all allow.
    pub fn weathers_in(&self, a: &ArchetypeInfo, enabled: &[String]) -> Vec<&'static str> {
        a.weather
            .iter()
            .copied()
            .filter(|w| {
                self.requires.weather.is_empty() || self.requires.weather.iter().any(|r| r == w)
            })
            .filter(|w| enabled.iter().any(|e| e == w))
            .collect()
    }

    /// Allowed times of day for one theme family, in canonical order.
    pub fn times_in(&self, family: &[TimeOfDay]) -> Vec<TimeOfDay> {
        family
            .iter()
            .copied()
            .filter(|t| self.requires.time_of_day.contains(t))
            .collect()
    }

    /// The state that follows `state` (when no `next`, back to `start`).
    pub fn successors(&self, state: &str) -> Vec<(&str, u32)> {
        match self.animation.states.get(state) {
            Some(s) if !s.next.is_empty() => s
                .next
                .iter()
                .map(|(k, w)| (k.as_str(), int_weight(*w)))
                .collect(),
            _ => vec![(self.animation.start.as_str(), 1)],
        }
    }

    /// Every rule from PLAN.md section 4.7 that can be checked without sprites.
    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        let id = &self.id;
        let mut err = |m: String| errs.push(format!("{id}: {m}"));
        if self.weight.is_nan() || self.weight <= 0.0 {
            err("weight must be positive".into());
        }
        for t in &self.requires.terrain {
            if !TERRAINS.contains(&t.as_str()) {
                err(format!("unknown terrain `{t}`"));
            }
        }
        for s in &self.requires.surface {
            if !SURFACES.contains(&s.as_str()) {
                err(format!("unknown surface `{s}`"));
            }
        }
        for w in &self.requires.weather {
            if !WEATHERS.contains(&w.as_str()) {
                err(format!("unknown weather `{w}`"));
            }
        }
        if self.requires.time_of_day.is_empty() {
            err("requires.time_of_day is empty".into());
        }
        let states = &self.animation.states;
        if !states.contains_key(&self.animation.start) {
            err(format!(
                "start state `{}` is not defined",
                self.animation.start
            ));
        }
        for (name, s) in states {
            for (to, w) in &s.next {
                if !states.contains_key(to) {
                    err(format!("state `{name}` goes to undefined state `{to}`"));
                }
                if w.is_nan() || *w <= 0.0 {
                    err(format!(
                        "state `{name}`: weight for `{to}` must be positive"
                    ));
                }
            }
            match (s.looping, s.dwell_s) {
                (true, None) => err(format!("looping state `{name}` needs dwell_s")),
                (true, Some([lo, hi])) if !(lo >= 0.2 && hi >= lo) => err(format!(
                    "state `{name}`: dwell_s must be [lo, hi] with 0.2 <= lo <= hi"
                )),
                (false, Some(_)) => err(format!(
                    "state `{name}`: dwell_s only applies to looping states"
                )),
                _ => {}
            }
        }
        for (state, effects) in &self.effects_by_state {
            if !states.contains_key(state) {
                err(format!("effects_by_state names undefined state `{state}`"));
            }
            for e in effects {
                if !EFFECTS.contains(&e.as_str()) {
                    err(format!("unknown effect `{e}`"));
                }
            }
        }
        for (prop, def) in &self.props {
            if !PROPS.contains(&prop.as_str()) {
                err(format!("unknown prop `{prop}`"));
            }
            if !ATTACH.contains(&def.attach.as_str()) {
                err(format!("prop `{prop}`: unknown attach `{}`", def.attach));
            }
            for e in &def.effects {
                if !EFFECTS.contains(&e.as_str()) {
                    err(format!("prop `{prop}`: unknown effect `{e}`"));
                }
            }
        }
        if let Some(face) = &self.motion.face {
            if !FACINGS.contains(&face.as_str()) {
                err(format!("unknown facing `{face}`"));
            }
            if face == "water" && self.requires.terrain != ["shore"] {
                err("facing the water needs requires.terrain = [\"shore\"]".into());
            }
        }
        for e in &self.effects {
            match e.kind.as_str() {
                "particles" if !PARTICLE_PRESETS.contains(&e.preset.as_str()) => {
                    err(format!("unknown particle preset `{}`", e.preset));
                }
                "tether" if !TETHER_PRESETS.contains(&e.preset.as_str()) => {
                    err(format!("unknown tether preset `{}`", e.preset));
                }
                "particles" | "tether" => {}
                other => err(format!(
                    "effect type `{other}` is not implemented (`particles`, `tether`)"
                )),
            }
            if e.preset == "fishing_line" && self.requires.terrain != ["shore"] {
                err("a fishing line needs requires.terrain = [\"shore\"]".into());
            }
            if !ANCHORS.contains(&e.at.as_str()) {
                err(format!("effect anchor `{}` is not a known anchor", e.at));
            }
            for s in &e.states {
                if !states.contains_key(s) {
                    err(format!("effect names undefined state `{s}`"));
                }
            }
        }
        if self.unattended.props.is_empty() {
            err("unattended.props is empty: Buddy-off scenes need a focal object".into());
        }
        for p in &self.unattended.props {
            if !PROPS.contains(&p.as_str()) && !EFFECTS.contains(&p.as_str()) {
                err(format!("unattended: unknown prop or effect `{p}`"));
            }
        }
        match self.motion.kind {
            MotionKind::Stationary => {}
            MotionKind::Pace => match self.motion.speed_cells_per_s {
                Some([lo, hi]) if lo > 0.0 && hi >= lo => {}
                _ => err("pace motion needs speed_cells_per_s = [lo, hi] with 0 < lo <= hi".into()),
            },
            MotionKind::Path | MotionKind::Bob | MotionKind::Fly => err(format!(
                "motion `{:?}` is not implemented yet (milestone M4)",
                self.motion.kind
            )),
        }
        if let Some(gif) = &self.gif {
            let script = &gif.loop_script;
            if script.first() != Some(&self.animation.start) {
                err("gif.loop_script must start with the start state".into());
            }
            for pair in script.windows(2).chain(std::iter::once(
                [
                    script.last().cloned().unwrap_or_default(),
                    script.first().cloned().unwrap_or_default(),
                ]
                .as_slice(),
            )) {
                let (from, to) = (&pair[0], &pair[1]);
                if !states.contains_key(from) {
                    err(format!("gif.loop_script names undefined state `{from}`"));
                } else if !self.successors(from).iter().any(|(s, _)| s == to) {
                    err(format!("gif.loop_script: `{from}` can't go to `{to}`"));
                }
            }
        }
        errs
    }
}

/// The activity catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub activities: Vec<Activity>,
    /// Hash of the source files; part of the content context.
    pub data_hash: u64,
}

impl Catalog {
    /// Parses and validates the embedded activity files. Panics on invalid data,
    /// which CI catches through the tests.
    pub fn builtin() -> Self {
        Self::from_sources(&SOURCES)
            .unwrap_or_else(|e| panic!("invalid built-in activities:\n{}", e.join("\n")))
    }

    /// Parses `(file name, toml)` pairs. Returns every error at once.
    pub fn from_sources(sources: &[(&str, &str)]) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let mut activities = Vec::new();
        let mut h = StableHasher::new();
        for (name, text) in sources {
            h.write_str(name);
            write_text(&mut h, text);
            match toml::from_str::<Activity>(text) {
                Ok(a) => {
                    if format!("{}.toml", a.id) != *name {
                        errors.push(format!("{name}: id `{}` must match the file name", a.id));
                    }
                    errors.extend(a.validate());
                    activities.push(a);
                }
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        activities.sort_by(|a, b| a.id.cmp(&b.id));
        if errors.is_empty() {
            Ok(Self {
                activities,
                data_hash: h.finish(),
            })
        } else {
            Err(errors)
        }
    }

    pub fn get(&self, id: &str) -> Option<&Activity> {
        self.activities.iter().find(|a| a.id == id)
    }
}

/// Hashes text with CRLF normalised to LF, so a Windows checkout with
/// `core.autocrlf` gives the same hash (and recipes) as everywhere else.
fn write_text(h: &mut StableHasher, text: &str) {
    h.write_str(&text.replace("\r\n", "\n"));
}

/// The built-in catalog, parsed once.
pub fn builtin_catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(Catalog::builtin)
}

/// Hash of the built-in activity files without parsing them.
pub fn builtin_data_hash() -> u64 {
    let mut h = StableHasher::new();
    for (name, text) in SOURCES {
        h.write_str(name);
        write_text(&mut h, text);
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_is_valid() {
        let c = Catalog::builtin();
        assert_eq!(c.activities.len(), SOURCES.len());
        assert_eq!(c.data_hash, builtin_data_hash());
    }

    #[test]
    fn validation_reports_bad_files() {
        let bad = r#"
            id = "bad"
            [requires]
            terrain = ["lava"]
            time_of_day = []
            [motion]
            type = "pace"
            [props.anvil]
            attach = "on_head"
            [animation]
            start = "idle"
            [animation.states.idle]
            loop = true
            next = { nowhere = 1.0 }
            [gif]
            loop_script = ["sit"]
            [unattended]
            props = []
        "#;
        let errs = Catalog::from_sources(&[("bad.toml", bad)]).unwrap_err();
        let all = errs.join("\n");
        for needle in [
            "unknown terrain `lava`",
            "time_of_day is empty",
            "undefined state `nowhere`",
            "needs dwell_s",
            "unknown prop `anvil`",
            "unknown attach `on_head`",
            "unattended.props is empty",
            "pace motion needs speed",
            "must start with the start state",
        ] {
            assert!(all.contains(needle), "missing `{needle}` in:\n{all}");
        }
    }

    #[test]
    fn data_hash_ignores_line_endings() {
        let lf = Catalog::builtin();
        let crlf: Vec<(String, String)> = SOURCES
            .iter()
            .map(|(n, t)| (n.to_string(), t.replace('\n', "\r\n")))
            .collect();
        let refs: Vec<(&str, &str)> = crlf.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        assert_eq!(
            Catalog::from_sources(&refs).unwrap().data_hash,
            lf.data_hash
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let text = "id = \"x\"\ncolour = \"red\"\n";
        assert!(Catalog::from_sources(&[("x.toml", text)]).is_err());
    }
}
