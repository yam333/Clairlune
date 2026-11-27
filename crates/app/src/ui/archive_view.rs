//! The Archive window (docs/dev/design.md section 4): every wallpaper the app set
//! recently, newest first and grouped by day, with one action bar for the
//! selection. Thumbnails are the content; there are no per-tile buttons.

use super::style::{self, c, colors};
use super::view::{Status, UiAction, empty_page, hairline, primary_button};
use crate::archive::Entry;
use crate::tokens::{radius, size, space};
use clairlune_scene::Theme;
use egui::{Align, Layout, RichText, Sense, Stroke, Ui, vec2};
use std::path::Path;

#[derive(Default)]
pub struct ArchiveState {
    selected: Option<String>,
    query: String,
    confirm_delete: bool,
    thumbs: super::Thumbs,
}

impl ArchiveState {
    /// Selects an entry (UI test).
    pub fn select(&mut self, name: &str) {
        self.selected = Some(name.to_string());
        self.confirm_delete = false;
    }
}

const THUMB_W: f32 = 192.0;

/// What the archive window needs from the app.
pub struct ArchiveModel<'a> {
    pub entries: &'a [Entry],
    pub dir: &'a Path,
    /// The entry on screen, if it's an archive entry.
    pub current: Option<&'a str>,
    pub keep_days: u32,
    pub max_mb: u64,
    pub status: &'a Status,
}

/// "214 MB", "1 GB", "1.5 GB".
fn megabytes(mb: f64) -> String {
    if mb >= 1024.0 {
        let gb = mb / 1024.0;
        if (gb - gb.round()).abs() < 0.05 {
            format!("{gb:.0} GB")
        } else {
            format!("{gb:.1} GB")
        }
    } else {
        format!("{mb:.0} MB")
    }
}

/// "14:00 stroll on the shore" style words for an entry.
fn describe(e: &Entry) -> String {
    let place = e.archetype.replace('_', " ");
    match &e.activity {
        Some(a) => format!("{} in {place}", a.replace('_', " ")),
        None => place,
    }
}

fn matches_archive_filter(e: &Entry, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    describe(e).to_lowercase().contains(&q)
        || e.theme.to_lowercase().contains(&q)
        || e.seed.to_string().contains(&q)
        || (q == "pinned" && e.pinned)
}

fn local(e: &Entry) -> Option<jiff::ZonedDateTime> {
    jiff::Timestamp::from_second(e.created as i64)
        .ok()
        .map(|t| t.to_zoned_datetime(jiff::tz::TimeZone::system()))
}

/// "Today", "Yesterday", or the date.
fn day_label(e: &Entry, today: Option<jiff::civil::Date>) -> String {
    let Some(z) = local(e) else {
        return "Earlier".into();
    };
    let d = z.date();
    match today {
        Some(t) if d == t => "Today".into(),
        Some(t) if t.yesterday().ok() == Some(d) => "Yesterday".into(),
        _ => z.strftime("%A %-d %B").to_string(),
    }
}

fn time_label(e: &Entry) -> String {
    local(e)
        .map(|z| z.strftime("%H:%M").to_string())
        .unwrap_or_default()
}

pub fn show(
    ui: &mut Ui,
    theme: Theme,
    st: &mut ArchiveState,
    m: &ArchiveModel,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    if let Some(sel) = &st.selected
        && !m.entries.iter().any(|e| &e.name == sel)
    {
        st.selected = None;
    }
    let selected = st
        .selected
        .as_ref()
        .and_then(|n| m.entries.iter().find(|e| &e.name == n))
        .cloned();
    if let Some(entry) = &selected {
        egui::Panel::bottom("selection")
            .frame(
                egui::Frame::new()
                    .fill(c(t.bg_sidebar))
                    .inner_margin(egui::Margin::symmetric(space::S6 as i8, space::S4 as i8)),
            )
            .show(ui, |ui| action_bar(ui, theme, st, entry, m, actions));
    }
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(c(t.bg))
                .inner_margin(egui::Margin::symmetric(space::S6 as i8, space::S5 as i8)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Archive").font(style::title()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Open folder").clicked() {
                        actions.push(UiAction::OpenArchiveFolder);
                    }
                    ui.add_space(space::S3);
                    let used: u64 = m
                        .entries
                        .iter()
                        .filter(|e| !e.pinned)
                        .map(|e| e.bytes)
                        .sum();
                    let keep = match m.keep_days {
                        0 => "Kept until the size cap".to_string(),
                        1 => "Keep 1 day".to_string(),
                        n => format!("Keep {n} days"),
                    };
                    ui.label(
                        RichText::new(format!(
                            "{keep} \u{b7} {} of {}",
                            megabytes(used as f64 / (1024.0 * 1024.0)),
                            megabytes(m.max_mb as f64)
                        ))
                        .font(style::mono())
                        .color(c(t.text_secondary)),
                    );
                    if !m.entries.is_empty() {
                        ui.add_space(space::S3);
                        if !st.query.trim().is_empty() && ui.button("Clear").clicked() {
                            st.query.clear();
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut st.query)
                                .desired_width(180.0)
                                .hint_text("Search scene or seed"),
                        );
                    }
                });
            });
            ui.add_space(space::S2);
            hairline(ui, t.hairline);
            ui.add_space(space::S4);
            if m.entries.is_empty() {
                empty_page(
                    ui,
                    theme,
                    "Nothing archived yet",
                    "Every wallpaper the app sets is kept here for a while.",
                );
                return;
            }
            let newest_first: Vec<&Entry> = m
                .entries
                .iter()
                .rev()
                .filter(|e| matches_archive_filter(e, &st.query))
                .collect();
            if newest_first.is_empty() {
                empty_page(
                    ui,
                    theme,
                    "No matching wallpapers",
                    "Try another search or clear the filter.",
                );
                return;
            }
            let today = jiff::ZonedDateTime::now().date();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut i = 0;
                while i < newest_first.len() {
                    let label = day_label(newest_first[i], Some(today));
                    let mut j = i;
                    while j < newest_first.len() && day_label(newest_first[j], Some(today)) == label
                    {
                        j += 1;
                    }
                    ui.label(RichText::new(&label).font(style::body_strong()));
                    ui.add_space(space::S2);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(space::S5, space::S5);
                        for e in &newest_first[i..j] {
                            tile(ui, theme, st, m, e, actions);
                        }
                    });
                    ui.add_space(space::S5);
                    i = j;
                }
            });
        });
}

/// One thumbnail with its time and scene under it; click to select, double-click
/// to apply.
fn tile(
    ui: &mut Ui,
    theme: Theme,
    st: &mut ArchiveState,
    m: &ArchiveModel,
    e: &Entry,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let h = THUMB_W * 9.0 / 16.0;
    let selected = st.selected.as_deref() == Some(e.name.as_str());
    ui.vertical(|ui| {
        ui.set_width(THUMB_W);
        ui.spacing_mut().item_spacing = vec2(space::S2, space::S1);
        let (rect, resp) = ui.allocate_exact_size(vec2(THUMB_W, h), Sense::click());
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, true, selected, describe(e))
        });
        match st.thumbs.get(ui.ctx(), m.dir, &e.name) {
            Some(tex) => {
                egui::Image::new(tex)
                    .corner_radius(radius::PANEL)
                    .paint_at(ui, rect);
            }
            None => {
                ui.painter()
                    .rect_filled(rect, radius::PANEL, c(t.bg_raised));
            }
        }
        let (stroke, width) = if selected {
            (c(t.accent), size::FOCUS_RING)
        } else {
            (c(t.hairline), size::HAIRLINE)
        };
        ui.painter().rect_stroke(
            rect,
            radius::PANEL,
            Stroke::new(width, stroke),
            egui::StrokeKind::Inside,
        );
        if resp.has_focus() && !selected {
            ui.painter().rect_stroke(
                rect.expand(size::FOCUS_OFFSET),
                radius::PANEL + size::FOCUS_OFFSET,
                Stroke::new(size::FOCUS_RING, c(t.accent)),
                egui::StrokeKind::Outside,
            );
        }
        if resp.double_clicked() && !m.status.busy {
            st.select(&e.name);
            actions.push(UiAction::ApplyArchive(e.name.clone()));
        } else if resp.clicked() {
            st.select(&e.name);
        }
        ui.add_space(space::S1);
        ui.label(
            RichText::new(format!("{} {}", time_label(e), describe(e))).font(style::body_medium()),
        );
        let mut facts = format!("seed {}", e.seed);
        if e.pinned {
            facts.push_str("  pinned");
        }
        if m.current == Some(e.name.as_str()) {
            facts.push_str("  on screen");
        }
        ui.label(
            RichText::new(facts)
                .font(style::mono())
                .color(c(t.text_secondary)),
        );
    });
}

/// Everything you can do with the selected entry, in one place.
fn action_bar(
    ui: &mut Ui,
    theme: Theme,
    st: &mut ArchiveState,
    e: &Entry,
    m: &ArchiveModel,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{} {}, seed {}",
                time_label(e),
                describe(e),
                e.seed
            ))
            .font(style::body_strong()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_enabled_ui(!m.status.busy, |ui| {
                if primary_button(ui, theme, "Use as wallpaper").clicked() {
                    actions.push(UiAction::ApplyArchive(e.name.clone()));
                }
            });
        });
    });
    ui.add_space(space::S3);
    ui.horizontal(|ui| {
        if !st.confirm_delete {
            if ui
                .button(if e.pinned { "Unpin" } else { "Pin" })
                .on_hover_text("Pinned wallpapers are kept however old they get.")
                .clicked()
            {
                actions.push(UiAction::PinArchive {
                    name: e.name.clone(),
                    pinned: !e.pinned,
                });
            }
            if ui.button("Save to library").clicked() {
                actions.push(UiAction::SaveArchiveToLibrary(e.name.clone()));
            }
            if ui.button("Save PNG").clicked() {
                actions.push(UiAction::ExportArchive {
                    name: e.name.clone(),
                    gif: false,
                });
            }
            if ui.button("Copy PNG").clicked() {
                actions.push(UiAction::CopyArchivePng(e.name.clone()));
            }
            if ui.button("Save GIF").clicked() {
                actions.push(UiAction::ExportArchive {
                    name: e.name.clone(),
                    gif: true,
                });
            }
            if ui.button("Copy seed").clicked() {
                ui.ctx().copy_text(e.seed.to_string());
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if st.confirm_delete {
                let delete =
                    egui::Button::new(RichText::new("Delete for good").color(c(t.on_accent)))
                        .fill(c(t.danger))
                        .stroke(Stroke::NONE);
                if ui.add(delete).clicked() {
                    actions.push(UiAction::DeleteArchive(e.name.clone()));
                    st.confirm_delete = false;
                }
                if ui.button("Keep").clicked() {
                    st.confirm_delete = false;
                }
            } else if ui.button("Delete").clicked() {
                st.confirm_delete = true;
            }
        });
    });
    if let Some(note) = &m.status.note {
        ui.add_space(space::S2);
        ui.label(RichText::new(note).color(c(t.text_secondary)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(created: u64) -> Entry {
        Entry {
            name: "x".into(),
            created,
            seed: 7,
            theme: "dark".into(),
            archetype: "city_rooftops".into(),
            activity: Some("read_lantern".into()),
            pinned: false,
            bytes: 0,
        }
    }

    #[test]
    fn words_for_entries() {
        let e = entry(1_700_000_000);
        assert_eq!(describe(&e), "read lantern in city rooftops");
        let today = local(&e).map(|z| z.date());
        assert_eq!(day_label(&e, today), "Today");
        let tomorrow = today.and_then(|d| d.tomorrow().ok());
        assert_eq!(day_label(&e, tomorrow), "Yesterday");
        let later = today.and_then(|d| d.checked_add(jiff::ToSpan::days(5)).ok());
        assert!(
            day_label(&e, later).contains("November"),
            "{}",
            day_label(&e, later)
        );
        assert_eq!(time_label(&e).len(), 5);
        assert_eq!(megabytes(214.2), "214 MB");
        assert_eq!(megabytes(1024.0), "1 GB");
        assert_eq!(megabytes(1536.0), "1.5 GB");
    }

    #[test]
    fn archive_search_matches_scene_seed_theme_and_pinned() {
        let mut e = entry(1_700_000_000);
        assert!(matches_archive_filter(&e, ""));
        assert!(matches_archive_filter(&e, "rooftops"));
        assert!(matches_archive_filter(&e, "read lantern"));
        assert!(matches_archive_filter(&e, "dark"));
        assert!(matches_archive_filter(&e, "7"));
        assert!(!matches_archive_filter(&e, "pinned"));
        e.pinned = true;
        assert!(matches_archive_filter(&e, "pinned"));
        assert!(!matches_archive_filter(&e, "mountains"));
    }
}
