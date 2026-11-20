//! The Library window (PLAN.md section 9a, docs/design.md section 4): saved
//! wallpapers as a grid of thumbnails (the content is the hero; no card
//! frames), and one action bar for the selection.

use super::style::{self, c, colors};
use super::view::{Status, UiAction, hairline, primary_button, segmented};
use crate::library::{BuddyChoice, Item, ThemeChoice};
use crate::tokens::{radius, size, space};
use clairlune_scene::Theme;
use egui::{Align, Layout, RichText, Sense, Stroke, Ui, vec2};
use std::path::Path;

#[derive(Default)]
pub struct LibraryState {
    selected: Option<String>,
    /// The selected item's name while it's being edited.
    name_edit: String,
    confirm_delete: bool,
    thumbs: super::Thumbs,
    /// The moon mark for the empty state (shared with the window).
    pub moon: Option<egui::TextureHandle>,
}

impl LibraryState {
    /// Selects an item (UI test, and after saving one).
    pub fn select(&mut self, item: &Item) {
        self.selected = Some(item.id.clone());
        self.name_edit = item.name.clone();
        self.confirm_delete = false;
    }
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
                    ui.label(
                        RichText::new(match items.len() {
                            1 => "1 saved".to_string(),
                            n => format!("{n} saved"),
                        })
                        .font(style::mono())
                        .color(c(t.text_secondary)),
                    );
                });
            });
            ui.add_space(space::S2);
            hairline(ui, t.hairline);
            ui.add_space(space::S4);
            if items.is_empty() {
                empty(ui, theme, st.moon.as_ref());
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(space::S5, space::S5);
                    for item in items {
                        tile(ui, theme, st, library_dir, item);
                    }
                });
            });
        });
}

fn empty(ui: &mut Ui, theme: Theme, moon: Option<&egui::TextureHandle>) {
    let t = colors(theme);
    ui.add_space(space::S7);
    ui.vertical_centered(|ui| {
        if let Some(m) = moon {
            let tint = match theme {
                Theme::Dark => egui::Color32::WHITE,
                Theme::Light => c(t.text_secondary),
            };
            ui.add(
                egui::Image::new(m)
                    .fit_to_exact_size(vec2(64.0, 64.0))
                    .tint(tint),
            );
        }
        ui.add_space(space::S3);
        ui.label(RichText::new("Nothing saved yet").font(style::body_strong()));
        ui.label(
            RichText::new(
                "Keep a wallpaper you like with Save to library, in the tray menu or in Settings.",
            )
            .color(c(t.text_secondary)),
        );
    });
}

/// One thumbnail with its name and facts under it; click to select.
fn tile(ui: &mut Ui, theme: Theme, st: &mut LibraryState, dir: &Path, item: &Item) {
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
        if resp.clicked() {
            st.selected = Some(item.id.clone());
            st.name_edit = item.name.clone();
            st.confirm_delete = false;
        }
        ui.add_space(space::S1);
        ui.label(RichText::new(&item.name).font(style::body_strong()));
        let theme_fact = match item.theme {
            ThemeChoice::Follow => "",
            ThemeChoice::Light => "  light",
            ThemeChoice::Dark => "  dark",
        };
        ui.label(
            RichText::new(format!("seed {}{theme_fact}", item.seed))
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
    ui.horizontal(|ui| {
        let edit = ui.add(
            egui::TextEdit::singleline(&mut st.name_edit)
                .desired_width(260.0)
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
    });
    ui.add_space(space::S3);
    ui.horizontal(|ui| {
        if ui.button("Save PNG").clicked() {
            actions.push(UiAction::ExportLibrary {
                id: item.id.clone(),
                gif: false,
            });
        }
        if ui.button("Save GIF").clicked() {
            actions.push(UiAction::ExportLibrary {
                id: item.id.clone(),
                gif: true,
            });
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
