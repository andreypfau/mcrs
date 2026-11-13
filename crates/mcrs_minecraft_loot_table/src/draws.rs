use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

/// The random values of one evaluation, in the order it takes them.
pub(crate) struct Draws(XoroshiroRandom);

impl Draws {
    pub(crate) fn new(seed: u64) -> Self {
        Self(XoroshiroRandom::new(seed))
    }

    /// A uniform value in `[0, 1)`.
    pub(crate) fn unit(&mut self) -> f32 {
        self.0.next_f32()
    }

    /// A uniform integer in `low..=high`; `low` without a draw when `low >= high`.
    pub(crate) fn between(&mut self, low: i32, high: i32) -> i32 {
        if low >= high {
            return low;
        }
        let span = u32::try_from(i64::from(high) - i64::from(low) + 1).unwrap_or(u32::MAX);
        low.saturating_add_unsigned(self.0.next_u32_bound(span))
    }

    /// How many of `trials` draws fall below `chance`.
    pub(crate) fn successes(&mut self, trials: i32, chance: f32) -> i32 {
        (0..trials).map(|_| i32::from(self.unit() < chance)).sum()
    }
}
