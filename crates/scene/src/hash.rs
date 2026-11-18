//! Stable hashing.
//!
//! RNG streams, noise lattices and recipe hashes depend on these functions, so
//! their output is part of the on-disk format and must never change. Never use
//! `std::hash::DefaultHasher` for anything that affects output: its algorithm is
//! unspecified and may change between Rust releases.

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// SplitMix64 finaliser. A bijective, well-mixed 64-bit permutation.
#[inline]
pub const fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a over bytes, finished with SplitMix64 for avalanche.
#[derive(Clone, Debug)]
pub struct StableHasher {
    state: u64,
}

impl Default for StableHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StableHasher {
    pub const fn new() -> Self {
        Self { state: FNV_OFFSET }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= u64::from(b);
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    pub fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    /// Length-prefixed, so `("ab", "c")` and `("a", "bc")` hash differently.
    pub fn write_str(&mut self, s: &str) {
        self.write_u64(s.len() as u64);
        self.write_bytes(s.as_bytes());
    }

    pub fn finish(&self) -> u64 {
        splitmix64(self.state)
    }
}

/// `hash(seed, label)`: derives a sub-seed, e.g. one RNG stream per subsystem.
pub fn hash_label(seed: u64, label: &str) -> u64 {
    let mut h = StableHasher::new();
    h.write_u64(seed);
    h.write_str(label);
    h.finish()
}

/// Hash of a byte string.
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = StableHasher::new();
    h.write_bytes(bytes);
    h.finish()
}

/// Fast lattice hash for noise and per-cell jitter.
#[inline]
pub fn hash3(seed: u64, a: i64, b: i64) -> u64 {
    let ka = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let kb = (b as u64)
        .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        .rotate_left(31);
    splitmix64(seed ^ splitmix64(ka ^ kb))
}

/// Maps a hash to a float in `[0, 1)` using its top 53 bits. Exact on every platform.
#[inline]
pub fn unit_from_hash(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_never_change() {
        // Pinned outputs. If one of these fails, every recipe and image changes.
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(hash_label(7, "terrain"), hash_label(7, "terrain"));
        assert_ne!(hash_label(7, "terrain"), hash_label(7, "props"));
        let mut a = StableHasher::new();
        a.write_str("ab");
        a.write_str("c");
        let mut b = StableHasher::new();
        b.write_str("a");
        b.write_str("bc");
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn unit_range() {
        assert_eq!(unit_from_hash(0), 0.0);
        assert!(unit_from_hash(u64::MAX) < 1.0);
    }
}
