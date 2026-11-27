//! The selected slot's choices.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Saved,
    Pool,
    Random,
}

/// The selected slot's choices; help when nothing is selected.
pub(crate) fn inspector(
    ui: &mut Ui,
    look: &Look,
    st: &mut TimelineState,
    tl: &mut Timeline,
    actions: &mut Vec<UiAction>,
) {
    let theme = look.theme;
    let t = colors(theme);
    let Some((d, i)) = st.selected else {
        demo(ui, look, st);
        return;
    };
    let slot = tl.slots(d)[i].clone();
    ui.label(RichText::new(day_label(tl, d)).font(style::title()));
    ui.label(
        RichText::new(format!(
            "{} to {}",
            format_time(slot.start),
            until(tl, d, i)
        ))
        .font(style::mono())
        .color(c(t.text_secondary)),
    );
    ui.add_space(space::S2);
    hairline(ui, t.hairline);
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Start: typed, or nudged by a quarter of an hour.
        if st.start_for != Some((d, i, slot.start)) {
            st.start_text = format_time(slot.start);
            st.start_for = Some((d, i, slot.start));
        }
        group(ui, "Starts at");
        let mut moved = false;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut st.start_text)
                    // "00:00" in full, with the field's own margins.
                    .desired_width(76.0)
                    .font(FontId::new(text::BODY.size, FontFamily::Monospace)),
            );
            if field.lost_focus() {
                match timeline::parse_time(&st.start_text) {
                    Some(m) if m != slot.start => {
                        if let Some(j) = commit(st, tl, actions, |tl| tl.set_start(d, i, m)) {
                            st.selected = Some((d, j));
                            moved = true;
                        }
                    }
                    _ => {}
                }
                st.start_for = None;
            }
            if icon_btn(ui, theme, Glyph::Earlier, "15 minutes earlier", true, false).clicked() {
                commit(st, tl, actions, |tl| {
                    tl.move_start(d, i, slot.start.saturating_sub(15))
                });
                moved = true;
            }
            if icon_btn(ui, theme, Glyph::Later, "15 minutes later", true, false).clicked() {
                commit(st, tl, actions, |tl| tl.move_start(d, i, slot.start + 15));
                moved = true;
            }
        });
        if moved {
            return;
        }

        let mut s = slot.clone();
        group(ui, "Shows");
        let mut kind = match &s.content {
            Content::Item { .. } => Kind::Saved,
            Content::Pool { .. } => Kind::Pool,
            Content::Random { .. } => Kind::Random,
        };
        let have = !look.items.is_empty();
        let switched = ui
            .horizontal(|ui| {
                segmented(
                    ui,
                    theme,
                    &mut kind,
                    &[
                        (Kind::Saved, "Saved", have),
                        (Kind::Pool, "Pool", have),
                        (Kind::Random, "Random", true),
                    ],
                )
            })
            .inner;
        if switched {
            let current = match &s.content {
                Content::Item { id } => Some(id.clone()),
                Content::Pool { ids, .. } => ids.first().cloned(),
                Content::Random { .. } => None,
            };
            s.content = match kind {
                Kind::Saved => Content::Item {
                    id: current.unwrap_or_else(|| look.items[0].id.clone()),
                },
                Kind::Pool => Content::Pool {
                    ids: current.into_iter().collect(),
                    tag: None,
                    order: Order::Shuffle,
                },
                Kind::Random => Slot::random(0).content,
            };
        }
        ui.add_space(space::S2);
        match &mut s.content {
            Content::Item { id } => {
                let name = describe(&Content::Item { id: id.clone() }, look.items);
                egui::ComboBox::from_id_salt(("tl-item", d, i))
                    .width(ui.available_width())
                    .selected_text(name)
                    .show_ui(ui, |ui| {
                        for item in look.items {
                            ui.selectable_value(id, item.id.clone(), &item.name);
                        }
                    });
                ui.add_space(space::S2);
                let w = ui.available_width();
                if let Some(tex) = st.thumbs.get(ui.ctx(), look.dir, id) {
                    ui.add(
                        egui::Image::new(tex)
                            .fit_to_exact_size(vec2(w, w * 9.0 / 16.0))
                            .corner_radius(radius::CONTROL),
                    );
                }
            }
            Content::Pool { ids, tag, order } => {
                for item in look.items {
                    let mut on = ids.contains(&item.id);
                    if ui.checkbox(&mut on, &item.name).changed() {
                        if on {
                            ids.push(item.id.clone());
                        } else {
                            ids.retain(|x| x != &item.id);
                        }
                    }
                }
                ui.add_space(space::S2);
                if st.tag_for != Some((d, i)) {
                    st.tag_text = tag.clone().unwrap_or_default();
                    st.tag_for = Some((d, i));
                }
                let field = ui.add(
                    egui::TextEdit::singleline(&mut st.tag_text)
                        .hint_text("tag, e.g. evening")
                        .desired_width(ui.available_width()),
                );
                if field.lost_focus() {
                    let typed = st.tag_text.trim();
                    *tag = (!typed.is_empty()).then(|| typed.to_string());
                }
                ui.add_space(space::S2);
                ui.horizontal(|ui| {
                    segmented(
                        ui,
                        theme,
                        order,
                        &[
                            (Order::Shuffle, "Shuffle", true),
                            (Order::Sequential, "In order", true),
                        ],
                    );
                });
            }
            Content::Random {
                archetypes,
                activities,
                weather,
            } => {
                group(ui, "Landscapes");
                filter_checks(ui, archetypes, &view::LANDSCAPES, &[]);
                group(ui, "Activities");
                filter_checks(ui, activities, &view::ACTIVITIES, &[]);
                group(ui, "Weather");
                filter_checks(ui, weather, &view::WEATHER, &view::CLEAR_WEATHER);
            }
        }

        let selected_item = match &s.content {
            Content::Item { id } => look.items.iter().find(|it| &it.id == id),
            _ => None,
        };
        let is_photo_item = selected_item
            .is_some_and(|it| it.archetype == clairlune_render::img2ascii::PHOTO_ARCHETYPE);
        let (has_light, has_dark, has_live) = match selected_item {
            Some(it) => {
                let item_dir = look.dir.join(&it.id);
                let scene_dir = item_dir.join("scene").join("0");
                let hl = !scene_dir.exists() || scene_dir.join("background-light.png").exists();
                let hd = !scene_dir.exists() || scene_dir.join("background-dark.png").exists();
                let hlive = crate::apply::read_photo_meta(&item_dir)
                    .is_none_or(|m| m.config.has_live_weather());
                (hl, hd, hlive)
            }
            None => (true, true, true),
        };

        group(ui, "Theme");
        ui.horizontal(|ui| {
            segmented(
                ui,
                theme,
                &mut s.theme,
                &[
                    (ThemeChoice::Follow, "Follow", has_light && has_dark),
                    (ThemeChoice::Light, "Light", has_light),
                    (ThemeChoice::Dark, "Dark", has_dark),
                ],
            )
        });
        if !is_photo_item {
            group(ui, "Clawd");
            ui.horizontal(|ui| {
                segmented(
                    ui,
                    theme,
                    &mut s.buddy,
                    &[
                        (BuddyChoice::Follow, "Follow", true),
                        (BuddyChoice::On, "On", true),
                        (BuddyChoice::Off, "Off", true),
                    ],
                )
            });
        }
        group(ui, "Display");
        ui.horizontal(|ui| {
            segmented(
                ui,
                theme,
                &mut s.display,
                &[
                    (DisplayChoice::Follow, "Follow", true),
                    (DisplayChoice::Still, "Still", true),
                    (DisplayChoice::Live, "Live", has_live),
                ],
            )
        });
        if s != slot {
            commit(st, tl, actions, |tl| tl.slots_mut(d)[i] = s);
        }
        ui.add_space(space::S5);
        if ui.button("Delete").clicked() {
            commit(st, tl, actions, |tl| tl.remove(d, i));
            st.selected = None;
        }
    });
}
