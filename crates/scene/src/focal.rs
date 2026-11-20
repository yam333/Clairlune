//! Focal-group layout: the character's motion range, the activity's props on
//! either side, and the unattended variant. Shared by every archetype; the
//! archetype only supplies the ground line.

use crate::activity::{Activity, MotionKind};
use crate::graph::CHARACTER_BOX;
use crate::graph::{CharacterTrack, Motion, SpriteInstance, SpriteKind};
use crate::math::pm;
use crate::recipe::SceneRecipe;

/// Character height as a fraction of the screen's short side (upper bound).
const CHARACTER_MAX_FRACTION: f64 = 0.09;
/// Safe frame: the focal group stays inside 6 %..94 % of the width.
const SAFE_MARGIN: f64 = 0.06;

/// Largest integer scale that keeps a sprite at most 9 % of the screen's short
/// side (the height on landscape screens; the width in portrait).
pub fn sprite_scale(short_side: u32, sprite_h: u32) -> u32 {
    ((CHARACTER_MAX_FRACTION * f64::from(short_side) / f64::from(sprite_h)).floor() as u32).max(1)
}

/// A prop that lights the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightSource {
    pub cx: f64,
    pub cy: f64,
    /// Glow radius relative to a campfire's.
    pub strength: f64,
}

/// Where everything in the focal group goes.
#[derive(Clone, Debug, PartialEq)]
pub struct FocalLayout {
    pub scale: u32,
    /// Horizontal extent of the whole group, including the motion range.
    pub x0: i32,
    pub x1: i32,
    /// Height of the tallest element.
    pub height: i32,
    /// Drawn in both Buddy states.
    pub props: Vec<SpriteInstance>,
    /// Drawn only with Buddy off, where the character would be.
    pub unattended: Vec<SpriteInstance>,
    pub character: Option<CharacterTrack>,
    pub light: Option<LightSource>,
    /// Ember emitter origin (flame tip), if a prop emits embers.
    pub embers_at: Option<(f64, f64)>,
}

fn prop_kind(id: &str) -> Option<SpriteKind> {
    match id {
        "campfire" => Some(SpriteKind::Campfire),
        "log_seat" => Some(SpriteKind::LogSeat),
        "telescope" => Some(SpriteKind::Telescope),
        "lantern" => Some(SpriteKind::Lantern),
        "spellbook" => Some(SpriteKind::Spellbook),
        "camp_stove" => Some(SpriteKind::CampStove),
        "coffee_mug" => Some(SpriteKind::CoffeeMug),
        "fallen_hat" => Some(SpriteKind::FallenHat),
        "book_closed" => Some(SpriteKind::BookClosed),
        "guitar_rest" => Some(SpriteKind::GuitarRest),
        "bucket" => Some(SpriteKind::Bucket),
        "kite_rest" => Some(SpriteKind::KiteRest),
        "stone_pile" => Some(SpriteKind::StonePile),
        "easel" => Some(SpriteKind::Easel),
        "snowman" => Some(SpriteKind::Snowman),
        "basket" => Some(SpriteKind::Basket),
        "picnic_basket" => Some(SpriteKind::PicnicBasket),
        "chess_table" => Some(SpriteKind::ChessTable),
        "practice_wall" => Some(SpriteKind::PracticeWall),
        _ => None,
    }
}

/// Lays out the focal group with feet on `ground_y`.
pub fn layout(
    recipe: &SceneRecipe,
    activity: Option<&Activity>,
    w: u32,
    h: u32,
    ground_y: i32,
    cell_w: u32,
    cell_h: u32,
) -> FocalLayout {
    let comp = &recipe.composition;
    let wf = f64::from(w);
    let scale = sprite_scale(w.min(h), CHARACTER_BOX.1);
    let s = scale as i32;
    let (cw, ch) = (
        (CHARACTER_BOX.0 * scale) as i32,
        (CHARACTER_BOX.1 * scale) as i32,
    );
    let gap = 3 * s;
    let (safe_l, safe_r) = ((SAFE_MARGIN * wf) as i32, ((1.0 - SAFE_MARGIN) * wf) as i32);

    let Some(activity) = activity else {
        // Archetype-first fallback: no character, no props; keep a focus point.
        let cx = (pm(comp.focus_x_pm) * wf).round() as i32;
        return FocalLayout {
            scale,
            x0: cx - cw / 2,
            x1: cx + cw / 2,
            height: ch,
            props: Vec::new(),
            unattended: Vec::new(),
            character: None,
            light: None,
            embers_at: None,
        };
    };

    // Props in file order; `either_side` alternates starting from the recipe's side.
    let mut sides: Vec<(SpriteKind, bool, &str, i32)> = Vec::new();
    let mut next_left = comp.prop_left;
    for (id, def) in &activity.props {
        let Some(kind) = prop_kind(id) else { continue };
        let left = match def.attach.as_str() {
            "left_of_clawd" => true,
            "right_of_clawd" => false,
            _ => {
                let l = next_left;
                next_left = !next_left;
                l
            }
        };
        sides.push((kind, left, id.as_str(), (def.gap * scale) as i32));
    }
    let side_width = |left: bool| -> i32 {
        sides
            .iter()
            .filter(|(_, l, _, _)| *l == left)
            .map(|(k, _, _, extra)| (k.map().width * scale) as i32 + gap + extra)
            .sum()
    };

    // The character's band: its rest width plus the pace range, shrunk if the
    // whole group would not fit the safe frame.
    let mut pace_half = match activity.motion.kind {
        MotionKind::Pace => (pm(comp.pace_half_pm) * f64::from(w.min(h))) as i32,
        _ => 0,
    };
    let fixed = cw + side_width(true) + side_width(false);
    pace_half = pace_half.min(((safe_r - safe_l - fixed) / 2).max(0));
    let half_band = pace_half + cw / 2;
    let group_w = fixed + 2 * pace_half;

    // Centre on the composition's focus, then shift into the safe frame.
    let mut cx = (pm(comp.focus_x_pm) * wf).round() as i32;
    let left_edge = cx - half_band - side_width(true);
    if left_edge < safe_l {
        cx += safe_l - left_edge;
    } else if left_edge + group_w > safe_r {
        cx -= left_edge + group_w - safe_r;
    }

    let mut props = Vec::new();
    let mut light = None;
    let mut embers_at = None;
    let (mut left_cursor, mut right_cursor) = (cx - half_band - gap, cx + half_band + gap);
    // Tall poses (a hat, a raised wand) get extra cleared space above the box.
    let mut height = ch + (activity.character.headroom * scale) as i32;
    for (kind, left, id, extra) in &sides {
        let map = kind.map();
        let (pw, ph) = ((map.width * scale) as i32, (map.height * scale) as i32);
        let x = if *left {
            left_cursor -= pw + extra;
            let x = left_cursor;
            left_cursor -= gap;
            x
        } else {
            right_cursor += extra;
            let x = right_cursor;
            right_cursor += pw + gap;
            x
        };
        let y = ground_y - ph;
        height = height.max(ph);
        props.push(SpriteInstance {
            kind: *kind,
            x,
            y,
            scale,
            mirror: !*left,
        });
        let pcx = f64::from(x) + f64::from(pw) * 0.5;
        match *id {
            "campfire" if light.is_none() => {
                light = Some(LightSource {
                    cx: pcx,
                    cy: f64::from(y) + f64::from(ph) * 0.6,
                    strength: 1.0,
                });
                embers_at = Some((pcx, f64::from(y + s)));
            }
            "spellbook" if light.is_none() => {
                light = Some(LightSource {
                    cx: pcx,
                    cy: f64::from(y) + f64::from(ph) * 0.3,
                    strength: 0.45,
                });
            }
            "camp_stove" if light.is_none() => {
                light = Some(LightSource {
                    cx: pcx,
                    cy: f64::from(y) + f64::from(ph) * 0.7,
                    strength: 0.35,
                });
            }
            "lantern" if light.is_none() => {
                light = Some(LightSource {
                    cx: pcx,
                    cy: f64::from(y) + f64::from(ph) * 0.55,
                    strength: 0.55,
                });
            }
            _ => {}
        }
    }

    // Unattended props that aren't already in the scene stand where the character rests.
    let mut unattended = Vec::new();
    for id in &activity.unattended.props {
        if activity.props.contains_key(id) {
            continue;
        }
        if let Some(kind) = prop_kind(id) {
            let map = kind.map();
            let (pw, ph) = ((map.width * scale) as i32, (map.height * scale) as i32);
            unattended.push(SpriteInstance {
                kind,
                x: cx - pw / 2,
                y: ground_y - ph,
                scale,
                mirror: comp.prop_left,
            });
        }
    }

    let motion = match activity.motion.kind {
        MotionKind::Pace if pace_half > 0 => {
            let span = 2 * pace_half;
            let px_per_s = f64::from(comp.motion_speed_cc) / 100.0 * f64::from(cell_w);
            let period_ms = ((2.0 * f64::from(span) / px_per_s.max(1.0)) * 1000.0).round() as u64;
            Motion::Pace {
                x_min: cx - pace_half - cw / 2,
                x_max: cx + pace_half - cw / 2,
                period_ms: period_ms.max(1000),
            }
        }
        // Across the middle of the screen, high over the group's ledge.
        MotionKind::Fly if comp.fly_lift_pm > 0 => {
            let (x_min, x_max) = ((w as f64 * 0.12) as i32, (w as f64 * 0.88) as i32 - cw);
            let px_per_s = f64::from(comp.motion_speed_cc) / 100.0 * f64::from(cell_w);
            let period_ms =
                ((2.0 * f64::from(x_max - x_min) / px_per_s.max(1.0)) * 1000.0).round() as u64;
            // Never above 6 % of the screen height.
            let lift = (f64::from(comp.fly_lift_pm) / 1000.0 * f64::from(h)) as i32;
            let lift = lift
                .min(ground_y - height - (h as f64 * 0.06) as i32)
                .max(0);
            Motion::Fly {
                x_min,
                x_max: x_max.max(x_min),
                period_ms: period_ms.max(4000),
                lift,
                bob: (h as i32 / 150).max(2),
                bob_ms: 3000,
            }
        }
        // A boat on the shore's water past the pier's end (the pier comes
        // in from the edge nearer the group), rowing back and forth.
        MotionKind::Bob if comp.motion_speed_cc > 0 => {
            let unit = f64::from(w.min(h));
            let margin = (pm(comp.platform_margin_pm) * unit) as i32 + 5 * cell_w as i32;
            let (x_min, x_max) = if cx < w as i32 / 2 {
                (cx + cw / 2 + margin, (w as f64 * 0.92) as i32 - cw)
            } else {
                ((w as f64 * 0.08) as i32, cx - cw / 2 - margin - cw)
            };
            let px_per_s = f64::from(comp.motion_speed_cc) / 100.0 * f64::from(cell_w);
            let period_ms = ((2.0 * f64::from((x_max - x_min).max(0)) / px_per_s.max(1.0)) * 1000.0)
                .round() as u64;
            // The waterline under the pier (see terrain::shore).
            let water = ground_y + 2 * cell_h as i32 + cell_h as i32 / 2;
            Motion::Fly {
                x_min,
                x_max: x_max.max(x_min),
                period_ms: period_ms.max(6000),
                lift: ground_y - water,
                bob: (h as i32 / 260).max(1),
                bob_ms: 2000,
            }
        }
        _ => Motion::Stationary { x: cx - cw / 2 },
    };

    FocalLayout {
        scale,
        x0: left_cursor + gap,
        x1: right_cursor - gap,
        height,
        props,
        unattended,
        character: Some(CharacterTrack {
            y: ground_y - ch,
            scale,
            motion,
            face_left: match activity.motion.face.as_deref() {
                // Toward the middle of the screen: the shore's pier comes in
                // from the nearer edge, so the open water lies that way, and
                // so does the most open sky.
                Some("water" | "open") => cx > w as i32 / 2,
                _ => comp.prop_left,
            },
        }),
        light,
        embers_at,
    }
}
