//! A small random number generator with a fixed algorithm, so a saved seed gives the same draw on every
//! computer and in every future version (specification 18.3). It is for sampling and resampling, not secrets.

/// SplitMix64: one 64-bit state, passes the usual statistical tests, and is easy to reproduce elsewhere.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A whole number in `0..n` with no modulo bias worth worrying about for sampling.
    pub fn below(&mut self, n: usize) -> usize {
        ((u128::from(self.next_u64()) * n as u128) >> 64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sequence_for_a_seed_never_changes() {
        // These are the published SplitMix64 outputs for seed 1234567.
        let mut r = SplitMix64::new(1234567);
        assert_eq!(r.next_u64(), 6457827717110365317);
        assert_eq!(r.next_u64(), 3203168211198807973);
        assert_eq!(r.next_u64(), 9817491932198370423);
    }

    #[test]
    fn below_stays_in_range_and_covers_it() {
        let mut r = SplitMix64::new(1);
        let mut seen = [false; 7];
        for _ in 0..1000 {
            seen[r.below(7)] = true;
        }
        assert!(seen.iter().all(|s| *s));
        assert_eq!(SplitMix64::new(5).below(1), 0);
    }
}
