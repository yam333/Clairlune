//! The Library window (docs/dev/design.md section 4): saved
//! wallpapers as a grid of thumbnails (the content is the hero; no card
//! frames), and one action bar for the selection.

use super::style::{self, c, colors};
use super::view::{Status, UiAction, empty_page, hairline, primary_button, segmented};
use crate::library::{BuddyChoice, DisplayChoice, Item, ThemeChoice};
use crate::tokens::{radius, size, space};
use clairlune_scene::Theme;
use egui::{Align, Layout, RichText, Sense, Stroke, Ui, vec2};
use std::path::Path;

#[derive(Default)]
pub struct LibraryState {
    selected: Option<String>,
    /// The selected item's name and tags while they're being edited.
    name_edit: String,
    tags_edit: String,
    tags_for: Option<(String, Vec<String>)>,
    query: String,
    tag_filter: Option<String>,
    confirm_delete: bool,
    thumbs: super::Thumbs,
}

impl LibraryState {
    /// Selects an item (UI test, and after saving one).
    pub fn select(&mut self, item: &Item) {
        self.selected = Some(item.id.clone());
        self.name_edit = item.name.clone();
        self.tags_edit = format_tags(&item.tags);
        self.tags_for = Some((item.id.clone(), item.tags.clone()));
        self.confirm_delete = false;
    }
}

fn format_tags(tags: &[String]) -> String {
    tags.join(", ")
}

fn parse_tags(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split(',') {
        let tag = part.trim();
        if !tag.is_empty() && !out.iter().any(|t| t == tag) {
            out.push(tag.to_string());
        }
    }
    out
}

fn all_tags(items: &[Item]) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for item in items {
        for tag in &item.tags {
            if !tags.iter().any(|t| t == tag) {
                tags.push(tag.clone());
            }
        }
    }
    tags.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
    tags
}

fn matches_library_filter(item: &Item, query: &str, tag_filter: Option<&str>) -> bool {
    if let Some(tag) = tag_filter
        && !item.tags.iter().any(|t| t == tag)
    {
        return false;
    }
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    item.name.to_lowercase().contains(&q)
        || item.archetype.replace('_', " ").to_lowercase().contains(&q)
        || item
            .activity
            .as_deref()
            .is_some_and(|a| a.replace('_', " ").to_lowercase().contains(&q))
        || item.tags.iter().any(|t| t.to_lowercase().contains(&q))
        || item.seed.to_string().contains(&q)
}

const THUMB_W: f32 = 192.0;

fn thumb<'a>(
    ui: &Ui,
    st: &'a mut LibraryState,
    dir: &Path,
    id: &str,
) -> Option<&'a egui::TextureHandle> {
    st.thumbs.get(ui.ctx(), dir, id)
}

pub fn show(
    ui: &mut Ui,
    theme: Theme,
    st: &mut LibraryState,
    model: &super::Model,
    actions: &mut Vec<UiAction>,
) {
    let (items, library_dir, uses, status) =
        (model.library, model.library_dir, model.uses, model.status);
    let t = colors(theme);
    // Forget a selection whose item is gone.
    if let Some(sel) = &st.selected
        && !items.iter().any(|i| &i.id == sel)
    {
        st.selected = None;
    }
    let selected = st
        .selected
        .as_ref()
        .and_then(|id| items.iter().find(|i| &i.id == id))
        .cloned();
    if let Some(item) = &selected {
        egui::Panel::bottom("selection")
            .frame(
                egui::Frame::new()
                    .fill(c(t.bg_sidebar))
                    .inner_margin(egui::Margin::symmetric(space::S6 as i8, space::S4 as i8)),
            )
            .show(ui, |ui| {
                let used = uses.get(&item.id).map(Vec::as_slice).unwrap_or(&[]);
                action_bar(ui, theme, st, item, used, status, actions)
            });
    }
    let tags = all_tags(items);
    if let Some(active) = &st.tag_filter
        && !tags.iter().any(|t| t == active)
    {
        st.tag_filter = None;
    }
    let filtering = !st.query.trim().is_empty() || st.tag_filter.is_some();
    let filtered: Vec<&Item> = items
        .iter()
        .filter(|i| matches_library_filter(i, &st.query, st.tag_filter.as_deref()))
        .collect();
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(c(t.bg))
                .inner_margin(egui::Margin::symmetric(space::S6 as i8, space::S5 as i8)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Library").font(style::title()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Image to ASCII\u{2026}").clicked() {
                        actions.push(UiAction::OpenAscii);
                    }
                    ui.add_space(space::S3);
                    let count = if filtering {
                        format!("{} of {} saved", filtered.len(), items.len())
                    } else {
                        match items.len() {
                            1 => "1 saved".to_string(),
                            n => format!("{n} saved"),
                        }
                    };
                    ui.label(
                        RichText::new(count)
                            .font(style::mono())
                            .color(c(t.text_secondary)),
                    );
                    if !items.is_empty() {
                        ui.add_space(space::S3);
                        if filtering && ui.button("Clear").clicked() {
                            st.query.clear();
                            st.tag_filter = None;
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut st.query)
                                .desired_width(200.0)
                                .hint_text("Search name, tag or seed"),
                        );
                    }
                });
            });
            ui.add_space(space::S2);
            hairline(ui, t.hairline);
            if !tags.is_empty() {
                ui.add_space(space::S3);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(space::S2, space::S2);
                    for tag in &tags {
                        let active = st.tag_filter.as_deref() == Some(tag.as_str());
                        if ui.add(egui::Button::selectable(active, tag)).clicked() {
                            st.tag_filter = if active { None } else { Some(tag.clone()) };
                        }
                    }
                });
            }
            ui.add_space(space::S4);
            if items.is_empty() {
                empty_page(
                    ui,
                    theme,
                    "Nothing saved yet",
                    "Keep a wallpaper you like with Save to library, in the tray menu or in Settings.",
                );
                return;
            }
            if filtered.is_empty() {
                empty_page(
                    ui,
                    theme,
                    "No matching wallpapers",
                    "Try another search or clear the filter.",
                );
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(space::S5, space::S5);
                    for item in filtered {
                        tile(ui, theme, st, library_dir, item, status.busy, actions);
                    }
                });
            });
        });
}

/// One thumbnail with its name and facts under it; click to select, double-click
/// to apply.
fn tile(
    ui: &mut Ui,
    theme: Theme,
    st: &mut LibraryState,
    dir: &Path,
    item: &Item,
    busy: bool,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    let h = THUMB_W * 9.0 / 16.0;
    let selected = st.selected.as_deref() == Some(item.id.as_str());
    ui.vertical(|ui| {
        ui.set_width(THUMB_W);
        // The grid's wide gaps are between tiles, not inside them.
        ui.spacing_mut().item_spacing = vec2(space::S2, space::S1);
        let (rect, resp) = ui.allocate_exact_size(vec2(THUMB_W, h), Sense::click());
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, true, selected, &item.name)
        });
        match thumb(ui, st, dir, &item.id) {
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
        if resp.double_clicked() && !busy {
            st.select(item);
            actions.push(UiAction::ApplyLibrary(item.id.clone()));
        } else if resp.clicked() {
            st.select(item);
        }
        ui.add_space(space::S1);
        ui.label(RichText::new(&item.name).font(style::body_strong()));
        let theme_fact = match item.theme {
            ThemeChoice::Follow => "",
            ThemeChoice::Light => "  light",
            ThemeChoice::Dark => "  dark",
        };
        let display_fact = match item.display {
            DisplayChoice::Follow => "",
            DisplayChoice::Live => "  live",
            DisplayChoice::Still => "  still",
        };
        ui.label(
            RichText::new(format!("seed {}{theme_fact}{display_fact}", item.seed))
                .font(style::mono())
                .color(c(t.text_secondary)),
        );
    });
}

/// Everything you can do with the selected item, in one place.
fn action_bar(
    ui: &mut Ui,
    theme: Theme,
    st: &mut LibraryState,
    item: &Item,
    used: &[String],
    status: &Status,
    actions: &mut Vec<UiAction>,
) {
    let t = colors(theme);
    if st
        .tags_for
        .as_ref()
        .is_none_or(|(id, tags)| id != &item.id || tags != &item.tags)
    {
        st.tags_edit = format_tags(&item.tags);
        st.tags_for = Some((item.id.clone(), item.tags.clone()));
    }
    ui.horizontal(|ui| {
        let edit = ui.add(
            egui::TextEdit::singleline(&mut st.name_edit)
                .desired_width(240.0)
                .hint_text("Name"),
        );
        if edit.lost_focus() {
            let name = st.name_edit.trim();
            if !name.is_empty() && name != item.name {
                let mut renamed = item.clone();
                renamed.name = name.to_string();
                actions.push(UiAction::UpdateLibrary(Box::new(renamed)));
            }
        }
        let tags = ui.add(
            egui::TextEdit::singleline(&mut st.tags_edit)
                .desired_width(200.0)
                .hint_text("Tags, e.g. evening"),
        );
        if tags.lost_focus() {
            let parsed = parse_tags(&st.tags_edit);
            st.tags_edit = format_tags(&parsed);
            if parsed != item.tags {
                let mut tagged = item.clone();
                tagged.tags = parsed.clone();
                st.tags_for = Some((item.id.clone(), parsed));
                actions.push(UiAction::UpdateLibrary(Box::new(tagged)));
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_enabled_ui(!status.busy, |ui| {
                if primary_button(ui, theme, "Use as wallpaper").clicked() {
                    actions.push(UiAction::ApplyLibrary(item.id.clone()));
                }
            });
        });
    });
    ui.add_space(space::S3);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Theme").color(c(t.text_secondary)));
        let mut theme_choice = item.theme;
        if segmented(
            ui,
            theme,
            &mut theme_choice,
            &[
                (ThemeChoice::Follow, "Follow", true),
                (ThemeChoice::Light, "Light", true),
                (ThemeChoice::Dark, "Dark", true),
            ],
        ) {
            let mut changed = item.clone();
            changed.theme = theme_choice;
            actions.push(UiAction::UpdateLibrary(Box::new(changed)));
        }
        ui.add_space(space::S5);
        ui.label(RichText::new("Clawd").color(c(t.text_secondary)));
        let mut buddy = item.buddy;
        if segmented(
            ui,
            theme,
            &mut buddy,
            &[
                (BuddyChoice::Follow, "Follow", true),
                (BuddyChoice::On, "On", true),
                (BuddyChoice::Off, "Off", true),
            ],
        ) {
            let mut changed = item.clone();
            changed.buddy = buddy;
            actions.push(UiAction::UpdateLibrary(Box::new(changed)));
        }
        ui.add_space(space::S5);
        ui.label(RichText::new("Display").color(c(t.text_secondary)));
        let mut display = item.display;
        if segmented(
            ui,
            theme,
            &mut display,
            &[
                (DisplayChoice::Follow, "Follow", true),
                (DisplayChoice::Still, "Still", true),
                (DisplayChoice::Live, "Live", true),
            ],
        ) {
            let mut changed = item.clone();
            changed.display = display;
            actions.push(UiAction::UpdateLibrary(Box::new(changed)));
        }
    });
    ui.add_space(space::S3);
    ui.horizontal(|ui| {
        if !st.confirm_delete {
            if ui.button("Save PNG").clicked() {
                actions.push(UiAction::ExportLibrary {
                    id: item.id.clone(),
                    gif: false,
                });
            }
            if ui.button("Copy PNG").clicked() {
                actions.push(UiAction::CopyLibraryPng(item.id.clone()));
            }
            if ui.button("Save GIF").clicked() {
                actions.push(UiAction::ExportLibrary {
                    id: item.id.clone(),
                    gif: true,
                });
            }
            if ui.button("Copy seed").clicked() {
                ui.ctx().copy_text(item.seed.to_string());
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if st.confirm_delete {
                let danger = |label: &str| {
                    egui::Button::new(RichText::new(label).color(c(t.on_accent)))
                        .fill(c(t.danger))
                        .stroke(Stroke::NONE)
                };
                let mut delete = |forget| {
                    actions.push(UiAction::DeleteLibrary {
                        id: item.id.clone(),
                        forget,
                    });
                };
                if used.is_empty() {
                    if ui.add(danger("Delete for good")).clicked() {
                        delete(crate::timeline::Forget::RemoveSlots);
                        st.confirm_delete = false;
                    }
                } else {
                    // Never leave the timeline pointing at nothing.
                    if ui.add(danger("Delete, remove those slots")).clicked() {
                        delete(crate::timeline::Forget::RemoveSlots);
                        st.confirm_delete = false;
                    }
                    if ui.add(danger("Delete, use random there")).clicked() {
                        delete(crate::timeline::Forget::UseRandom);
                        st.confirm_delete = false;
                    }
                }
                if ui.button("Keep").clicked() {
                    st.confirm_delete = false;
                }
            } else if ui.button("Delete").clicked() {
                st.confirm_delete = true;
            }
        });
    });
    if st.confirm_delete && !used.is_empty() {
        ui.add_space(space::S2);
        ui.label(
            RichText::new(format!("The timeline uses it: {}.", used.join(", "))).color(c(t.danger)),
        );
    }
    if let Some(note) = &status.note {
        ui.add_space(space::S2);
        ui.label(RichText::new(note).color(c(t.text_secondary)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_parse_and_round_trip_without_duplicates_or_blanks() {
        assert_eq!(
            parse_tags(" evening , calm, evening, , late night "),
            vec!["evening", "calm", "late night"]
        );
        assert!(parse_tags(" ,  ").is_empty());
        assert_eq!(
            format_tags(&["evening".into(), "calm".into()]),
            "evening, calm"
        );
    }

    #[test]
    fn library_search_and_tag_filters_match_name_scene_tags_and_seed() {
        let item = Item {
            id: "lib_1".into(),
            name: "Quiet harbour".into(),
            created: 1,
            tags: vec!["evening".into(), "calm".into()],
            display: DisplayChoice::Follow,
            theme: ThemeChoice::Follow,
            buddy: BuddyChoice::Follow,
            seed: 4242,
            archetype: "lantern_lake".into(),
            activity: Some("read_lantern".into()),
            generator_version: 4,
        };
        assert!(matches_library_filter(&item, "", None));
        assert!(matches_library_filter(&item, "harbour", None));
        assert!(matches_library_filter(&item, "lantern lake", None));
        assert!(matches_library_filter(&item, "read lantern", None));
        assert!(matches_library_filter(&item, "CALM", None));
        assert!(matches_library_filter(&item, "4242", None));
        assert!(!matches_library_filter(&item, "snowfield", None));
        assert!(matches_library_filter(&item, "", Some("evening")));
        assert!(!matches_library_filter(&item, "", Some("morning")));
        assert_eq!(all_tags(&[item]), vec!["calm", "evening"]);
    }
}
