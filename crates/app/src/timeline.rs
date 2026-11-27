//! The daily timeline: a schedule of wallpapers by time
//! of day, one day for every day or one per weekday, in
//! `<config_dir>/clairlune/timeline.toml`.
//!
//! The active slot is resolved by UTC instants, not wall-clock comparisons, so
//! daylight saving can't make a slot fire twice or never:
//! - a repeated local time (fall back) uses its first occurrence;
//! - a skipped local time (spring forward) uses the first valid instant after;
//! - slots that land on the same instant: the latest-defined wins;
//! - the active slot is the one with the latest instant at or before now.
//!
//! The resolver takes the time zone and `now` (jiff), so tests run it in
//! real zones from the time zone database.

use crate::library::{BuddyChoice, DisplayChoice, ThemeChoice};
use jiff::civil::{Date, Weekday};
use jiff::tz::{AmbiguousOffset, TimeZone};
use jiff::{SignedDuration, Timestamp, ToSpan};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Single,
    Weekly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    #[default]
    Shuffle,
    Sequential,
}

/// What a slot shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// One saved wallpaper.
    Item { id: String },
    /// Saved wallpapers picked from by ids or a tag.
    Pool {
        #[serde(default)]
        ids: Vec<String>,
        #[serde(default)]
        tag: Option<String>,
        #[serde(default)]
        order: Order,
    },
    /// A fresh scene, optionally narrowed.
    Random {
        #[serde(default)]
        archetypes: Vec<String>,
        #[serde(default)]
        activities: Vec<String>,
        #[serde(default)]
        weather: Vec<String>,
    },
}

/// One slot: from `start` until the next slot starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    /// Local time of day, `HH:MM`.
    #[serde(with = "hhmm")]
    pub start: u16,
    pub content: Content,
    #[serde(default)]
    pub display: DisplayChoice,
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default)]
    pub buddy: BuddyChoice,
}

/// Minutes after midnight as `HH:MM` text.
mod hhmm {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn parse(s: &str) -> Option<u16> {
        let (h, m) = s.trim().split_once(':')?;
        let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
        (h < 24 && m < 60).then_some(h * 60 + m)
    }

    pub fn format(m: u16) -> String {
        format!("{:02}:{:02}", m / 60, m % 60)
    }

    pub fn serialize<S: Serializer>(m: &u16, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format(*m))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
        let s = String::deserialize(d)?;
        parse(&s).ok_or_else(|| serde::de::Error::custom(format!("`{s}` is not a time (HH:MM)")))
    }
}

pub use hhmm::format as format_time;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct DayFile {
    #[serde(default)]
    slot: Vec<Slot>,
}

/// `timeline.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct TimelineFile {
    #[serde(default)]
    layout: Layout,
    #[serde(default)]
    single: DayFile,
    /// Keyed by `monday` .. `sunday`.
    #[serde(default)]
    day: BTreeMap<String, DayFile>,
}

const DAY_NAMES: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timeline {
    pub layout: Layout,
    /// The single day (single layout).
    pub single: Vec<Slot>,
    /// Monday first (weekly layout).
    pub week: [Vec<Slot>; 7],
}

impl Timeline {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let file: TimelineFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut week: [Vec<Slot>; 7] = Default::default();
        for (name, day) in file.day {
            let i = DAY_NAMES
                .iter()
                .position(|d| *d == name)
                .ok_or_else(|| format!("unknown day `{name}`"))?;
            week[i] = day.slot;
        }
        let mut t = Timeline {
            layout: file.layout,
            single: file.single.slot,
            week,
        };
        t.normalize();
        Ok(t)
    }

    pub fn to_toml(&self) -> String {
        let mut day = BTreeMap::new();
        if self.layout == Layout::Weekly {
            for (i, slots) in self.week.iter().enumerate() {
                day.insert(
                    DAY_NAMES[i].to_string(),
                    DayFile {
                        slot: slots.clone(),
                    },
                );
            }
        }
        let file = TimelineFile {
            layout: self.layout,
            single: DayFile {
                slot: if self.layout == Layout::Single {
                    self.single.clone()
                } else {
                    Vec::new()
                },
            },
            day,
        };
        toml::to_string_pretty(&file).unwrap_or_default()
    }

    /// Loads `timeline.toml`; a missing file is an empty timeline.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(t) => Self::from_toml(&t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.part");
        std::fs::write(
            &tmp,
            format!(
                "# Clairlune timeline. Edit here or in the app.\n\n{}",
                self.to_toml()
            ),
        )?;
        std::fs::rename(tmp, path)
    }

    /// Sorts every day's slots by start and keeps the last of duplicate
    /// starts (the latest-defined wins).
    pub fn normalize(&mut self) {
        let fix = |slots: &mut Vec<Slot>| {
            // A stable sort keeps definition order among equal starts.
            slots.sort_by_key(|s| s.start);
            let mut out: Vec<Slot> = Vec::with_capacity(slots.len());
            for s in slots.drain(..) {
                if out.last().is_some_and(|l| l.start == s.start) {
                    out.pop();
                }
                out.push(s);
            }
            *slots = out;
        };
        fix(&mut self.single);
        for d in &mut self.week {
            fix(d);
        }
    }

    /// The slots used on a weekday.
    pub fn day(&self, weekday: Weekday) -> &[Slot] {
        match self.layout {
            Layout::Single => &self.single,
            Layout::Weekly => &self.week[weekday_index(weekday)],
        }
    }

    pub fn is_empty(&self) -> bool {
        match self.layout {
            Layout::Single => self.single.is_empty(),
            Layout::Weekly => self.week.iter().all(Vec::is_empty),
        }
    }

    /// Problems worth showing (unknown items or ids); the timeline still runs,
    /// skipping what it can't show.
    pub fn problems(&self, item_exists: &dyn Fn(&str) -> bool) -> Vec<String> {
        let mut out = Vec::new();
        let days: Vec<(&str, &[Slot])> = match self.layout {
            Layout::Single => vec![("Every day", &self.single)],
            Layout::Weekly => (0..7)
                .map(day_name)
                .zip(&self.week)
                .map(|(n, s)| (n, s.as_slice()))
                .collect(),
        };
        for (day, slots) in days {
            for s in slots {
                let at = format!("{day} {}", format_time(s.start));
                match &s.content {
                    Content::Item { id } if !item_exists(id) => {
                        out.push(format!("{at}: its saved wallpaper is gone ({id})"));
                    }
                    Content::Pool { ids, .. } => {
                        for id in ids.iter().filter(|id| !item_exists(id)) {
                            out.push(format!("{at}: a pool wallpaper is gone ({id})"));
                        }
                    }
                    Content::Random {
                        archetypes,
                        activities,
                        weather,
                    } => {
                        for a in archetypes {
                            if clairlune_scene::archetype::archetype(a)
                                .is_none_or(|info| !info.implemented)
                            {
                                out.push(format!("{at}: unknown landscape \u{201c}{a}\u{201d}"));
                            }
                        }
                        for a in activities {
                            if clairlune_scene::activity::builtin_catalog()
                                .get(a)
                                .is_none()
                            {
                                out.push(format!("{at}: unknown activity \u{201c}{a}\u{201d}"));
                            }
                        }
                        for w in weather {
                            if !clairlune_scene::archetype::WEATHERS.contains(&w.as_str()) {
                                out.push(format!("{at}: unknown weather \u{201c}{w}\u{201d}"));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        out
    }
}

/// What the timeline says should be on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    /// A slot of the day it started on.
    Slot { weekday: Weekday, index: usize },
    /// An empty day: the Generate schedule runs.
    Generate,
}

/// The resolved timeline at an instant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Active {
    pub pick: Pick,
    /// When it started.
    pub since: Timestamp,
    /// When the next slot (or empty day) starts.
    pub next: Timestamp,
}

/// Monday is 0.
pub fn weekday_index(weekday: Weekday) -> usize {
    weekday.to_monday_zero_offset() as usize
}

/// An instant as local `HH:MM` in the system's time zone.
pub fn local_hhmm(t: Timestamp) -> String {
    t.to_zoned_datetime(TimeZone::system())
        .strftime("%H:%M")
        .to_string()
}

/// The UTC instant of a local wall-clock time: the first occurrence when it
/// repeats, the first valid instant after it when it is skipped.
fn instant_of(tz: &TimeZone, date: Date, minute: u16) -> Option<Timestamp> {
    let start = date.at((minute / 60) as i8, (minute % 60) as i8, 0, 0);
    // In a gap, step forward a minute at a time (gaps are at most a few
    // hours) to the first local time that exists.
    (0..=6 * 60i64).find_map(|k| {
        let local = start.checked_add(k.minutes()).ok()?;
        let at = tz.to_ambiguous_timestamp(local);
        match at.offset() {
            AmbiguousOffset::Gap { .. } => None,
            // A repeated time: its first occurrence.
            AmbiguousOffset::Unambiguous { .. } | AmbiguousOffset::Fold { .. } => at.earlier().ok(),
        }
    })
}

/// Resolves the timeline at `now` in `tz`. `None` for an empty timeline.
pub fn resolve(tl: &Timeline, tz: &TimeZone, now: Timestamp) -> Option<Active> {
    if tl.is_empty() {
        return None;
    }
    let today = now.to_zoned_datetime(tz.clone()).date();
    // Events from yesterday to the day after tomorrow: (instant, order, pick).
    // `order` keeps definition order so that, at one instant, the
    // latest-defined slot wins.
    let mut events: Vec<(Timestamp, usize, Pick)> = Vec::new();
    let mut order = 0usize;
    for offset in -1..=2i64 {
        let date = today.checked_add(offset.days()).ok()?;
        let weekday = date.weekday();
        let slots = tl.day(weekday);
        if slots.is_empty() {
            if let Some(t) = instant_of(tz, date, 0) {
                events.push((t, order, Pick::Generate));
                order += 1;
            }
            continue;
        }
        for (index, s) in slots.iter().enumerate() {
            if let Some(t) = instant_of(tz, date, s.start) {
                events.push((t, order, Pick::Slot { weekday, index }));
                order += 1;
            }
        }
    }
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    // Collapse events at the same instant to the latest-defined.
    let mut collapsed: Vec<(Timestamp, Pick)> = Vec::new();
    for (t, _, pick) in events {
        if collapsed.last().is_some_and(|(lt, _)| *lt == t) {
            collapsed.pop();
        }
        collapsed.push((t, pick));
    }
    let i = collapsed.iter().rposition(|(t, _)| *t <= now)?;
    let next = collapsed.get(i + 1).map(|(t, _)| *t).unwrap_or_else(|| {
        now.checked_add(SignedDuration::from_hours(24))
            .unwrap_or(now)
    });
    Some(Active {
        pick: collapsed[i].1.clone(),
        since: collapsed[i].0,
        next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_york() -> TimeZone {
        TimeZone::get("America/New_York").expect("the time zone database has New York")
    }

    fn slot(start: &str, id: &str) -> Slot {
        Slot {
            start: hhmm::parse(start).unwrap(),
            content: Content::Item { id: id.into() },
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
        }
    }

    fn single(slots: Vec<Slot>) -> Timeline {
        let mut t = Timeline {
            layout: Layout::Single,
            single: slots,
            ..Default::default()
        };
        t.normalize();
        t
    }

    fn utc(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    fn picked<'a>(tl: &'a Timeline, a: &Active) -> &'a str {
        match &a.pick {
            Pick::Slot { weekday, index } => match &tl.day(*weekday)[*index].content {
                Content::Item { id } => id,
                _ => "other",
            },
            Pick::Generate => "generate",
        }
    }

    #[test]
    fn toml_round_trip_in_the_plan_format() {
        let text = r#"
layout = "weekly"

[[day.monday.slot]]
start = "07:00"
content = { type = "item", id = "lib_8f2c" }
display = "follow"
theme = "light"
buddy = "follow"

[[day.monday.slot]]
start = "19:30"
content = { type = "pool", tag = "evening", order = "shuffle" }
display = "live"
theme = "dark"

[[day.monday.slot]]
start = "23:00"
content = { type = "random", archetypes = ["night_sky", "canyon"] }
"#;
        let t = Timeline::from_toml(text).unwrap();
        assert_eq!(t.layout, Layout::Weekly);
        assert_eq!(t.week[0].len(), 3);
        assert_eq!(t.week[0][1].theme, ThemeChoice::Dark);
        assert!(
            matches!(&t.week[0][1].content, Content::Pool { tag: Some(tag), .. } if tag == "evening")
        );
        let back = Timeline::from_toml(&t.to_toml()).unwrap();
        assert_eq!(back, t);
        assert!(
            Timeline::from_toml(
                "[[single.slot]]\nstart = \"25:00\"\ncontent = { type = \"random\" }"
            )
            .is_err()
        );
    }

    #[test]
    fn the_active_slot_wraps_around_midnight() {
        let tl = single(vec![slot("07:00", "morning"), slot("23:00", "night")]);
        // 00:30 local (EST, UTC-5) is still last night's slot.
        let a = resolve(&tl, &new_york(), utc("2026-01-15T05:30:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "night");
        assert_eq!(a.since, utc("2026-01-15T04:00:00Z"));
        assert_eq!(a.next, utc("2026-01-15T12:00:00Z"));
        let a = resolve(&tl, &new_york(), utc("2026-01-15T12:00:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "morning");
    }

    #[test]
    fn spring_forward_runs_a_skipped_slot_at_the_first_valid_instant() {
        // 2026-03-08 in New York: 02:00 EST jumps to 03:00 EDT.
        let tl = single(vec![
            slot("01:00", "early"),
            slot("02:30", "gap"),
            slot("06:00", "dawn"),
        ]);
        let a = resolve(&tl, &new_york(), utc("2026-03-08T07:00:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "gap");
        // It starts at 03:00 EDT = 07:00 UTC, the first valid instant after 02:30.
        assert_eq!(a.since, utc("2026-03-08T07:00:00Z"));
        let before = resolve(&tl, &new_york(), utc("2026-03-08T06:59:00Z")).unwrap();
        assert_eq!(picked(&tl, &before), "early");
    }

    #[test]
    fn slots_in_a_gap_at_one_instant_go_to_the_latest_defined() {
        let tl = single(vec![slot("02:15", "first"), slot("02:45", "second")]);
        let a = resolve(&tl, &new_york(), utc("2026-03-08T07:30:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "second");
    }

    #[test]
    fn fall_back_fires_a_repeated_slot_once() {
        // 2026-11-01 in New York: 01:00-02:00 happens twice (EDT, then EST).
        let tl = single(vec![
            slot("00:00", "midnight"),
            slot("01:30", "repeat"),
            slot("03:00", "late"),
        ]);
        // The first 01:30 (EDT, 05:30 UTC) starts the slot...
        let a = resolve(&tl, &new_york(), utc("2026-11-01T05:45:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "repeat");
        assert_eq!(a.since, utc("2026-11-01T05:30:00Z"));
        // ...and during the second 01:30 (EST, 06:30 UTC) it is still the
        // same slot from the same start: it did not fire again.
        let b = resolve(&tl, &new_york(), utc("2026-11-01T06:45:00Z")).unwrap();
        assert_eq!(picked(&tl, &b), "repeat");
        assert_eq!(b.since, a.since);
        assert_eq!(b.next, utc("2026-11-01T08:00:00Z"));
    }

    #[test]
    fn weekly_days_and_empty_days() {
        let mut week: [Vec<Slot>; 7] = Default::default();
        week[0] = vec![slot("08:00", "monday")]; // Monday
        // Tuesday empty: the Generate schedule runs from Tuesday midnight.
        let tl = Timeline {
            layout: Layout::Weekly,
            single: Vec::new(),
            week,
        };
        // 2026-01-12 is a Monday.
        let mon = resolve(&tl, &new_york(), utc("2026-01-12T15:00:00Z")).unwrap();
        assert_eq!(picked(&tl, &mon), "monday");
        let tue = resolve(&tl, &new_york(), utc("2026-01-13T15:00:00Z")).unwrap();
        assert_eq!(tue.pick, Pick::Generate);
        assert_eq!(tue.since, utc("2026-01-13T05:00:00Z"));
        assert!(
            resolve(
                &Timeline::default(),
                &new_york(),
                utc("2026-01-13T15:00:00Z")
            )
            .is_none()
        );
    }

    #[test]
    fn duplicates_keep_the_latest_defined() {
        let tl = single(vec![
            slot("08:00", "a"),
            slot("08:00", "b"),
            slot("06:00", "c"),
        ]);
        let ids: Vec<String> = tl
            .single
            .iter()
            .map(|s| match &s.content {
                Content::Item { id } => id.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(ids, ["c", "b"]);
    }

    #[test]
    fn problems_name_missing_items_and_unknown_ids() {
        let mut tl = single(vec![slot("08:00", "lib_gone")]);
        tl.single.push(Slot {
            start: 600,
            content: Content::Random {
                archetypes: vec!["atlantis".into()],
                activities: vec!["juggling".into()],
                weather: vec!["tornado".into()],
            },
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
        });
        let p = tl.problems(&|id| id == "lib_here");
        assert_eq!(p.len(), 4, "{p:?}");
        assert!(p[0].contains("lib_gone"));
        assert!(p[3].contains("tornado"));
    }
}

/// Picks from a pool. Shuffle: seeded by the slot's start instant, never the
/// item shown last time unless it is the only one. Sequential: the next one
/// each time the slot starts (`position` is kept between runs).
pub fn pick_from_pool(
    candidates: &[String],
    order: Order,
    seed: u64,
    last: Option<&str>,
    position: &mut usize,
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let n = candidates.len();
    let i = match order {
        Order::Shuffle => {
            let mut i = (clairlune_scene::hash::hash_label(seed, "pool") % n as u64) as usize;
            if n > 1 && Some(candidates[i].as_str()) == last {
                i = (i + 1) % n;
            }
            i
        }
        Order::Sequential => {
            let i = *position % n;
            *position = position.wrapping_add(1);
            i
        }
    };
    Some(candidates[i].clone())
}

#[cfg(test)]
mod pool_tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("lib_{i}")).collect()
    }

    #[test]
    fn shuffle_is_seeded_and_never_repeats_the_last() {
        let c = ids(4);
        let mut pos = 0;
        let a = pick_from_pool(&c, Order::Shuffle, 42, None, &mut pos).unwrap();
        assert_eq!(
            pick_from_pool(&c, Order::Shuffle, 42, None, &mut pos).unwrap(),
            a
        );
        for seed in 0..50 {
            let first = pick_from_pool(&c, Order::Shuffle, seed, None, &mut pos).unwrap();
            let again = pick_from_pool(&c, Order::Shuffle, seed, Some(&first), &mut pos).unwrap();
            assert_ne!(first, again);
        }
        let one = ids(1);
        assert_eq!(
            pick_from_pool(&one, Order::Shuffle, 1, Some("lib_0"), &mut pos).as_deref(),
            Some("lib_0")
        );
    }

    #[test]
    fn sequential_walks_the_pool() {
        let c = ids(3);
        let mut pos = 0;
        let got: Vec<String> = (0..5)
            .map(|_| pick_from_pool(&c, Order::Sequential, 0, None, &mut pos).unwrap())
            .collect();
        assert_eq!(got, ["lib_0", "lib_1", "lib_2", "lib_0", "lib_1"]);
        assert!(pick_from_pool(&[], Order::Sequential, 0, None, &mut pos).is_none());
    }
}

/// What to do with slots that show a library item being deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forget {
    /// Show a random scene there instead.
    UseRandom,
    /// Remove those slots (the previous slot extends over them).
    RemoveSlots,
}

impl Timeline {
    fn days_mut(&mut self) -> Vec<&mut Vec<Slot>> {
        match self.layout {
            Layout::Single => vec![&mut self.single],
            Layout::Weekly => self.week.iter_mut().collect(),
        }
    }

    /// Where a library item is used, as "Monday 07:00" (or "Every day 07:00")
    /// labels, slots that show it directly and pools that list it.
    pub fn uses(&self, id: &str) -> Vec<String> {
        let label = |day: usize, s: &Slot| match self.layout {
            Layout::Single => format!("Every day {}", format_time(s.start)),
            Layout::Weekly => format!("{} {}", day_name(day), format_time(s.start)),
        };
        let days: Vec<&Vec<Slot>> = match self.layout {
            Layout::Single => vec![&self.single],
            Layout::Weekly => self.week.iter().collect(),
        };
        let mut out = Vec::new();
        for (i, slots) in days.into_iter().enumerate() {
            for s in slots {
                let used = match &s.content {
                    Content::Item { id: x } => x == id,
                    Content::Pool { ids, .. } => ids.iter().any(|x| x == id),
                    Content::Random { .. } => false,
                };
                if used {
                    out.push(label(i, s));
                }
            }
        }
        out
    }

    /// Removes every reference to a library item: slots showing it become
    /// random or go away; pools drop it from their lists.
    pub fn forget_item(&mut self, id: &str, how: Forget) {
        for slots in self.days_mut() {
            for s in slots.iter_mut() {
                match &mut s.content {
                    Content::Item { id: x } if x == id && how == Forget::UseRandom => {
                        s.content = Content::Random {
                            archetypes: Vec::new(),
                            activities: Vec::new(),
                            weather: Vec::new(),
                        };
                    }
                    Content::Pool { ids, .. } => ids.retain(|x| x != id),
                    _ => {}
                }
            }
            if how == Forget::RemoveSlots {
                slots.retain(|s| !matches!(&s.content, Content::Item { id: x } if x == id));
            }
        }
    }
}

#[cfg(test)]
mod forget_tests {
    use super::*;

    fn tl() -> Timeline {
        let item = |start: u16, id: &str| Slot {
            start,
            content: Content::Item { id: id.into() },
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
        };
        let mut week: [Vec<Slot>; 7] = Default::default();
        week[0] = vec![item(420, "lib_a"), item(600, "lib_b")];
        week[2] = vec![Slot {
            start: 1200,
            content: Content::Pool {
                ids: vec!["lib_a".into(), "lib_c".into()],
                tag: None,
                order: Order::Shuffle,
            },
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
        }];
        Timeline {
            layout: Layout::Weekly,
            single: Vec::new(),
            week,
        }
    }

    #[test]
    fn uses_lists_slots_and_pools() {
        assert_eq!(tl().uses("lib_a"), ["Monday 07:00", "Wednesday 20:00"]);
        assert!(tl().uses("lib_z").is_empty());
    }

    #[test]
    fn forgetting_leaves_no_reference() {
        let mut random = tl();
        random.forget_item("lib_a", Forget::UseRandom);
        assert!(random.uses("lib_a").is_empty());
        assert!(matches!(random.week[0][0].content, Content::Random { .. }));
        let mut removed = tl();
        removed.forget_item("lib_a", Forget::RemoveSlots);
        assert!(removed.uses("lib_a").is_empty());
        assert_eq!(removed.week[0].len(), 1);
        assert!(
            matches!(&removed.week[2][0].content, Content::Pool { ids, .. } if ids == &["lib_c".to_string()])
        );
    }
}

/// Minutes in a day; a slot's end when it is the day's last.
pub const DAY_MINUTES: u16 = 24 * 60;

pub use hhmm::parse as parse_time;

/// Snaps a time of day to 15 minutes (or to the minute with `fine`), kept
/// inside the day.
pub fn snap(minute: f32, fine: bool) -> u16 {
    let step = if fine { 1.0 } else { 15.0 };
    let m = (minute / step).round() * step;
    let last = f32::from(DAY_MINUTES) - step;
    m.clamp(0.0, last) as u16
}

/// What the start of a day shows until its first slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carried {
    /// A slot of the day before (or of the same day, in single layout)
    /// carries on past midnight: `(day, index)` as the editor counts days.
    Slot(usize, usize),
    /// The day before is empty, so the Generate schedule runs.
    Generate,
}

/// Editing, as the timeline editor does it. Days are counted as the editor
/// shows them: `0` is the single day, or Monday in the weekly layout.
impl Timeline {
    /// How many day rows the editor shows.
    pub fn day_count(&self) -> usize {
        match self.layout {
            Layout::Single => 1,
            Layout::Weekly => 7,
        }
    }

    pub fn slots(&self, day: usize) -> &[Slot] {
        match self.layout {
            Layout::Single => &self.single,
            Layout::Weekly => &self.week[day % 7],
        }
    }

    pub fn slots_mut(&mut self, day: usize) -> &mut Vec<Slot> {
        match self.layout {
            Layout::Single => &mut self.single,
            Layout::Weekly => &mut self.week[day % 7],
        }
    }

    /// What runs from a day's midnight until its first slot.
    pub fn carried_into(&self, day: usize) -> Carried {
        let before = match self.layout {
            Layout::Single => 0,
            Layout::Weekly => (day + 6) % 7,
        };
        match self.slots(before).len() {
            0 => Carried::Generate,
            n => Carried::Slot(before, n - 1),
        }
    }

    /// When a slot ends: the next slot's start, or midnight for the last.
    pub fn end_of(&self, day: usize, index: usize) -> u16 {
        self.slots(day)
            .get(index + 1)
            .map_or(DAY_MINUTES, |s| s.start)
    }

    /// The slot of `day` that covers `minute`; `None` before the first slot
    /// (see [`Timeline::carried_into`]) and on an empty day.
    pub fn slot_at(&self, day: usize, minute: u16) -> Option<usize> {
        self.slots(day).iter().rposition(|s| s.start <= minute)
    }

    /// Adds a slot, replacing one that starts at the same minute. Returns
    /// its index.
    pub fn insert(&mut self, day: usize, slot: Slot) -> usize {
        let slots = self.slots_mut(day);
        match slots.binary_search_by_key(&slot.start, |s| s.start) {
            Ok(i) => {
                slots[i] = slot;
                i
            }
            Err(i) => {
                slots.insert(i, slot);
                i
            }
        }
    }

    /// Splits the slot (or carried-over time) that covers `minute`: the new
    /// slot starts there and shows the same thing. `None` when a slot already
    /// starts at that minute.
    pub fn split(&mut self, day: usize, minute: u16) -> Option<usize> {
        if self.slots(day).iter().any(|s| s.start == minute) {
            return None;
        }
        // An empty day runs the Generate schedule: its first slot is random.
        if self.slots(day).is_empty() {
            return Some(self.insert(day, Slot::random(minute)));
        }
        let source = match self.slot_at(day, minute) {
            Some(i) => Some(self.slots(day)[i].clone()),
            None => match self.carried_into(day) {
                Carried::Slot(d, i) => Some(self.slots(d)[i].clone()),
                Carried::Generate => None,
            },
        };
        let slot = match source {
            Some(s) => Slot { start: minute, ..s },
            None => Slot::random(minute),
        };
        Some(self.insert(day, slot))
    }

    /// Moves a slot's start, kept between its neighbours so the order never
    /// changes. Returns the start it got.
    pub fn move_start(&mut self, day: usize, index: usize, start: u16) -> u16 {
        let slots = self.slots_mut(day);
        let lo = if index == 0 {
            0
        } else {
            slots[index - 1].start + 1
        };
        let hi = slots
            .get(index + 1)
            .map_or(DAY_MINUTES - 1, |s| s.start - 1);
        let start = start.clamp(lo, hi.max(lo));
        slots[index].start = start;
        start
    }

    /// Sets a slot's start to a typed time; the slot moves among the others
    /// as needed. `None` (and no change) when another slot starts then.
    pub fn set_start(&mut self, day: usize, index: usize, start: u16) -> Option<usize> {
        let slots = self.slots_mut(day);
        if index >= slots.len() || start >= DAY_MINUTES {
            return None;
        }
        if slots
            .iter()
            .enumerate()
            .any(|(i, s)| i != index && s.start == start)
        {
            return None;
        }
        let mut slot = slots.remove(index);
        slot.start = start;
        Some(self.insert(day, slot))
    }

    /// Adds a slot in the middle of the day's longest stretch (the button
    /// for what double-clicking a day does). An empty day gets a random slot
    /// from midnight.
    pub fn add_slot(&mut self, day: usize) -> Option<usize> {
        if self.slots(day).is_empty() {
            return Some(self.insert(day, Slot::random(0)));
        }
        let mut bounds = vec![0];
        bounds.extend(self.slots(day).iter().map(|s| s.start));
        bounds.push(DAY_MINUTES);
        bounds.dedup();
        let (a, b) = bounds
            .windows(2)
            .map(|w| (w[0], w[1]))
            .max_by_key(|(a, b)| b - a)?;
        let mid = f32::from(a + b) / 2.0;
        self.split(day, snap(mid, false))
            .or_else(|| self.split(day, snap(mid, true)))
    }

    /// Removes a slot; the one before it now runs until the next.
    pub fn remove(&mut self, day: usize, index: usize) {
        let slots = self.slots_mut(day);
        if index < slots.len() {
            slots.remove(index);
        }
    }

    pub fn clear_day(&mut self, day: usize) {
        self.slots_mut(day).clear();
    }

    /// Copies a day's slots over other days (weekly layout).
    pub fn copy_day(&mut self, from: usize, to: &[usize]) {
        if self.layout != Layout::Weekly {
            return;
        }
        let slots = self.week[from % 7].clone();
        for &d in to.iter().filter(|&&d| d % 7 != from % 7) {
            self.week[d % 7] = slots.clone();
        }
    }

    /// Single to weekly: every day starts as a copy of the single day.
    pub fn make_weekly(&mut self) {
        if self.layout == Layout::Weekly {
            return;
        }
        let single = std::mem::take(&mut self.single);
        self.week = std::array::from_fn(|_| single.clone());
        self.layout = Layout::Weekly;
    }

    /// Weekly to single: `keep` becomes the one day; the others are dropped.
    pub fn make_single(&mut self, keep: usize) {
        if self.layout == Layout::Single {
            return;
        }
        self.single = std::mem::take(&mut self.week[keep % 7]);
        self.week = Default::default();
        self.layout = Layout::Single;
    }
}

impl Slot {
    /// Shows the same thing as another slot, whatever their starts.
    pub fn shows_same(&self, other: &Slot) -> bool {
        self.content == other.content
            && self.display == other.display
            && self.theme == other.theme
            && self.buddy == other.buddy
    }

    /// A random scene from `start`, with every choice following the settings.
    pub fn random(start: u16) -> Slot {
        Slot {
            start,
            content: Content::Random {
                archetypes: Vec::new(),
                activities: Vec::new(),
                weather: Vec::new(),
            },
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
        }
    }
}

/// English day names, Monday first.
pub fn day_name(day: usize) -> &'static str {
    [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ][day % 7]
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    fn item(start: u16, id: &str) -> Slot {
        Slot {
            start,
            content: Content::Item { id: id.into() },
            ..Slot::random(start)
        }
    }

    fn weekly() -> Timeline {
        let mut week: [Vec<Slot>; 7] = Default::default();
        week[0] = vec![item(420, "a"), item(1170, "b")];
        Timeline {
            layout: Layout::Weekly,
            single: Vec::new(),
            week,
        }
    }

    fn ids(slots: &[Slot]) -> Vec<(u16, String)> {
        slots
            .iter()
            .map(|s| match &s.content {
                Content::Item { id } => (s.start, id.clone()),
                Content::Pool { .. } => (s.start, "pool".into()),
                Content::Random { .. } => (s.start, "random".into()),
            })
            .collect()
    }

    fn pairs(v: &[(u16, &str)]) -> Vec<(u16, String)> {
        v.iter().map(|(m, s)| (*m, s.to_string())).collect()
    }

    #[test]
    fn snapping() {
        assert_eq!(snap(422.0, false), 420);
        assert_eq!(snap(428.0, false), 435);
        assert_eq!(snap(428.0, true), 428);
        assert_eq!(snap(-30.0, false), 0);
        assert_eq!(snap(1439.0, false), 1425);
        assert_eq!(snap(1500.0, true), 1439);
    }

    #[test]
    fn coverage_and_carry_over() {
        let t = weekly();
        assert_eq!(t.slot_at(0, 300), None);
        assert_eq!(t.slot_at(0, 420), Some(0));
        assert_eq!(t.slot_at(0, 1439), Some(1));
        assert_eq!(t.end_of(0, 0), 1170);
        assert_eq!(t.end_of(0, 1), DAY_MINUTES);
        // Tuesday starts with Monday's last slot; Monday with Sunday's
        // (empty, so the Generate schedule).
        assert_eq!(t.carried_into(1), Carried::Slot(0, 1));
        assert_eq!(t.carried_into(0), Carried::Generate);
        let s = Timeline {
            single: vec![item(60, "x"), item(600, "y")],
            ..Timeline::default()
        };
        assert_eq!(s.carried_into(0), Carried::Slot(0, 1));
    }

    #[test]
    fn splitting_inserting_and_removing() {
        let mut t = weekly();
        // Inside "a": the new slot shows "a" too.
        assert_eq!(t.split(0, 600), Some(1));
        assert_eq!(
            ids(&t.week[0]),
            pairs(&[(420, "a"), (600, "a"), (1170, "b")])
        );
        assert_eq!(t.split(0, 600), None);
        // Before the first slot: what carries over (Sunday is empty: random).
        assert_eq!(t.split(0, 60), Some(0));
        assert_eq!(ids(&t.week[0])[0], (60, "random".into()));
        // Tuesday, empty (the Generate schedule): a random slot.
        assert_eq!(t.split(1, 480), Some(0));
        assert_eq!(ids(&t.week[1]), pairs(&[(480, "random")]));
        // Inside Tuesday's carried-over part: what carries over (Monday's "b").
        assert_eq!(t.split(1, 120), Some(0));
        assert_eq!(ids(&t.week[1]), pairs(&[(120, "b"), (480, "random")]));
        t.remove(1, 0);
        // Insert replaces a slot at the same start.
        assert_eq!(t.insert(1, item(480, "c")), 0);
        assert_eq!(t.insert(1, item(300, "d")), 0);
        assert_eq!(ids(&t.week[1]), pairs(&[(300, "d"), (480, "c")]));
        t.remove(1, 0);
        assert_eq!(ids(&t.week[1]), pairs(&[(480, "c")]));
        t.remove(1, 9);
        t.clear_day(1);
        assert!(t.week[1].is_empty());
    }

    #[test]
    fn moving_a_start_stays_between_neighbours() {
        let mut t = weekly();
        t.split(0, 600);
        assert_eq!(t.move_start(0, 1, 300), 421);
        assert_eq!(t.move_start(0, 1, 2000), 1169);
        assert_eq!(t.move_start(0, 1, 700), 700);
        assert_eq!(t.move_start(0, 0, 0), 0);
        assert_eq!(t.move_start(0, 2, 2000), DAY_MINUTES - 1);
        let starts: Vec<u16> = t.week[0].iter().map(|s| s.start).collect();
        assert!(starts.windows(2).all(|w| w[0] < w[1]), "{starts:?}");
    }

    #[test]
    fn adding_fills_the_longest_stretch() {
        let mut t = weekly();
        // Monday: 00:00-07:00, 07:00-19:30, 19:30-24:00; the middle is longest.
        assert_eq!(t.add_slot(0), Some(1));
        assert_eq!(ids(&t.week[0])[1], (795, "a".into()));
        assert_eq!(t.add_slot(3), Some(0));
        assert_eq!(ids(&t.week[3]), pairs(&[(0, "random")]));
    }

    #[test]
    fn typing_a_start_reorders() {
        let mut t = weekly();
        assert_eq!(t.set_start(0, 0, 1200), Some(1));
        assert_eq!(ids(&t.week[0]), pairs(&[(1170, "b"), (1200, "a")]));
        assert_eq!(t.set_start(0, 1, 1170), None);
        assert_eq!(t.set_start(0, 1, DAY_MINUTES), None);
        assert_eq!(t.set_start(0, 0, 1170), Some(0));
    }

    #[test]
    fn copying_days_and_switching_layouts() {
        let mut t = weekly();
        t.copy_day(0, &[1, 2, 0, 6]);
        assert_eq!(t.week[1], t.week[0]);
        assert_eq!(t.week[6], t.week[0]);
        assert!(t.week[3].is_empty());
        t.make_single(3);
        assert_eq!(t.layout, Layout::Single);
        assert!(t.single.is_empty());
        assert!(t.week.iter().all(Vec::is_empty));
        let mut s = Timeline {
            single: vec![item(420, "a")],
            ..Timeline::default()
        };
        s.make_weekly();
        assert_eq!(s.layout, Layout::Weekly);
        assert!(s.single.is_empty());
        assert!(s.week.iter().all(|d| ids(d) == pairs(&[(420, "a")])));
        // Round trip keeps the chosen day.
        s.week[4] = vec![item(60, "fri")];
        s.make_single(4);
        assert_eq!(ids(&s.single), pairs(&[(60, "fri")]));
        // The file keeps what the editor made.
        assert_eq!(Timeline::from_toml(&s.to_toml()).unwrap(), s);
    }
}
