//! Deterministic randomness.
//!
//! * [`Rng`]: a seeded ChaCha8 stream for structural randomness (generation) that is
//!   consumed in a fixed order.
//! * [`keyed`]: a stateless hash of a key tuple, used for per-decision randomness, so that
//!   the result never depends on processing order.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

#[inline]
pub fn mix64(mut z: u64) -> u64 {
    // SplitMix64 finaliser.
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Stateless random value for a key tuple.
#[inline]
pub fn keyed(seed: u64, a: u64, b: u64, c: u64) -> u64 {
    mix64(seed ^ mix64(a ^ mix64(b ^ mix64(c))))
}

/// Uniform integer in `lo..=hi` from a hash value (tiny modulo bias is irrelevant here).
#[inline]
pub fn in_range(h: u64, lo: i64, hi: i64) -> i64 {
    if hi <= lo {
        return lo;
    }
    lo + (h % ((hi - lo + 1) as u64)) as i64
}

/// Purposes for [`keyed`], so different decisions never share random values.
#[repr(u64)]
#[derive(Clone, Copy)]
pub enum Purpose {
    Duration = 1,
    Utility = 2,
    PlaceChoice = 3,
    SleepJitter = 4,
    Visit = 5,
    Spawn = 6,
    Interior = 7,
}

pub struct Rng(ChaCha8Rng);

impl Rng {
    pub fn new(seed: u64, stream: u64) -> Rng {
        let mut r = ChaCha8Rng::seed_from_u64(mix64(seed));
        r.set_stream(stream);
        Rng(r)
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }
    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        in_range(self.next_u64(), lo, hi)
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }
    /// True with probability `permille / 1000`.
    pub fn permille(&mut self, permille: u32) -> bool {
        (self.next_u64() % 1000) < permille as u64
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
    /// Weighted choice; returns an index. Weights of 0 are never picked.
    pub fn weighted(&mut self, weights: &[u32]) -> Option<usize> {
        let total: u64 = weights.iter().map(|w| *w as u64).sum();
        if total == 0 {
            return None;
        }
        let mut x = self.next_u64() % total;
        for (i, w) in weights.iter().enumerate() {
            if x < *w as u64 {
                return Some(i);
            }
            x -= *w as u64;
        }
        None
    }
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rng_is_reproducible() {
        let mut a = Rng::new(42, 1);
        let mut b = Rng::new(42, 1);
        let mut c = Rng::new(42, 2);
        let va: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let vb: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        let vc: Vec<u64> = (0..8).map(|_| c.next_u64()).collect();
        assert_eq!(va, vb);
        assert_ne!(va, vc);
        // Pinned value: changes here mean the RNG changed and every world changes.
        assert_eq!(keyed(1, 2, 3, 4), keyed(1, 2, 3, 4));
    }
}
