//! Hand-written value noise. Small, dependency-free and deterministic: only
//! `floor`, `+`, `-`, `*` on f64, with lattice values from the stable hash.

use crate::hash::{hash3, splitmix64, unit_from_hash};

#[inline]
fn lattice(seed: u64, ix: i64, iy: i64) -> f64 {
    unit_from_hash(hash3(seed, ix, iy))
}

/// Quintic fade, C2-continuous.
#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn octave_seed(seed: u64, octave: u32) -> u64 {
    splitmix64(seed ^ u64::from(octave).wrapping_mul(0xA24B_AED4_963E_E407))
}

/// 2D value noise in `[0, 1)`.
pub fn value2(seed: u64, x: f64, y: f64) -> f64 {
    let x0 = x.floor();
    let y0 = y.floor();
    let ix = x0 as i64;
    let iy = y0 as i64;
    let fx = fade(x - x0);
    let fy = fade(y - y0);
    let a = lattice(seed, ix, iy);
    let b = lattice(seed, ix + 1, iy);
    let c = lattice(seed, ix, iy + 1);
    let d = lattice(seed, ix + 1, iy + 1);
    let ab = a + (b - a) * fx;
    let cd = c + (d - c) * fx;
    ab + (cd - ab) * fy
}

/// 1D value noise in `[0, 1)`.
pub fn value1(seed: u64, x: f64) -> f64 {
    let x0 = x.floor();
    let ix = x0 as i64;
    let fx = fade(x - x0);
    let a = lattice(seed, ix, 0);
    let b = lattice(seed, ix + 1, 0);
    a + (b - a) * fx
}

/// Fractal (fBm) 2D value noise, normalised to `[0, 1)`.
pub fn fbm2(seed: u64, x: f64, y: f64, octaves: u32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;
    for o in 0..octaves {
        sum += amp * value2(octave_seed(seed, o), x * freq, y * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Fractal (fBm) 1D value noise, normalised to `[0, 1)`.
pub fn fbm1(seed: u64, x: f64, octaves: u32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;
    for o in 0..octaves {
        sum += amp * value1(octave_seed(seed, o), x * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Ridged 1D noise in `[0, 1]`: sharp peaks, for cliff and mountain silhouettes.
pub fn ridged1(seed: u64, x: f64, octaves: u32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut norm = 0.0;
    let mut freq = 1.0;
    for o in 0..octaves {
        let v = value1(octave_seed(seed, o), x * freq);
        let r = 1.0 - (2.0 * v - 1.0).abs();
        sum += amp * r * r;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        for i in 0..2000 {
            let x = f64::from(i) * 0.137 - 50.0;
            let y = f64::from(i) * 0.071 + 3.0;
            let v = fbm2(11, x, y, 4);
            assert!((0.0..1.0).contains(&v), "{v}");
            let r = ridged1(11, x, 4);
            assert!((0.0..=1.0).contains(&r), "{r}");
        }
    }

    #[test]
    fn continuous_at_lattice() {
        let a = value2(3, 1.0 - 1e-9, 2.5);
        let b = value2(3, 1.0, 2.5);
        assert!((a - b).abs() < 1e-6);
    }
}
