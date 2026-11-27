//! New wallpapers, going back, and the timeline: what goes on the
//! desktop next, and when.

use crate::*;

impl App {
    pub(crate) fn timeline_mode(&self) -> bool {
        self.settings.rotation.mode == settings::RotationMode::Timeline && !self.timeline.is_empty()
    }

    pub(crate) fn interval(&self) -> Option<Duration> {
        // In Timeline mode the slots decide, except on an empty day.
        if self.timeline_mode() && !self.timeline_generate {
            return None;
        }
        let m = self.settings.schedule.change_every_minutes;
        (m > 0 && !self.settings.schedule.paused).then(|| Duration::from_secs(u64::from(m) * 60))
    }

    pub(crate) fn reschedule(&mut self) {
        self.next_change = self.interval().map(|d| Instant::now() + d);
        if let Some(t) = &self.tray {
            t.show_paused(self.settings.schedule.paused);
        }
    }

    pub(crate) fn set_current_live(&mut self, live: Option<bool>) {
        if self.current_live != live {
            self.current_live = live;
            self.follow_display_settings();
        }
    }

    /// Starts automatic wallpaper changes again (after Pause, or after Restore
    /// put the user's own wallpaper back).
    pub(crate) fn resume_changes(&mut self) {
        let was_restored = self.restored();
        self.set_restored(false);
        self.settings.schedule.paused = false;
        self.save_settings();
        self.status.note = None;
        if self.timeline_mode() {
            if was_restored || self.current.is_none() {
                self.timeline_since = None;
            }
            self.check_timeline();
        } else if was_restored || self.current.is_none() {
            self.new_wallpaper();
        }
        self.reschedule();
    }

    pub(crate) fn new_wallpaper(&mut self) {
        self.chosen = false;
        self.current_theme = None;
        self.current_buddy = None;
        self.set_current_live(None);
        let seed = random_seed();
        let at = self.apply_at.take();
        let scrolling = self.live.scrolling_ok();
        self.submit(Job::New {
            seed,
            exact: false,
            settings: Box::new(self.settings.clone()),
            at,
            scrolling,
        });
    }

    /// The wallpaper a typed-in seed makes: from the archive or the library
    /// when it's there (exactly as it was shown), otherwise made again from
    /// the seed (the same scenes for the same Content choices and theme).
    pub(crate) fn wallpaper_from_seed(&mut self, seed: u64) {
        if self.timeline_mode() && !self.timeline_generate {
            self.manual_until = self.timeline_next;
        }
        let archived = self.archive().and_then(|a| {
            a.entries()
                .into_iter()
                .filter(|e| e.seed == seed)
                .max_by_key(|e| e.created)
                .map(|e| a.dir().join(e.name))
        });
        let saved = self.library().and_then(|lib| {
            lib.items()
                .into_iter()
                .find(|i| i.seed == seed)
                .map(|i| (lib.item_dir(&i.id), i))
        });
        if let Some((dir, item)) = saved {
            let (theme, buddy, live) = Self::item_choices(
                &item,
                library::ThemeChoice::Follow,
                library::BuddyChoice::Follow,
                library::DisplayChoice::Follow,
            );
            self.current_theme = theme;
            self.current_buddy = buddy;
            self.set_current_live(live);
            self.chosen = true;
            if !self.apply_folder(dir, theme, buddy, false, false) {
                self.status.note = Some("That saved wallpaper couldn't be read.".into());
            }
        } else if let Some(dir) = archived {
            let (theme, buddy, live) = Self::photo_choices_for_dir(&dir);
            self.current_theme = theme;
            self.current_buddy = buddy;
            self.set_current_live(live);
            self.chosen = true;
            if !self.apply_folder(dir, theme, buddy, false, false) {
                self.status.note = Some("That saved wallpaper couldn't be read.".into());
            }
        } else {
            self.chosen = true;
            self.current_theme = None;
            self.current_buddy = None;
            self.set_current_live(None);
            let scrolling = self.live.scrolling_ok();
            self.submit(Job::New {
                seed,
                exact: true,
                settings: Box::new(self.settings.clone()),
                at: None,
                scrolling,
            });
        }
        self.reschedule();
    }

    pub(crate) fn archive(&self) -> Option<archive::Archive> {
        archive::Archive::open(self.data_dir.join("archive")).ok()
    }

    pub(crate) fn photo_choices_for_dir(dir: &Path) -> (Option<Theme>, Option<bool>, Option<bool>) {
        use clairlune_render::img2ascii::AsciiColorMode;
        if let Some(meta) = apply::read_photo_meta(dir) {
            let th = match meta.config.color_mode {
                AsciiColorMode::Black => Some(Theme::Dark),
                AsciiColorMode::White => Some(Theme::Light),
                _ => None,
            };
            (th, Some(false), Some(meta.config.has_live_weather()))
        } else {
            (None, None, None)
        }
    }

    /// Re-applies saved scenes: the previous archive entry, or what's on
    /// screen again (for a theme change).
    pub(crate) fn reapply(&mut self, previous: bool) {
        let Some(archive) = self.archive() else {
            return;
        };
        let (dir, theme, buddy) = if previous {
            // The archive entry before the current one; from a library item,
            // the newest archive entry.
            let current_name = self
                .current
                .as_ref()
                .filter(|p| p.starts_with(archive.dir()))
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned());
            let entry = match current_name {
                Some(n) => archive.before(&n),
                None => archive.latest(),
            };
            let Some(entry) = entry else {
                return;
            };
            let entry_dir = archive.dir().join(entry.name);
            let (th, bd, lv) = Self::photo_choices_for_dir(&entry_dir);
            self.current_theme = th;
            self.current_buddy = bd;
            self.set_current_live(lv);
            (entry_dir, th, bd)
        } else {
            let Some(dir) = self.current.clone() else {
                return;
            };
            (dir, self.current_theme, self.current_buddy)
        };
        if previous {
            self.chosen = true;
        }
        // Going back shows the older entry as it is (archiving it again
        // would make the next Previous come back here).
        let refresh = !previous && !self.chosen;
        let _ = self.apply_folder(dir, theme, buddy, false, refresh);
    }

    /// A new scene on one monitor (its id); the others keep theirs. It is
    /// a new wallpaper, archived, so Previous goes back.
    pub(crate) fn new_wallpaper_on(&mut self, monitor: String) {
        let Some(dir) = self.current.clone() else {
            return self.new_wallpaper_by_hand();
        };
        self.current_theme = None;
        self.current_buddy = None;
        self.set_current_live(None);
        self.chosen = false;
        self.renew = Some((monitor, random_seed()));
        // Archived only if a monitor really got a new scene (it may have
        // been unplugged since the menu was made).
        let _ = self.apply_folder(dir, None, None, false, false);
    }

    /// Shows the scenes saved in a folder (archive entry or library item).
    /// Returns whether the saved recipes could be read and submitted.
    pub(crate) fn apply_folder(
        &mut self,
        dir: PathBuf,
        theme: Option<Theme>,
        buddy: Option<bool>,
        archive: bool,
        refresh: bool,
    ) -> bool {
        let recipes = std::fs::read_to_string(dir.join("recipe.json"))
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()));
        let at = self.apply_at.take();
        let renew = self.renew.take();
        let scrolling = self.live.scrolling_ok();
        match recipes {
            Ok(recipes) => {
                self.submit(Job::Recipes {
                    at,
                    recipes: Box::new(recipes),
                    source: Some(dir),
                    theme,
                    buddy,
                    settings: Box::new(self.settings.clone()),
                    archive,
                    renew,
                    scrolling,
                    refresh,
                });
                true
            }
            Err(e) => {
                tracing::warn!(target: "clairlune", dir = %dir.display(), error = %e, "unreadable saved scene");
                false
            }
        }
    }

    pub(crate) fn save_pools(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.pools) {
            let _ = std::fs::write(self.data_dir.join("state.json"), json);
        }
    }

    /// Applies the timeline's active slot when it changed.
    pub(crate) fn check_timeline(&mut self) {
        if !self.timeline_mode() {
            if self.timeline_since.take().is_some() {
                self.timeline_applied = None;
                self.timeline_generate = false;
                self.set_current_live(None);
                self.reschedule();
            }
            return;
        }
        let now = jiff::Timestamp::now();
        let tz = jiff::tz::TimeZone::system();
        let Some(active) = timeline::resolve(&self.timeline, &tz, now) else {
            return;
        };
        self.timeline_next = Some(active.next);
        if self.ui_test.is_some() {
            return; // the UI test leaves the wallpaper alone
        }
        // Paused: the wallpaper stays until you resume.
        if self.settings.schedule.paused {
            return;
        }
        // Shortly before the next slot starts, make it now and show it at
        // its start.
        if !self.busy && self.timeline_since != Some(active.next) {
            let ahead = active.next.duration_since(now);
            if ahead > jiff::SignedDuration::ZERO
                && ahead <= TIMELINE_LEAD
                && let Some(up) = timeline::resolve(&self.timeline, &tz, active.next)
                && up.since == active.next
                && let Ok(wait) = std::time::Duration::try_from(ahead)
            {
                tracing::info!(target: "clairlune", seconds = wait.as_secs_f32(), "timeline: preparing the next slot");
                self.manual_until = None;
                self.timeline_since = Some(up.since);
                self.apply_at = Some(Instant::now() + wait);
                self.apply_pick(up.pick, up.since);
                self.apply_at = None;
                return;
            }
        }
        // The next slot is already on its way.
        if self.timeline_since.is_some_and(|s| s > now) {
            return;
        }
        if let Some(until) = self.manual_until {
            if now < until {
                return;
            }
            self.manual_until = None;
        }
        if self.timeline_since == Some(active.since) || self.busy {
            return;
        }
        self.timeline_since = Some(active.since);
        self.apply_pick(active.pick, active.since);
    }

    /// Shows what the timeline picked: a slot, or new scenes on an empty day.
    pub(crate) fn apply_pick(&mut self, pick: timeline::Pick, since: jiff::Timestamp) {
        match pick {
            timeline::Pick::Generate => {
                tracing::info!(target: "clairlune", "timeline: empty day, generating");
                self.timeline_applied = None;
                self.timeline_generate = true;
                self.new_wallpaper();
                self.reschedule();
            }
            timeline::Pick::Slot { weekday, index } => {
                self.timeline_applied = self.timeline.day(weekday).get(index).cloned();
                self.timeline_generate = false;
                self.next_change = None;
                self.apply_slot(weekday, index, since.as_second() as u64);
            }
        }
    }

    /// Shows one timeline slot.
    pub(crate) fn apply_slot(&mut self, weekday: jiff::civil::Weekday, index: usize, seed: u64) {
        let Some(slot) = self.timeline.day(weekday).get(index).cloned() else {
            return;
        };
        // Pools keep their place per slot: per weekday slot with one
        // plan per weekday, per slot of the one plan for every day.
        let key = if self.timeline.day_count() == 1 {
            format!("every-day-{index}")
        } else {
            format!(
                "{}-{index}",
                timeline::day_name(timeline::weekday_index(weekday))
            )
        };
        tracing::info!(target: "clairlune", slot = %key, start = %timeline::format_time(slot.start), "timeline slot");
        match &slot.content {
            timeline::Content::Item { id } => self.show_slot_item(id, &slot),
            timeline::Content::Pool { ids, tag, order } => {
                let items = self.library().map(|l| l.items()).unwrap_or_default();
                let mut candidates: Vec<String> = ids
                    .iter()
                    .filter(|id| items.iter().any(|i| &i.id == *id))
                    .cloned()
                    .collect();
                if let Some(tag) = tag {
                    for i in items.iter().filter(|i| i.tags.contains(tag)) {
                        if !candidates.contains(&i.id) {
                            candidates.push(i.id.clone());
                        }
                    }
                }
                let last = self.pools.last.get(&key).cloned();
                let position = self.pools.positions.entry(key.clone()).or_default();
                match timeline::pick_from_pool(&candidates, *order, seed, last.as_deref(), position)
                {
                    Some(id) => {
                        self.pools.last.insert(key, id.clone());
                        self.save_pools();
                        self.show_slot_item(&id, &slot);
                    }
                    None => {
                        tracing::warn!(target: "clairlune", "timeline pool is empty; generating");
                        self.generate_for_slot(&slot, &[], &[], &[]);
                    }
                }
            }
            timeline::Content::Random {
                archetypes,
                activities,
                weather,
            } => {
                self.generate_for_slot(&slot, archetypes, activities, weather);
            }
        }
    }

    /// Generates a fresh wallpaper honouring a timeline slot's theme, Clawd,
    /// display and content choices (across per-monitor choices too).
    pub(crate) fn generate_for_slot(
        &mut self,
        slot: &timeline::Slot,
        archetypes: &[String],
        activities: &[String],
        weather: &[String],
    ) {
        let s = slot_settings(&self.settings, slot, archetypes, activities, weather);
        let (theme, buddy, live) = Self::resolve_choices(slot.theme, slot.buddy, slot.display);
        // The slot's own choices hold on screen: a theme change doesn't
        // re-render them away.
        self.current_theme = theme;
        self.current_buddy = buddy;
        self.set_current_live(live);
        self.chosen = false;
        let at = self.apply_at.take();
        let scrolling = self.live.scrolling_ok();
        self.submit(Job::New {
            seed: random_seed(),
            exact: false,
            settings: Box::new(s),
            at,
            scrolling,
        });
    }

    /// Shows a library item for a timeline slot; the slot's choices win over
    /// the item's, which win over the global settings. Falls back to a fresh
    /// scene with the slot's choices if the saved item is missing or unreadable.
    pub(crate) fn show_slot_item(&mut self, id: &str, slot: &timeline::Slot) {
        let Some(lib) = self.library() else {
            self.generate_for_slot(slot, &[], &[], &[]);
            return;
        };
        let Some(item) = lib.get(id) else {
            tracing::warn!(target: "clairlune", id, "timeline item is gone; generating");
            self.generate_for_slot(slot, &[], &[], &[]);
            return;
        };
        let (theme, buddy, live) = Self::item_choices(&item, slot.theme, slot.buddy, slot.display);
        self.current_theme = theme;
        self.current_buddy = buddy;
        self.set_current_live(live);
        self.chosen = true;
        if !self.apply_folder(lib.item_dir(id), theme, buddy, false, false) {
            self.generate_for_slot(slot, &[], &[], &[]);
        }
    }

    /// After the timeline changed (in the editor or in the file): the active
    /// slot is shown afresh only if what it shows changed, so moving another
    /// slot doesn't replace the wallpaper.
    pub(crate) fn timeline_replanned(&mut self) {
        let active = timeline::resolve(
            &self.timeline,
            &jiff::tz::TimeZone::system(),
            jiff::Timestamp::now(),
        );
        let Some(active) = active else {
            self.timeline_since = None;
            return;
        };
        let same = match (&active.pick, &self.timeline_applied) {
            (timeline::Pick::Slot { weekday, index }, Some(applied)) => self
                .timeline
                .day(*weekday)
                .get(*index)
                .is_some_and(|s| s.shows_same(applied)),
            (timeline::Pick::Generate, None) => self.timeline_generate,
            _ => false,
        };
        if same && self.timeline_since.is_some() {
            self.timeline_since = Some(active.since);
            self.timeline_next = Some(active.next);
        } else {
            self.timeline_since = None;
        }
    }

    /// The Timeline window edited the timeline: save it and follow it.
    pub(crate) fn timeline_edited(&mut self) {
        if let Err(e) = self.timeline.save(&self.timeline_path) {
            tracing::warn!(target: "clairlune", error = %e, "could not save the timeline");
            self.status.note = Some(format!("Couldn't save the timeline: {e}"));
        }
        self.timeline_stamp = file_stamp(&self.timeline_path);
        self.timeline_replanned();
        self.check_timeline();
        self.refresh_library();
    }

    /// "New wallpaper" by hand: in Timeline mode it shows until the next slot.
    pub(crate) fn new_wallpaper_by_hand(&mut self) {
        if self.timeline_mode() && !self.timeline_generate {
            self.manual_until = self.timeline_next;
        }
        self.new_wallpaper();
        self.reschedule();
    }

    /// When to prepare the next timeline slot, if one is coming and isn't
    /// prepared yet.
    pub(crate) fn timeline_prepare_at(&self) -> Option<Instant> {
        if !self.timeline_mode() || self.ui_test.is_some() {
            return None;
        }
        let next = self.timeline_next?;
        if self.timeline_since == Some(next) {
            return None;
        }
        let ahead = next.duration_since(jiff::Timestamp::now()) - TIMELINE_LEAD;
        let wait = std::time::Duration::try_from(ahead).unwrap_or(Duration::ZERO);
        Some(Instant::now() + wait)
    }
}

/// Applies a timeline slot's content and appearance overrides to `base`,
/// narrowing per-monitor content choices too when the slot names landscapes,
/// activities or weather.
fn slot_settings(
    base: &Settings,
    slot: &timeline::Slot,
    archetypes: &[String],
    activities: &[String],
    weather: &[String],
) -> Settings {
    let mut s = base.clone();
    let narrow = |c: &mut settings::Content| {
        if !archetypes.is_empty() {
            c.archetypes = archetypes.to_vec();
        }
        if !activities.is_empty() {
            c.activities = activities.to_vec();
        }
        if !weather.is_empty() {
            c.weather = weather.to_vec();
        }
    };
    narrow(&mut s.content);
    for m in &mut s.display.monitors {
        if let Some(c) = &mut m.content {
            narrow(c);
        }
    }
    match slot.theme {
        library::ThemeChoice::Light => s.theme.mode = settings::ThemeMode::Light,
        library::ThemeChoice::Dark => s.theme.mode = settings::ThemeMode::Dark,
        library::ThemeChoice::Follow => {}
    }
    match slot.buddy {
        library::BuddyChoice::On => s.buddy.show = true,
        library::BuddyChoice::Off => s.buddy.show = false,
        library::BuddyChoice::Follow => {}
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_settings_narrow_per_monitor_choices_and_apply_overrides() {
        let mut base = Settings::default();
        base.edit_monitor("m2", "Right", |m| {
            m.content = Some(settings::Content {
                archetypes: vec!["lake".into()],
                activities: vec!["fish".into()],
                weather: vec!["rain".into()],
                ..settings::Content::default()
            });
        });
        let slot = timeline::Slot {
            start: 420,
            content: timeline::Content::Random {
                archetypes: vec!["night_sky".into()],
                activities: Vec::new(),
                weather: vec!["aurora".into()],
            },
            display: library::DisplayChoice::Live,
            theme: library::ThemeChoice::Dark,
            buddy: library::BuddyChoice::Off,
        };
        let out = slot_settings(&base, &slot, &["night_sky".into()], &[], &["aurora".into()]);
        assert_eq!(out.content.archetypes, ["night_sky"]);
        assert_eq!(out.content.weather, ["aurora"]);
        let m2 = out.monitor("m2").unwrap().content.as_ref().unwrap();
        assert_eq!(m2.archetypes, ["night_sky"]);
        assert_eq!(
            m2.activities,
            ["fish"],
            "unspecified filters keep monitor choices"
        );
        assert_eq!(m2.weather, ["aurora"]);
        assert_eq!(out.theme.mode, settings::ThemeMode::Dark);
        assert!(!out.buddy.show);
    }

    #[test]
    fn slot_choices_override_library_item_choices() {
        let item = library::Item {
            id: "lib_1".into(),
            name: "Shore".into(),
            created: 1,
            tags: Vec::new(),
            display: library::DisplayChoice::Still,
            theme: library::ThemeChoice::Light,
            buddy: library::BuddyChoice::Off,
            seed: 7,
            archetype: "shore".into(),
            activity: None,
            generator_version: 1,
        };
        assert_eq!(
            App::item_choices(
                &item,
                library::ThemeChoice::Follow,
                library::BuddyChoice::Follow,
                library::DisplayChoice::Follow,
            ),
            (Some(Theme::Light), Some(false), Some(false))
        );
        assert_eq!(
            App::item_choices(
                &item,
                library::ThemeChoice::Dark,
                library::BuddyChoice::On,
                library::DisplayChoice::Live,
            ),
            (Some(Theme::Dark), Some(true), Some(true))
        );
    }
}
