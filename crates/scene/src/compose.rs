//! `compose(seed, aspect, ctx) -> SceneRecipe`: sequential, seeded, weighted picks.
//!
//! Each subsystem draws from its own RNG stream, so adding a pick in one place
//! never shifts the draws of another.
//!
//! M1 scope: only the `canyon` archetype exists and the activity engine (M2) does
//! not, so step 0 (activity) records `None` and the focal group is the M1
//! placeholder (character + campfire on a rock ledge).

use crate::palette::{Theme, families_for};
use crate::recipe::{
    Aspect, Composition, ContentContext, GENERATOR_VERSION, PalettePick, SceneRecipe, TimeOfDay,
};
use crate::rng::Rng;

/// Archetypes this generator version can build.
pub const IMPLEMENTED_ARCHETYPES: [&str; 1] = ["canyon"];

/// Weight multiplier (out of 4) for an archetype used in the most recent scene.
const RECENT_REPEAT_WEIGHT: u32 = 1;
const BASE_WEIGHT: u32 = 4;

/// Builds a recipe. Pure: same inputs, same recipe, on every platform.
pub fn compose(seed: u64, aspect: Aspect, ctx: &ContentContext) -> SceneRecipe {
    let archetype = pick_archetype(seed, ctx);

    // Time of day: one per theme. The campfire placeholder allows all of them,
    // so M1 scenes are never theme-locked.
    let mut tod = Rng::stream(seed, "time_of_day");
    let light_tod = *tod.choose(&TimeOfDay::LIGHT);
    let dark_tod = *tod.choose(&TimeOfDay::DARK);

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

    let mut props = Rng::stream(seed, "props");
    let prop_left = props.chance(1, 2);

    let mut pal = Rng::stream(seed, "palette");
    let light: Vec<_> = families_for(Theme::Light).collect();
    let dark: Vec<_> = families_for(Theme::Dark).collect();
    let light_family = pal.choose(&light).id.to_string();
    let dark_family = pal.choose(&dark).id.to_string();

    SceneRecipe {
        generator_version: GENERATOR_VERSION,
        seed,
        aspect,
        content_ctx_hash: format!("{:016x}", ctx.hash()),
        activity: None,
        archetype,
        light_tod: Some(light_tod),
        dark_tod: Some(dark_tod),
        platform: "rock_ledge".to_string(),
        props: vec!["campfire".to_string()],
        weather: "embers".to_string(),
        composition: Composition {
            horizon_pm,
            focus_x_pm,
            gap_pm,
            shape_pow_pm,
            platform_margin_pm,
            cliff_layers: 3,
            prop_left,
        },
        palette: PalettePick {
            light: light_family,
            dark: dark_family,
        },
    }
}

/// Weighted pick among archetypes that are both enabled and implemented, with
/// the most recent archetype down-weighted. Falls back to canyon (logged by the
/// caller in later milestones) when the filters leave nothing.
fn pick_archetype(seed: u64, ctx: &ContentContext) -> String {
    let candidates: Vec<&str> = IMPLEMENTED_ARCHETYPES
        .iter()
        .copied()
        .filter(|a| ctx.enabled_archetypes.iter().any(|e| e == a))
        .collect();
    if candidates.is_empty() {
        return IMPLEMENTED_ARCHETYPES[0].to_string();
    }
    let last = ctx.recent.last().map(|h| h.archetype.as_str());
    let weights: Vec<u32> = candidates
        .iter()
        .map(|a| {
            if Some(*a) == last && candidates.len() > 1 {
                RECENT_REPEAT_WEIGHT
            } else {
                BASE_WEIGHT
            }
        })
        .collect();
    let mut rng = Rng::stream(seed, "archetype");
    candidates[rng.weighted_index(&weights)].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_is_deterministic() {
        let ctx = ContentContext::default();
        let a = compose(7, Aspect::from_size(3840, 2160), &ctx);
        let b = compose(7, Aspect::from_size(3840, 2160), &ctx);
        assert_eq!(a.to_json(), b.to_json());
        assert_eq!(a.aspect, Aspect { w: 16, h: 9 });
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
}
