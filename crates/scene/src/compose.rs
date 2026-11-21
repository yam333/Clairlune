//! `compose(seed, aspect, ctx) -> SceneRecipe`: sequential, seeded, weighted picks.
//!
//! The activity is picked first (PLAN.md section 4.2, step 0) and constrains
//! every later pick: archetype, time of day per theme, weather and props. Each
//! subsystem draws from its own RNG stream, so adding a pick in one place never
//! shifts the draws of another. When no activity is eligible, the scene falls back
//! to an archetype-first composition with no character.

use crate::activity::{Activity, Catalog, MotionKind, builtin_catalog, int_weight};
use crate::archetype::{ARCHETYPE_INFO, ArchetypeInfo};
use crate::palette::{Theme, families_for};
use crate::recipe::{
    Aspect, CONTRASTS, Composition, ContentContext, GENERATOR_VERSION, LOOKS, PalettePick,
    SceneRecipe, TimeOfDay,
};
use crate::rng::Rng;

/// Builds a recipe from the built-in activity catalog.
pub fn compose(seed: u64, aspect: Aspect, ctx: &ContentContext) -> SceneRecipe {
    compose_with(builtin_catalog(), seed, aspect, ctx)
}

/// Builds a recipe. Pure: same inputs, same recipe, on every platform.
pub fn compose_with(
    catalog: &Catalog,
    seed: u64,
    aspect: Aspect,
    ctx: &ContentContext,
) -> SceneRecipe {
    let archetypes: Vec<&'static ArchetypeInfo> = ARCHETYPE_INFO
        .iter()
        .filter(|a| a.implemented && ctx.enabled_archetypes.iter().any(|e| e == a.id))
        .collect();

    // Step 0: activity, among those with at least one archetype that fits.
    let eligible: Vec<(&Activity, Vec<&'static ArchetypeInfo>)> = catalog
        .activities
        .iter()
        .filter(|a| ctx.enabled_activities.contains(&a.id))
        // Shown in a known theme: only activities that happen in it.
        .filter(|a| match ctx.theme {
            Some(Theme::Light) => !a.times_in(&TimeOfDay::LIGHT).is_empty(),
            Some(Theme::Dark) => !a.times_in(&TimeOfDay::DARK).is_empty(),
            None => true,
        })
        .filter_map(|a| {
            let fits: Vec<_> = archetypes
                .iter()
                .copied()
                .filter(|ar| a.fits(ar, &ctx.enabled_weather))
                .collect();
            (!fits.is_empty()).then_some((a, fits))
        })
        .collect();
    tracing::debug!(
        target: "clairlune_scene::compose",
        seed,
        candidates = ?eligible.iter().map(|(a, f)| (a.id.as_str(), f.len())).collect::<Vec<_>>(),
        "activity candidates (id, fitting archetypes)"
    );
    let picked = pick_activity(seed, &eligible, ctx);
    if picked.is_none() {
        tracing::warn!(
            target: "clairlune_scene::compose",
            seed,
            "no eligible activity; archetype-first fallback without a character"
        );
    }

    // Step 1: archetype.
    let fallback = [&ARCHETYPE_INFO[0]];
    let candidates: &[&ArchetypeInfo] = match picked {
        Some((_, fits)) => fits,
        None if !archetypes.is_empty() => &archetypes,
        None => &fallback,
    };
    let archetype = pick_archetype(seed, candidates, &eligible, ctx);

    // Step 2: one time of day per theme, from what the activity allows.
    let mut tod = Rng::stream(seed, "time_of_day");
    let (light_tod, dark_tod) = match picked {
        Some((a, _)) => (
            pick_opt(&mut tod, &a.times_in(&TimeOfDay::LIGHT)),
            pick_opt(&mut tod, &a.times_in(&TimeOfDay::DARK)),
        ),
        None => (
            Some(*tod.choose(&TimeOfDay::LIGHT)),
            Some(*tod.choose(&TimeOfDay::DARK)),
        ),
    };

    // Step 3: weather the activity, archetype and user all allow.
    let mut wx = Rng::stream(seed, "weather");
    let weather = match picked {
        Some((a, _)) => {
            pick_opt(&mut wx, &a.weathers_in(archetype, &ctx.enabled_weather)).unwrap_or("none")
        }
        None => "none",
    };

    let mut platform_rng = Rng::stream(seed, "platform");
    let platform = *platform_rng.choose(archetype.platforms);

    // Composition variables.
    let mut comp = Rng::stream(seed, "composition");
    let portrait = aspect.w < aspect.h;
    let horizon_pm = comp.range_u32(560, 700);
    // Rule-of-thirds lines are favoured over the centre. Portrait stays nearer the middle.
    let lines: [u32; 3] = if portrait {
        [420, 500, 580]
    } else {
        [333, 500, 667]
    };
    let focus_base = lines[comp.weighted_index(&[2, 1, 2])];
    let focus_x_pm = (focus_base as i32 + comp.range_i32(-30, 30)) as u32;
    let gap_pm = comp.range_u32(200, 340);
    // Below 1 the wall profile is convex: steep cliff faces, then a plateau.
    let shape_pow_pm = comp.range_u32(450, 850);
    let platform_margin_pm = comp.range_u32(12, 40);

    // Props and motion, from the activity.
    let mut props_rng = Rng::stream(seed, "props");
    let prop_left = props_rng.chance(1, 2);
    let mut motion = Rng::stream(seed, "motion");
    let (motion_speed_cc, pace_half_pm, fly_lift_pm) =
        match picked.map(|(a, _)| (&a.motion.kind, a.motion.speed_cells_per_s)) {
            Some((MotionKind::Pace, Some([lo, hi]))) => {
                let (lo, hi) = ((lo * 100.0).round() as u32, (hi * 100.0).round() as u32);
                (
                    motion.range_u32(lo, hi.max(lo)),
                    motion.range_u32(60, 140),
                    0,
                )
            }
            Some((MotionKind::Bob, Some([lo, hi]))) => {
                let (lo, hi) = ((lo * 100.0).round() as u32, (hi * 100.0).round() as u32);
                (motion.range_u32(lo, hi.max(lo)), 0, 0)
            }
            Some((MotionKind::Fly, Some([lo, hi]))) => {
                let (lo, hi) = ((lo * 100.0).round() as u32, (hi * 100.0).round() as u32);
                (
                    motion.range_u32(lo, hi.max(lo)),
                    0,
                    motion.range_u32(170, 270),
                )
            }
            _ => (0, 0, 0),
        };
    // A journey's pace: the ground underfoot goes by at about the walk's
    // own speed; the loop length follows from it at build time.
    let journey_pace_c = match picked.map(|(a, _)| (&a.motion.kind, a.motion.speed_cells_per_s)) {
        Some((MotionKind::Journey, Some([lo, hi]))) => {
            let (lo, hi) = ((lo * 100.0).round() as u32, (hi * 100.0).round() as u32);
            motion.range_u32(lo, hi.max(lo))
        }
        _ => 0,
    };

    // How it is drawn, from the looks allowed; the contrast, fixed or varied.
    let mut look_rng = Rng::stream(seed, "look");
    let looks: Vec<&str> = LOOKS
        .iter()
        .copied()
        .filter(|l| ctx.enabled_looks.iter().any(|e| e == l))
        .collect();
    let look = if looks.is_empty() {
        "line"
    } else {
        *look_rng.choose(&looks)
    };
    let contrast = match ctx.contrast.as_str() {
        "vary" => *look_rng.choose(&CONTRASTS),
        c if CONTRASTS.contains(&c) => c,
        _ => "normal",
    };

    let mut pal = Rng::stream(seed, "palette");
    let light: Vec<_> = families_for(Theme::Light).collect();
    let dark: Vec<_> = families_for(Theme::Dark).collect();
    let light_family = pal.choose(&light).id.to_string();
    let dark_family = pal.choose(&dark).id.to_string();

    let recipe = SceneRecipe {
        generator_version: GENERATOR_VERSION,
        seed,
        aspect,
        content_ctx_hash: format!("{:016x}", ctx.hash()),
        activity: picked.map(|(a, _)| a.id.clone()),
        archetype: archetype.id.to_string(),
        light_tod,
        dark_tod,
        platform: platform.to_string(),
        props: picked
            .map(|(a, _)| a.props.keys().cloned().collect())
            .unwrap_or_default(),
        weather: weather.to_string(),
        composition: Composition {
            horizon_pm,
            focus_x_pm,
            gap_pm,
            shape_pow_pm,
            platform_margin_pm,
            cliff_layers: 3,
            prop_left,
            motion_speed_cc,
            pace_half_pm,
            fly_lift_pm,
            journey_ms: 0,
            journey_pace_c,
        },
        palette: PalettePick {
            light: light_family,
            dark: dark_family,
        },
        look: look.to_string(),
        contrast: contrast.to_string(),
    };
    log_composed(&recipe, eligible.len());
    recipe
}

/// One `info` line per composed scene, with everything needed to reproduce it.
fn log_composed(r: &SceneRecipe, eligible: usize) {
    if tracing::enabled!(target: "clairlune_scene::compose", tracing::Level::INFO) {
        tracing::info!(
            target: "clairlune_scene::compose",
            seed = r.seed,
            recipe_hash = %r.hash_hex(),
            generator_version = r.generator_version,
            activity = r.activity.as_deref().unwrap_or("-"),
            archetype = %r.archetype,
            light_tod = ?r.light_tod,
            dark_tod = ?r.dark_tod,
            eligible,
            "scene composed"
        );
    }
}

fn pick_opt<T: Copy>(rng: &mut Rng, items: &[T]) -> Option<T> {
    (!items.is_empty()).then(|| *rng.choose(items))
}

/// Weighted pick. The most recent activity is excluded unless it is the only one,
/// and activities used often in the recent history are down-weighted.
fn pick_activity<'a, 'b>(
    seed: u64,
    eligible: &'b [(&'a Activity, Vec<&'static ArchetypeInfo>)],
    ctx: &ContentContext,
) -> Option<&'b (&'a Activity, Vec<&'static ArchetypeInfo>)> {
    if eligible.is_empty() {
        return None;
    }
    let last = ctx.recent.last().and_then(|h| h.activity.as_deref());
    let weights: Vec<u32> = eligible
        .iter()
        .map(|(a, _)| {
            if eligible.len() > 1 && Some(a.id.as_str()) == last {
                return 0;
            }
            let uses = ctx
                .recent
                .iter()
                .filter(|h| h.activity.as_deref() == Some(a.id.as_str()))
                .count() as u32;
            (int_weight(a.weight).saturating_mul(4) / (4 + uses)).max(1)
        })
        .collect();
    let mut rng = Rng::stream(seed, "activity");
    Some(&eligible[rng.weighted_index(&weights)])
}

/// Weighted pick with the most recent archetype down-weighted. Each
/// landscape is also weighted against how many of the eligible activities
/// (by their weight) it hosts, so one that hosts few, like the interior, is
/// not crowded out by those that host nearly everything.
fn pick_archetype(
    seed: u64,
    candidates: &[&'static ArchetypeInfo],
    eligible: &[(&Activity, Vec<&'static ArchetypeInfo>)],
    ctx: &ContentContext,
) -> &'static ArchetypeInfo {
    let last = ctx.recent.last().map(|h| h.archetype.as_str());
    let hosted = |id: &str| -> u64 {
        eligible
            .iter()
            .filter(|(_, fits)| fits.iter().any(|f| f.id == id))
            .map(|(a, _)| u64::from(int_weight(a.weight)))
            .sum::<u64>()
            .max(1)
    };
    let weights: Vec<u32> = candidates
        .iter()
        .map(|a| {
            let base: u64 = if candidates.len() > 1 && Some(a.id) == last {
                1
            } else {
                4
            };
            (base * 1_000_000 / hosted(a.id)).max(1) as u32
        })
        .collect();
    let mut rng = Rng::stream(seed, "archetype");
    candidates[rng.weighted_index(&weights)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::HistoryEntry;

    #[test]
    fn a_known_theme_only_gets_scenes_that_have_it() {
        for theme in [Theme::Light, Theme::Dark] {
            let ctx = ContentContext {
                theme: Some(theme),
                ..ContentContext::default()
            };
            for seed in 0..300 {
                let r = compose(seed, Aspect::from_size(1920, 1080), &ctx);
                assert!(
                    r.tod_for(theme).is_some(),
                    "seed {seed}: {:?} has no {theme:?} variant",
                    r.activity
                );
            }
        }
        // Without one, theme-locked activities still come up.
        let any = ContentContext::default();
        assert!(
            (0..300).any(|s| compose(s, Aspect::from_size(1920, 1080), &any).is_theme_locked())
        );
    }

    #[test]
    fn compose_is_deterministic() {
        let ctx = ContentContext::default();
        let a = compose(7, Aspect::from_size(3840, 2160), &ctx);
        let b = compose(7, Aspect::from_size(3840, 2160), &ctx);
        assert_eq!(a.to_json(), b.to_json());
        assert_eq!(a.aspect, Aspect { w: 16, h: 9 });
        assert!(a.activity.is_some());
    }

    #[test]
    fn recipe_round_trips_through_json() {
        let r = compose(
            123,
            Aspect::from_size(1920, 1080),
            &ContentContext::default(),
        );
        let back = SceneRecipe::from_json(&r.to_json()).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn different_seeds_differ() {
        let ctx = ContentContext::default();
        let a = compose(1, Aspect::from_size(16, 9), &ctx);
        let b = compose(2, Aspect::from_size(16, 9), &ctx);
        assert_ne!(a.composition, b.composition);
    }

    #[test]
    fn night_only_activities_are_theme_locked() {
        let ctx = ContentContext {
            enabled_activities: vec!["stargaze".into()],
            ..ContentContext::default()
        };
        let r = compose(5, Aspect::from_size(16, 9), &ctx);
        assert_eq!(r.activity.as_deref(), Some("stargaze"));
        assert!(r.light_tod.is_none() && r.dark_tod.is_some());
        assert!(r.is_theme_locked());
    }

    #[test]
    fn never_repeats_the_last_activity_when_there_is_a_choice() {
        for seed in 0..200 {
            let mut ctx = ContentContext::default();
            ctx.recent.push(HistoryEntry {
                activity: Some("warm_hands".into()),
                archetype: "canyon".into(),
                platform: "rock_ledge".into(),
                prop: None,
            });
            assert_ne!(
                compose(seed, Aspect::from_size(16, 9), &ctx)
                    .activity
                    .as_deref(),
                Some("warm_hands")
            );
        }
    }

    #[test]
    fn falls_back_when_nothing_is_eligible() {
        let ctx = ContentContext {
            enabled_activities: vec![],
            ..ContentContext::default()
        };
        let r = compose(9, Aspect::from_size(16, 9), &ctx);
        assert_eq!(r.activity, None);
        assert!(r.props.is_empty());
        assert!(crate::terrain::PAINTED.contains(&r.archetype.as_str()));
    }
}
