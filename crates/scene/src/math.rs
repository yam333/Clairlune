//! Small math helpers.
//!
//! Basic IEEE operations (`+ - * /`, `sqrt`, `floor`, `round`, `min`, `max`) give
//! identical results on every platform. Transcendental functions (`pow`, `exp`,
//! `sin`) do not when they come from the platform's C library, so they must go
//! through `libm`, which is pure Rust and identical everywhere.

pub use core::f64::consts::TAU;

#[inline]
pub fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Hermite smoothstep between `e0` and `e1`.
#[inline]
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = clamp01((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

/// `x^y` via libm (deterministic across platforms).
#[inline]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// `sin(x)` via libm (deterministic across platforms).
#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Per-mille integer to a fraction.
#[inline]
pub fn pm(v: u32) -> f64 {
    f64::from(v) / 1000.0
}
