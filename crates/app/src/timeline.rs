//! The daily timeline (PLAN.md section 9b): a schedule of wallpapers by time
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
//! The resolver is generic over `chrono::TimeZone` and takes `now`, so tests
//! run it in real zones with chrono-tz.

use crate::library::{BuddyChoice, DisplayChoice, ThemeChoice};
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, TimeZone, Utc, Weekday};
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
            Layout::Weekly => &self.week[weekday.num_days_from_monday() as usize],
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
            Layout::Single => vec![("every day", &self.single)],
            Layout::Weekly => DAY_NAMES
                .iter()
                .zip(&self.week)
                .map(|(n, s)| (*n, s.as_slice()))
                .collect(),
        };
        for (day, slots) in days {
            for s in slots {
                let at = format!("{day} {}", format_time(s.start));
                match &s.content {
                    Content::Item { id } if !item_exists(id) => {
                        out.push(format!("{at}: library item `{id}` is gone"));
                    }
                    Content::Pool { ids, .. } => {
                        for id in ids.iter().filter(|id| !item_exists(id)) {
                            out.push(format!("{at}: pool item `{id}` is gone"));
                        }
                    }
                    Content::Random {
                        archetypes,
                        activities,
                        ..
                    } => {
                        for a in archetypes {
                            if clairlune_scene::archetype::archetype(a).is_none() {
                                out.push(format!("{at}: unknown landscape `{a}`"));
                            }
                        }
                        for a in activities {
                            if clairlune_scene::activity::builtin_catalog()
                                .get(a)
                                .is_none()
                            {
                                out.push(format!("{at}: unknown activity `{a}`"));
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
    /// When it started (UTC).
    pub since: DateTime<Utc>,
    /// When the next slot (or empty day) starts.
    pub next: DateTime<Utc>,
}

/// The UTC instant of a local wall-clock time: the first occurrence when it
/// repeats, the first valid instant after it when it is skipped.
fn instant_of<Tz: TimeZone>(tz: &Tz, date: NaiveDate, minute: u16) -> Option<DateTime<Utc>> {
    let start = date.and_hms_opt(u32::from(minute / 60), u32::from(minute % 60), 0)?;
    match tz.from_local_datetime(&start) {
        LocalResult::Single(t) => Some(t.with_timezone(&Utc)),
        LocalResult::Ambiguous(a, b) => Some(a.with_timezone(&Utc).min(b.with_timezone(&Utc))),
        LocalResult::None => {
            // In a gap: step forward a minute at a time (gaps are at most a
            // few hours) to the first local time that exists.
            (1..=6 * 60).find_map(|k| {
                match tz.from_local_datetime(&(start + Duration::minutes(k))) {
                    LocalResult::Single(t) => Some(t.with_timezone(&Utc)),
                    LocalResult::Ambiguous(a, b) => {
                        Some(a.with_timezone(&Utc).min(b.with_timezone(&Utc)))
                    }
                    LocalResult::None => None,
                }
            })
        }
    }
}

/// Resolves the timeline at `now` in `tz`. `None` for an empty timeline.
pub fn resolve<Tz: TimeZone>(tl: &Timeline, tz: &Tz, now: DateTime<Utc>) -> Option<Active> {
    if tl.is_empty() {
        return None;
    }
    let today = now.with_timezone(tz).date_naive();
    // Events from yesterday to the day after tomorrow: (instant, order, pick).
    // `order` keeps definition order so that, at one instant, the
    // latest-defined slot wins.
    let mut events: Vec<(DateTime<Utc>, usize, Pick)> = Vec::new();
    let mut order = 0usize;
    for offset in -1..=2i64 {
        let date = today + Duration::days(offset);
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
    let mut collapsed: Vec<(DateTime<Utc>, Pick)> = Vec::new();
    for (t, _, pick) in events {
        if collapsed.last().is_some_and(|(lt, _)| *lt == t) {
            collapsed.pop();
        }
        collapsed.push((t, pick));
    }
    let i = collapsed.iter().rposition(|(t, _)| *t <= now)?;
    let next = collapsed
        .get(i + 1)
        .map(|(t, _)| *t)
        .unwrap_or(now + Duration::days(1));
    Some(Active {
        pick: collapsed[i].1.clone(),
        since: collapsed[i].0,
        next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::America::New_York;

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

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
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
        let a = resolve(&tl, &New_York, utc("2026-01-15T05:30:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "night");
        assert_eq!(a.since, utc("2026-01-15T04:00:00Z"));
        assert_eq!(a.next, utc("2026-01-15T12:00:00Z"));
        let a = resolve(&tl, &New_York, utc("2026-01-15T12:00:00Z")).unwrap();
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
        let a = resolve(&tl, &New_York, utc("2026-03-08T07:00:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "gap");
        // It starts at 03:00 EDT = 07:00 UTC, the first valid instant after 02:30.
        assert_eq!(a.since, utc("2026-03-08T07:00:00Z"));
        let before = resolve(&tl, &New_York, utc("2026-03-08T06:59:00Z")).unwrap();
        assert_eq!(picked(&tl, &before), "early");
    }

    #[test]
    fn slots_in_a_gap_at_one_instant_go_to_the_latest_defined() {
        let tl = single(vec![slot("02:15", "first"), slot("02:45", "second")]);
        let a = resolve(&tl, &New_York, utc("2026-03-08T07:30:00Z")).unwrap();
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
        let a = resolve(&tl, &New_York, utc("2026-11-01T05:45:00Z")).unwrap();
        assert_eq!(picked(&tl, &a), "repeat");
        assert_eq!(a.since, utc("2026-11-01T05:30:00Z"));
        // ...and during the second 01:30 (EST, 06:30 UTC) it is still the
        // same slot from the same start: it did not fire again.
        let b = resolve(&tl, &New_York, utc("2026-11-01T06:45:00Z")).unwrap();
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
        let mon = resolve(&tl, &New_York, utc("2026-01-12T15:00:00Z")).unwrap();
        assert_eq!(picked(&tl, &mon), "monday");
        let tue = resolve(&tl, &New_York, utc("2026-01-13T15:00:00Z")).unwrap();
        assert_eq!(tue.pick, Pick::Generate);
        assert_eq!(tue.since, utc("2026-01-13T05:00:00Z"));
        assert!(resolve(&Timeline::default(), &New_York, utc("2026-01-13T15:00:00Z")).is_none());
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
                archetypes: vec!["volcano".into()],
                activities: vec!["juggling".into()],
                weather: Vec::new(),
            },
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
        });
        let p = tl.problems(&|id| id == "lib_here");
        assert_eq!(p.len(), 3, "{p:?}");
        assert!(p[0].contains("lib_gone"));
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
            Layout::Weekly => {
                let mut d = DAY_NAMES[day].to_string();
                d[..1].make_ascii_uppercase();
                format!("{d} {}", format_time(s.start))
            }
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
