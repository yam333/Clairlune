//! Deterministic scene generation for Clairlune.
//!
//! `seed -> SceneRecipe -> SceneGraph`. This crate is pure: no OS calls, no I/O,
//! no clocks. The same inputs give byte-identical outputs on every platform, which
//! is why it uses its own stable hashing, ChaCha8 as a raw bit source with
//! hand-written integer sampling, and `libm` for transcendental functions.

/// Compiles the README example as a doctest so it can't go stale.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod build;
pub mod compose;
pub mod glyphs;
pub mod graph;
pub mod hash;
pub mod ident;
pub mod math;
pub mod noise;
pub mod palette;
pub mod particles;
pub mod placeholder;
pub mod recipe;
pub mod rng;

pub use build::{BuildError, BuildOptions, build};
pub use compose::compose;
pub use graph::{Cell, GridSpec, Lighting, SceneGraph, SpriteInstance, SpriteKind, grid_spec};
pub use ident::{APP_ID, APP_NAME};
pub use palette::{PaletteFamily, Rgb, Theme};
pub use recipe::{Aspect, ContentContext, GENERATOR_VERSION, SceneRecipe, TimeOfDay};
