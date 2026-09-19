//! A tiny deterministic PRNG (SplitMix64).
//!
//! Procedural scenes need reproducible randomness, not cryptographic quality —
//! and pinning the algorithm here means a seeded scene stays byte-identical
//! across machines and crate updates.

#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_add(0x9E37_79B9_7F4A_7C15))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`. Returns 0 when `n == 0`.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo + 1) as usize) as i32
    }

    pub fn chance(&mut self, percent: u32) -> bool {
        (self.next_u64() % 100) < percent as u64
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take(seed: u64, n: usize) -> Vec<u64> {
        let mut rng = Rng::new(seed);
        (0..n).map(|_| rng.next_u64()).collect()
    }

    #[test]
    fn same_seed_same_stream() {
        assert_eq!(take(42, 8), take(42, 8));
    }

    #[test]
    fn different_seeds_diverge() {
        assert_ne!(take(1, 8), take(2, 8));
    }

    #[test]
    fn stays_inside_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..500 {
            let v = r.range(-3, 9);
            assert!((-3..=9).contains(&v));
            assert!(r.below(5) < 5);
        }
        assert_eq!(r.range(4, 4), 4);
        assert_eq!(r.below(0), 0);
    }
}
