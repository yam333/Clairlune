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

// ---- Periodic noise, for scrolling scenes that loop (docs/scrolling-scenes.md).
// The lattice wraps every `period` cells, so the noise repeats exactly in x. Each
// fBm octave doubles both frequency and period, so wrapping holds at every octave.
// For a bit-exact seam, sample at `x` already reduced into `[0, period)`, e.g.
// `(column % period_columns) as f64 * step`.

/// 1D value noise in `[0, 1)` that repeats every `period` lattice cells (`period >= 1`).
pub fn value1_periodic(seed: u64, x: f64, period: i64) -> f64 {
    let x0 = x.floor();
    let ix = x0 as i64;
    let fx = fade(x - x0);
    let a = lattice(seed, ix.rem_euclid(period), 0);
    let b = lattice(seed, (ix + 1).rem_euclid(period), 0);
    a + (b - a) * fx
}

/// 2D value noise in `[0, 1)` that repeats every `period_x` lattice cells in x.
pub fn value2_periodic_x(seed: u64, x: f64, y: f64, period_x: i64) -> f64 {
    let x0 = x.floor();
    let y0 = y.floor();
    let (ix, iy) = (x0 as i64, y0 as i64);
    let (fx, fy) = (fade(x - x0), fade(y - y0));
    let (i0, i1) = (ix.rem_euclid(period_x), (ix + 1).rem_euclid(period_x));
    let a = lattice(seed, i0, iy);
    let b = lattice(seed, i1, iy);
    let c = lattice(seed, i0, iy + 1);
    let d = lattice(seed, i1, iy + 1);
    let ab = a + (b - a) * fx;
    let cd = c + (d - c) * fx;
    ab + (cd - ab) * fy
}

/// Periodic fBm (1D): repeats every `period` units of `x`.
pub fn fbm1_periodic(seed: u64, x: f64, period: i64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..octaves {
        let k = 1i64 << o;
        sum += amp * value1_periodic(octave_seed(seed, o), x * k as f64, period * k);
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

/// Periodic ridged noise (1D): repeats every `period` units of `x`, with the
/// sharp peaks of [`ridged1`].
pub fn ridged1_periodic(seed: u64, x: f64, period: i64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..octaves {
        let k = 1i64 << o;
        let v = value1_periodic(octave_seed(seed, o), x * k as f64, period * k);
        let r = 1.0 - (2.0 * v - 1.0).abs();
        sum += amp * r * r;
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

/// Periodic fBm (2D, wrapping in x): repeats every `period_x` units of `x`.
pub fn fbm2_periodic_x(seed: u64, x: f64, y: f64, period_x: i64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..octaves {
        let k = 1i64 << o;
        let kf = k as f64;
        sum += amp * value2_periodic_x(octave_seed(seed, o), x * kf, y * kf, period_x * k);
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

#[cfg(test)]
mod periodic_tests {
    use super::*;

    #[test]
    fn periodic_noise_repeats_exactly_on_the_lattice() {
        // Sampled the way a scrolling layer samples: integer columns, wrapped.
        let (period, cols_per_cell) = (12i64, 8i64);
        let period_cols = period * cols_per_cell;
        for col in 0..period_cols * 3 {
            // `x` runs past the period unwrapped; `x0` is the same point in the
            // first period. Dyadic steps keep both exact, so the values must match
            // bit for bit.
            let x = col as f64 / cols_per_cell as f64;
            let x0 = col.rem_euclid(period_cols) as f64 / cols_per_cell as f64;
            assert_eq!(
                fbm1_periodic(5, x, period, 4),
                fbm1_periodic(5, x0, period, 4)
            );
            assert_eq!(
                fbm2_periodic_x(5, x, 1.5, period, 4),
                fbm2_periodic_x(5, x0, 1.5, period, 4)
            );
        }
    }

    #[test]
    fn the_seam_is_continuous() {
        // Approaching the period from below meets the value at 0: no visible seam.
        let period = 7;
        let near_end = fbm1_periodic(9, period as f64 - 1e-9, period, 4);
        let start = fbm1_periodic(9, 0.0, period, 4);
        assert!((near_end - start).abs() < 1e-6, "{near_end} vs {start}");
        let near_end = fbm2_periodic_x(9, period as f64 - 1e-9, 0.3, period, 4);
        let start = fbm2_periodic_x(9, 0.0, 0.3, period, 4);
        assert!((near_end - start).abs() < 1e-6);
        // And it is genuinely varying noise, not a constant.
        assert!((fbm1_periodic(9, 0.5, period, 4) - fbm1_periodic(9, 3.5, period, 4)).abs() > 1e-6);
    }
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
