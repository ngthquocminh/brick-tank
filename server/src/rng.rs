//! Small deterministic PRNG (SplitMix64).

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `lo..hi` (half-open). `hi` must be greater than `lo`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let span = (hi as i64 - lo as i64) as u64;
        lo + (self.next_u64() % span) as i32
    }

    pub fn chance(&mut self, percent: u32) -> bool {
        (self.next_u64() % 100) < percent as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn range_stays_in_bounds() {
        let mut r = Rng::new(7);
        let (mut lo_seen, mut hi_seen) = (false, false);
        for _ in 0..10_000 {
            let v = r.range(-3, 7);
            assert!((-3..7).contains(&v));
            lo_seen |= v == -3;
            hi_seen |= v == 6;
        }
        assert!(lo_seen && hi_seen);
    }
}
