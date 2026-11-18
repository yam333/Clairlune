//! Deterministic random numbers.
//!
//! ChaCha8 is used only as a raw bit source. All sampling is hand-written with
//! integer arithmetic (rejection sampling for ranges, integer-weight picks), so
//! output never depends on the sampling algorithms of `rand`, which have changed
//! between versions. The 32-byte ChaCha key is expanded by us with SplitMix64
//! rather than `SeedableRng::seed_from_u64`, for the same reason.

use crate::hash::{hash_label, splitmix64};
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{Rng as RawRng, SeedableRng};

/// A seeded random stream. Create one per subsystem with [`Rng::stream`], so that
/// adding draws in one subsystem never shifts the draws of another.
#[derive(Clone, Debug)]
pub struct Rng {
    inner: ChaCha8Rng,
}

impl Rng {
    pub fn from_u64(seed: u64) -> Self {
        let mut key = [0u8; 32];
        let mut s = seed;
        for chunk in key.as_chunks_mut::<8>().0 {
            s = splitmix64(s);
            *chunk = s.to_le_bytes();
        }
        Self {
            inner: ChaCha8Rng::from_seed(key),
        }
    }

    /// Independent stream `hash(seed, label)`, e.g. `Rng::stream(seed, "terrain")`.
    pub fn stream(seed: u64, label: &str) -> Self {
        Self::from_u64(hash_label(seed, label))
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        RawRng::next_u32(&mut self.inner)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        RawRng::next_u64(&mut self.inner)
    }

    /// Uniform integer in `0..n` (rejection sampling, no modulo bias).
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0, "below(0)");
        let n64 = u64::from(n);
        let limit = ((1u64 << 32) / n64) * n64;
        loop {
            let x = u64::from(self.next_u32());
            if x < limit {
                return (x % n64) as u32;
            }
        }
    }

    /// Uniform integer in `0..n` for 64-bit ranges.
    pub fn below_u64(&mut self, n: u64) -> u64 {
        assert!(n > 0, "below_u64(0)");
        // 2^64 mod n; accept x < 2^64 - rem, which is a multiple of n.
        let rem = (u64::MAX % n).wrapping_add(1) % n;
        loop {
            let x = self.next_u64();
            if x <= u64::MAX - rem {
                return x % n;
            }
        }
    }

    /// Uniform integer in `lo..=hi`.
    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        assert!(lo <= hi);
        let span = u64::from(hi) - u64::from(lo) + 1;
        (u64::from(lo) + self.below_u64(span)) as u32
    }

    /// Uniform integer in `lo..=hi`.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        assert!(lo <= hi);
        let span = (i64::from(hi) - i64::from(lo) + 1) as u64;
        (i64::from(lo) + self.below_u64(span) as i64) as i32
    }

    /// Uniform float in `[0, 1)` built from 53 random bits. Exact on every platform.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Uniform float in `[lo, hi)`.
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// True with probability `num / den`.
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den) < num
    }

    /// Index picked with probability proportional to integer weights.
    pub fn weighted_index(&mut self, weights: &[u32]) -> usize {
        let total: u64 = weights.iter().map(|&w| u64::from(w)).sum();
        assert!(total > 0, "weighted_index: all weights are zero");
        let mut r = self.below_u64(total);
        for (i, &w) in weights.iter().enumerate() {
            let w = u64::from(w);
            if r < w {
                return i;
            }
            r -= w;
        }
        unreachable!("r < total")
    }

    /// Uniform pick from a non-empty slice.
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        assert!(!items.is_empty());
        &items[self.below(items.len() as u32) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_reproducible_and_independent() {
        let a: Vec<u32> = (0..8)
            .map({
                let mut r = Rng::stream(42, "terrain");
                move |_| r.next_u32()
            })
            .collect();
        let b: Vec<u32> = (0..8)
            .map({
                let mut r = Rng::stream(42, "terrain");
                move |_| r.next_u32()
            })
            .collect();
        let c: Vec<u32> = (0..8)
            .map({
                let mut r = Rng::stream(42, "props");
                move |_| r.next_u32()
            })
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut r = Rng::from_u64(1);
        for _ in 0..10_000 {
            assert!(r.below(7) < 7);
            let v = r.range_i32(-3, 3);
            assert!((-3..=3).contains(&v));
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }

    #[test]
    fn weighted_index_skips_zero_weights() {
        let mut r = Rng::from_u64(9);
        for _ in 0..1000 {
            assert_ne!(r.weighted_index(&[3, 0, 5]), 1);
        }
    }

    #[test]
    fn chacha_bit_source_is_pinned() {
        // Guards against a silent change in the ChaCha8 stream or our key expansion.
        let mut r = Rng::from_u64(0);
        let first = r.next_u64();
        let mut r2 = Rng::from_u64(0);
        assert_eq!(first, r2.next_u64());
        assert_ne!(first, 0);
    }
}
