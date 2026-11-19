//! The embedded art (drawn as code in assets/clawd/design/, exported by
//! scripts/export_sprites.py) is loaded and used instead of the placeholder.

use clairlune_scene::activity::builtin_catalog;
use clairlune_sprites::{Character, SheetCharacter};

#[test]
fn drawn_art_is_used() {
    let c = SheetCharacter::builtin().expect("embedded sheets parse");
    assert!(c.has_sheet("_base"));
    assert!(c.has_sheet("warm_hands"));
    assert!(c.validate(builtin_catalog()).is_empty());

    // Activity-specific tags come from the activity's sheet...
    let warm = c.animation("warm_hands", "warm").unwrap();
    assert_eq!(warm.name, "warm");
    assert_eq!(warm.frames.len(), 4);
    // ...shared states from the base sheet, even for activities without a sheet.
    assert_eq!(c.animation("warm_hands", "idle").unwrap().name, "idle");
    assert_eq!(c.animation("stroll", "walk").unwrap().name, "walk");

    for (activity, state) in [
        ("warm_hands", "warm"),
        ("warm_hands", "look_around"),
        ("stroll", "walk"),
    ] {
        for f in &c.animation(activity, state).unwrap().frames {
            assert_eq!((f.width, f.height), (24, 20));
            assert_eq!(f.origin(), (11, 19), "{activity}/{state}");
            assert!(f.anchors.contains_key("hand_r"));
            // The origin is on the lowest foot row: a foot pixel sits in that row.
            assert!((0..f.width).any(|x| f.pixel(x, 19).is_some()));
        }
    }
    // One-shot length comes from the art: look_around sums its frame durations.
    assert_eq!(
        c.one_shot_ms("warm_hands", "look_around"),
        320 + 320 + 110 + 320 + 320 + 200
    );
}
