//! Sprite sheets and the `Character` trait.
//!
//! All character-specific code and data live here, behind [`Character`]
//! (PLAN.md section 7.2, step 6). If the character ever has to be replaced, only
//! this crate and its `assets/clawd/` change.

pub mod aseprite;
pub mod character;
pub mod sheet;

pub use character::{Character, Placeholder, SheetCharacter};
pub use sheet::{Animation, Frame, SheetError, SpriteSheet};

/// Sheets embedded from `assets/clawd/` by the build script.
pub mod embedded {
    include!(concat!(env!("OUT_DIR"), "/sheets.rs"));
}
