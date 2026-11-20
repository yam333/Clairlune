//! Landscape coverage (M4): every implemented archetype hosts several of the
//! shipped activities, builds in both themes at landscape and portrait sizes
//! with Clawd standing on its platform, and gets a fair share of seeds.

use clairlune_scene::archetype::ARCHETYPE_INFO;
use clairlune_scene::glyphs::EMPTY;
use clairlune_scene::{
    Aspect, BuildError, BuildOptions, ContentContext, SpriteKind, Theme, build, compose,
};
use clairlune_tests::recipe;
use std::collections::{BTreeMap, BTreeSet};

fn implemented() -> impl Iterator<Item = &'static str> {
    ARCHETYPE_INFO
        .iter()
        .filter(|a| a.implemented)
        .map(|a| a.id)
}

#[test]
fn every_landscape_hosts_activities_in_both_themes() {
    for id in implemented() {
        let ctx = ContentContext {
            enabled_archetypes: vec![id.to_string()],
            ..ContentContext::default()
        };
        let mut activities = BTreeSet::new();
        let mut themes = BTreeSet::new();
        for seed in 0..24 {
            let r = compose(seed, Aspect::from_size(16, 9), &ctx);
            assert_eq!(r.archetype, id, "seed {seed}");
            let activity = r
                .activity
                .clone()
                .unwrap_or_else(|| panic!("{id}: seed {seed} has no activity"));
            activities.insert(activity);
            for theme in [Theme::Dark, Theme::Light] {
                for (w, h) in [(1920, 1080), (1080, 1920)] {
                    let opts = BuildOptions {
                        width: w,
                        height: h,
                        theme,
                        buddy: true,
                    };
                    let g = match build(&r, &opts) {
                        Ok(g) => g,
                        Err(BuildError::ThemeLocked(_)) => continue,
                        Err(e) => panic!("{id}: seed {seed} {w}x{h} {theme:?}: {e}"),
                    };
                    themes.insert(theme.as_str());
                    if g.character.is_some_and(|t| t.flies()) {
                        continue; // in the air on purpose
                    }
                    // The cell under the character's feet is platform, not air.
                    let c = g
                        .sprites_at(g.t_hero_ms)
                        .into_iter()
                        .find(|s| s.kind == SpriteKind::Character)
                        .expect("a character");
                    if let Some(j) = &g.journey {
                        // The walking layer's strip, where it is under the
                        // feet now: its ground line, on the feet's row or
                        // just above (a line near a cell's top is drawn as
                        // `_` in the row above).
                        let walk = j.layers.last().expect("a walking layer");
                        let off = j.offset_px(j.layers.len() - 1, g.grid.cell_w, g.t_hero_ms);
                        // Across the middle of the feet, where the ground's
                        // highest point is taken.
                        let x = (j.feet_x - g.grid.origin_x as i32) as u32 + off;
                        let half = c.width() * 3 / 10;
                        let feet = (c.y + c.height() as i32) as u32;
                        let row = (feet - g.grid.origin_y) / g.grid.cell_h;
                        let ground = (x - half..=x + half).any(|px| {
                            let scol = (px / g.grid.cell_w) % walk.cols;
                            (row.saturating_sub(1)..=row)
                                .any(|r| walk.cells[(r * walk.cols + scol) as usize].glyph != EMPTY)
                        });
                        assert!(
                            ground,
                            "{id}: seed {seed} {w}x{h} {theme:?}: journey feet in the air"
                        );
                        continue;
                    }
                    let col =
                        ((c.x + c.width() as i32 / 2) as u32 - g.grid.origin_x) / g.grid.cell_w;
                    let row = (g.ground_y as u32 - g.grid.origin_y) / g.grid.cell_h;
                    assert_ne!(
                        g.cell(col, row).glyph,
                        EMPTY,
                        "{id}: seed {seed} {w}x{h} {theme:?}: nothing under the feet"
                    );
                }
            }
        }
        assert!(activities.len() >= 3, "{id} hosts only {activities:?}");
        assert_eq!(themes.len(), 2, "{id} renders only in {themes:?}");
    }
}

/// With default settings every landscape shows up, and none crowds out the rest.
#[test]
fn no_landscape_dominates() {
    let n = 1000;
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for seed in 0..n {
        *counts
            .entry(recipe(seed, 1920, 1080).archetype)
            .or_default() += 1;
    }
    let ids: Vec<&str> = implemented().collect();
    let expected = n as f64 / ids.len() as f64;
    for id in ids {
        let c = f64::from(counts.get(id).copied().unwrap_or(0));
        assert!(
            c >= 0.4 * expected && c <= 2.5 * expected,
            "{id} appears {c} times, expected about {expected}: {counts:?}"
        );
    }
}
