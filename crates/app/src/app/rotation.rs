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
    }

    pub(crate) fn new_wallpaper(&mut self) {
        self.chosen = false;
        self.current_theme = None;
        self.current_buddy = None;
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
        if let Some(dir) = archived {
            self.current_theme = None;
            self.current_buddy = None;
            self.chosen = true;
            self.apply_folder(dir, None, None, false, false);
        } else if let Some((dir, item)) = saved {
            let (theme, buddy) = Self::library_choices(&item, &self.settings);
            self.current_theme = theme;
            self.current_buddy = buddy;
            self.chosen = true;
            self.apply_folder(dir, theme, buddy, false, false);
        } else {
            self.chosen = true;
            self.current_theme = None;
            self.current_buddy = None;
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
            self.current_theme = None;
            self.current_buddy = None;
            (archive.dir().join(entry.name), None, None)
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
        self.apply_folder(dir, theme, buddy, false, refresh);
    }

    /// A new scene on one monitor (its id); the others keep theirs. It is
    /// a new wallpaper, archived, so Previous goes back.
    pub(crate) fn new_wallpaper_on(&mut self, monitor: String) {
        let Some(dir) = self.current.clone() else {
            return self.new_wallpaper_by_hand();
        };
        self.current_theme = None;
        self.current_buddy = None;
        self.chosen = false;
        self.renew = Some((monitor, random_seed()));
        // Archived only if a monitor really got a new scene (it may have
        // been unplugged since the menu was made).
        self.apply_folder(dir, None, None, false, false);
    }

    /// Shows the scenes saved in a folder (archive entry or library item).
    pub(crate) fn apply_folder(
        &mut self,
        dir: PathBuf,
        theme: Option<Theme>,
        buddy: Option<bool>,
        archive: bool,
        refresh: bool,
    ) {
        let recipes = std::fs::read_to_string(dir.join("recipe.json"))
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()));
        let at = self.apply_at.take();
        let renew = self.renew.take();
        let scrolling = self.live.scrolling_ok();
        match recipes {
            Ok(recipes) => self.submit(Job::Recipes {
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
            }),
            Err(e) => {
                tracing::warn!(target: "clairlune", dir = %dir.display(), error = %e, "unreadable saved scene")
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
                self.timeline_generate = false;
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
        match slot.content {
            timeline::Content::Item { id } => self.show_library_item(&id, slot.theme, slot.buddy),
            timeline::Content::Pool { ids, tag, order } => {
                let items = self.library().map(|l| l.items()).unwrap_or_default();
                let mut candidates: Vec<String> = ids
                    .into_iter()
                    .filter(|id| items.iter().any(|i| &i.id == id))
                    .collect();
                if let Some(tag) = tag {
                    for i in items.iter().filter(|i| i.tags.contains(&tag)) {
                        if !candidates.contains(&i.id) {
                            candidates.push(i.id.clone());
                        }
                    }
                }
                let last = self.pools.last.get(&key).cloned();
                let position = self.pools.positions.entry(key.clone()).or_default();
                match timeline::pick_from_pool(&candidates, order, seed, last.as_deref(), position)
                {
                    Some(id) => {
                        self.pools.last.insert(key, id.clone());
                        self.save_pools();
                        self.show_library_item(&id, slot.theme, slot.buddy);
                    }
                    None => {
                        tracing::warn!(target: "clairlune", "timeline pool is empty; generating");
                        self.new_wallpaper();
                    }
                }
            }
            timeline::Content::Random {
                archetypes,
                activities,
                weather,
            } => {
                let mut s = self.settings.clone();
                if !archetypes.is_empty() {
                    s.content.archetypes = archetypes;
                }
                if !activities.is_empty() {
                    s.content.activities = activities;
                }
                if !weather.is_empty() {
                    s.content.weather = weather;
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
                // The slot's own choices hold on screen: a theme change
                // doesn't re-render them away.
                self.current_theme = match slot.theme {
                    library::ThemeChoice::Light => Some(Theme::Light),
                    library::ThemeChoice::Dark => Some(Theme::Dark),
                    library::ThemeChoice::Follow => None,
                };
                self.current_buddy = match slot.buddy {
                    library::BuddyChoice::On => Some(true),
                    library::BuddyChoice::Off => Some(false),
                    library::BuddyChoice::Follow => None,
                };
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
        }
    }

    /// Shows a library item; a slot's choices win over the item's, which win
    /// over the global settings.
    pub(crate) fn show_library_item(
        &mut self,
        id: &str,
        slot_theme: library::ThemeChoice,
        slot_buddy: library::BuddyChoice,
    ) {
        let Some(lib) = self.library() else {
            return;
        };
        let Some(mut item) = lib.get(id) else {
            tracing::warn!(target: "clairlune", id, "timeline item is gone; generating");
            self.new_wallpaper();
            return;
        };
        if slot_theme != library::ThemeChoice::Follow {
            item.theme = slot_theme;
        }
        if slot_buddy != library::BuddyChoice::Follow {
            item.buddy = slot_buddy;
        }
        let (theme, buddy) = Self::library_choices(&item, &self.settings);
        self.current_theme = theme;
        self.current_buddy = buddy;
        self.chosen = true;
        self.apply_folder(lib.item_dir(id), theme, buddy, false, false);
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
