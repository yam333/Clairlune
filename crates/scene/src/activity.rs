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
pub const PROPS: [&str; 25] = [
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
    "stone_pile",
    "easel",
    "snowman",
    "basket",
    "picnic_basket",
    "chess_table",
    "practice_wall",
    "skis_rest",
    "flower_pots",
    "music_stand",
    "camera_tripod",
    "sandcastle",
    "yoga_mat",
];
/// Effect ids the engine can draw.
pub const EFFECTS: [&str; 1] = ["embers"];
/// Particle presets an `[[effects]]` entry can attach to an anchor.
pub const PARTICLE_PRESETS: [&str; 6] = [
    "sparkles",
    "steam",
    "notes",
    "snow_spray",
    "bubbles",
    "drops",
];
/// Tether presets: a line from an anchor to a moving end (a float on the
/// water, a kite in the sky).
pub const TETHER_PRESETS: [&str; 2] = ["fishing_line", "kite_string"];
/// Projectile presets: something thrown on its own arc, timed from the start
/// of the effect's state.
pub const PROJECTILE_PRESETS: [&str; 3] = ["skipping_stone", "snowball", "tennis_ball"];
/// Which way the character faces, when it matters: `water` (the open water,
/// on the shore) or `open` (the middle of the screen, where the sky is
/// widest).
pub const FACINGS: [&str; 2] = ["water", "open"];
/// Anchor names the art exports (docs/clawd-style.md).
pub const ANCHORS: [&str; 6] = ["origin", "feet", "head", "hand_l", "hand_r", "item_tip"];
/// Generic prop attachment points.
pub const ATTACH: [&str; 4] = ["path", "left_of_clawd", "right_of_clawd", "either_side"];

/// Embedded activity files, sorted by file name.
const SOURCES: [(&str, &str); 38] = [
    (
        "balloon_journey.toml",
        include_str!("../activities/balloon_journey.toml"),
    ),
    (
        "balloon_ride.toml",
        include_str!("../activities/balloon_ride.toml"),
    ),
    (
        "birdwatching.toml",
        include_str!("../activities/birdwatching.toml"),
    ),
    (
        "broom_flight.toml",
        include_str!("../activities/broom_flight.toml"),
    ),
    (
        "broom_journey.toml",
        include_str!("../activities/broom_journey.toml"),
    ),
    ("bubbles.toml", include_str!("../activities/bubbles.toml")),
    (
        "catch_snowflakes.toml",
        include_str!("../activities/catch_snowflakes.toml"),
    ),
    ("chess.toml", include_str!("../activities/chess.toml")),
    ("dance.toml", include_str!("../activities/dance.toml")),
    (
        "desert_trek.toml",
        include_str!("../activities/desert_trek.toml"),
    ),
    ("fishing.toml", include_str!("../activities/fishing.toml")),
    ("fly_kite.toml", include_str!("../activities/fly_kite.toml")),
    (
        "forest_walk.toml",
        include_str!("../activities/forest_walk.toml"),
    ),
    (
        "gardening.toml",
        include_str!("../activities/gardening.toml"),
    ),
    (
        "hike_ridge.toml",
        include_str!("../activities/hike_ridge.toml"),
    ),
    (
        "hot_cocoa.toml",
        include_str!("../activities/hot_cocoa.toml"),
    ),
    (
        "make_coffee.toml",
        include_str!("../activities/make_coffee.toml"),
    ),
    ("meditate.toml", include_str!("../activities/meditate.toml")),
    (
        "paint_easel.toml",
        include_str!("../activities/paint_easel.toml"),
    ),
    (
        "photography.toml",
        include_str!("../activities/photography.toml"),
    ),
    ("picnic.toml", include_str!("../activities/picnic.toml")),
    (
        "play_flute.toml",
        include_str!("../activities/play_flute.toml"),
    ),
    (
        "play_guitar.toml",
        include_str!("../activities/play_guitar.toml"),
    ),
    (
        "read_lantern.toml",
        include_str!("../activities/read_lantern.toml"),
    ),
    ("rowing.toml", include_str!("../activities/rowing.toml")),
    (
        "sandcastle.toml",
        include_str!("../activities/sandcastle.toml"),
    ),
    ("ski_tour.toml", include_str!("../activities/ski_tour.toml")),
    ("skiing.toml", include_str!("../activities/skiing.toml")),
    (
        "skip_stones.toml",
        include_str!("../activities/skip_stones.toml"),
    ),
    ("sledding.toml", include_str!("../activities/sledding.toml")),
    (
        "snowballs.toml",
        include_str!("../activities/snowballs.toml"),
    ),
    ("stargaze.toml", include_str!("../activities/stargaze.toml")),
    ("stroll.toml", include_str!("../activities/stroll.toml")),
    ("sweep.toml", include_str!("../activities/sweep.toml")),
    ("tennis.toml", include_str!("../activities/tennis.toml")),
    (
        "warm_hands.toml",
        include_str!("../activities/warm_hands.toml"),
    ),
    (
        "wizard_magic.toml",
        include_str!("../activities/wizard_magic.toml"),
    ),
    ("yoga.toml", include_str!("../activities/yoga.toml")),
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub id: String,
    #[serde(default = "one")]
    pub weight: f64,
    pub requires: Requires,
    pub motion: Motion,
    /// Marks left in the ground along the character's way: `ski_tracks`.
    #[serde(default)]
    pub trail: Option<String>,
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
    /// `particles`, `tether` or `projectile`.
    #[serde(rename = "type")]
    pub kind: String,
    /// One of [`PARTICLE_PRESETS`], [`TETHER_PRESETS`] or
    /// [`PROJECTILE_PRESETS`], by type.
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
    /// The camera travels: the landscape scrolls past in layers while the
    /// character walks on the nearest (docs/scrolling-scenes.md).
    Journey,
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
    /// Journeys before 0.12: the loop length range in seconds. Unused now
    /// (the pace, `speed_cells_per_s`, sets the loop); still read.
    #[serde(default)]
    pub loop_s: Option<[f64; 2]>,
    /// Journeys: where the character walks, as a fraction of the width.
    #[serde(default)]
    pub screen_x: Option<f64>,
    /// Journeys: fly instead of walking, the feet this far down the screen
    /// (a fraction of the height), the land going by below.
    #[serde(default)]
    pub altitude: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PropDef {
    pub attach: String,
    #[serde(default)]
    pub effects: Vec<String>,
    /// Extra space before the prop, in sprite pixels (a practice wall
    /// stands back from the player).
    #[serde(default)]
    pub gap: u32,
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

/// The largest weight a file may give (picks multiply weights).
pub const MAX_WEIGHT: f64 = 1000.0;

/// A weight from a file that picks can use: at least 0.001 (it rounds to a
/// whole thousandth) and at most `MAX_WEIGHT`.
fn usable_weight(w: f64) -> bool {
    w <= MAX_WEIGHT && int_weight(w) > 0
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

    /// Whether the state machine can run: every state that goes on has a
    /// way to go. Looser than [`validate`](Self::validate), which also holds
    /// art and names to today's rules, for activities saved in older scenes.
    pub fn playable(&self) -> bool {
        self.animation.states.values().all(|s| {
            s.next.is_empty()
                || s.next
                    .values()
                    .map(|w| u64::from(int_weight(*w)))
                    .sum::<u64>()
                    > 0
        })
    }

    /// Every rule from PLAN.md section 4.7 that can be checked without sprites.
    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        let id = &self.id;
        let mut err = |m: String| errs.push(format!("{id}: {m}"));
        if !usable_weight(self.weight) {
            err(format!("weight must be between 0.001 and {MAX_WEIGHT}"));
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
                if !usable_weight(*w) {
                    err(format!(
                        "state `{name}`: weight for `{to}` must be between 0.001 and {MAX_WEIGHT}"
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
        if let Some(t) = &self.trail
            && t != "ski_tracks"
        {
            err(format!("unknown trail `{t}`"));
        }
        if self.motion.kind == MotionKind::Bob && self.requires.terrain != ["shore"] {
            err("bob motion floats on the shore's water: requires.terrain = [\"shore\"]".into());
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
                "projectile" if !PROJECTILE_PRESETS.contains(&e.preset.as_str()) => {
                    err(format!("unknown projectile preset `{}`", e.preset));
                }
                "particles" | "tether" | "projectile" => {}
                other => err(format!(
                    "effect type `{other}` is not implemented (`particles`, `tether`, `projectile`)"
                )),
            }
            if e.preset == "tennis_ball" && !self.props.contains_key("practice_wall") {
                err("a tennis ball needs a practice wall (props.practice_wall)".into());
            }
            if e.preset == "snowball" && !self.props.contains_key("snowman") {
                err("a snowball needs a snowman to throw at (props.snowman)".into());
            }
            if ["fishing_line", "skipping_stone"].contains(&e.preset.as_str())
                && self.requires.terrain != ["shore"]
            {
                err(format!(
                    "`{}` needs requires.terrain = [\"shore\"]",
                    e.preset
                ));
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
        let journey = self.motion.kind == MotionKind::Journey;
        if journey && !self.props.is_empty() {
            err("a journey has no props: the ground moves under them".into());
        }
        // A journey without the character is the landscape going by.
        if self.unattended.props.is_empty() && !journey {
            err("unattended.props is empty: Buddy-off scenes need a focal object".into());
        }
        for p in &self.unattended.props {
            if !PROPS.contains(&p.as_str()) && !EFFECTS.contains(&p.as_str()) {
                err(format!("unattended: unknown prop or effect `{p}`"));
            }
        }
        if self.motion.altitude.is_some() && self.motion.kind != MotionKind::Journey {
            err("altitude is for journeys".into());
        }
        match self.motion.kind {
            MotionKind::Stationary => {}
            MotionKind::Pace | MotionKind::Fly | MotionKind::Bob => {
                match self.motion.speed_cells_per_s {
                    Some([lo, hi]) if lo > 0.0 && hi >= lo => {}
                    _ => err(format!(
                        "{} motion needs speed_cells_per_s = [lo, hi] with 0 < lo <= hi",
                        format!("{:?}", self.motion.kind).to_lowercase()
                    )),
                }
            }
            MotionKind::Journey => {
                match self.motion.speed_cells_per_s {
                    Some([lo, hi]) if lo >= 1.0 && hi >= lo && hi <= 20.0 => {}
                    _ => err(
                        "journey motion needs speed_cells_per_s = [lo, hi] within 1..=20 (the ground's pace)"
                            .into(),
                    ),
                }
                if let Some(x) = self.motion.screen_x
                    && !(0.15..=0.85).contains(&x)
                {
                    err("journey screen_x must be within 0.15..=0.85".into());
                }
                if let Some(y) = self.motion.altitude
                    && !(0.2..=0.6).contains(&y)
                {
                    err("journey altitude must be within 0.2..=0.6".into());
                }
            }
            MotionKind::Path => err(format!(
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
            next = { nowhere = 1.0, idle = 0.0001 }
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
            "weight for `idle` must be between 0.001",
        ] {
            assert!(all.contains(needle), "missing `{needle}` in:\n{all}");
        }
    }

    #[test]
    fn sources_are_sorted_by_file_name_like_a_folder_listing() {
        // The data hash follows this order, and tools that read the folder
        // (the preview window) sort by name, so both must agree.
        let names: Vec<&str> = SOURCES.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
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
