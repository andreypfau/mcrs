use crate::OrderKey;

/// The seed of the world that every keyed stream mixes in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorldSeed(pub u64);

/// The part of the AI that draws from a stream, for example the gate of the random stroll. Two
/// parts of one mob in one tick draw from independent streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Part(pub u32);

/// A stream of random values that is a function of the world seed, the order key of the mob, the
/// issue tick and the part only. The `i`-th value of a stream does not depend on the order of the
/// mobs, the thread count or any other draw.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyedRandom {
    state: u64,
}

impl KeyedRandom {
    #[must_use]
    pub fn new(seed: WorldSeed, key: &OrderKey, tick: u64, part: Part) -> Self {
        let mut state = mix(seed.0 ^ 0x6A09_E667_F3BC_C908);
        for word in key.words() {
            state = mix(state ^ word);
        }
        state = mix(state ^ tick);
        state = mix(state ^ u64::from(part.0).wrapping_mul(0xBB67_AE85_84CA_A73B));
        Self { state }
    }

    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        mix(self.state)
    }

    /// An integer uniform in `[0, bound)`; 0 for a bound of 0.
    #[must_use]
    pub fn below(&mut self, bound: u32) -> u32 {
        (((self.next_u64() >> 32) * u64::from(bound)) >> 32) as u32
    }

    /// An integer uniform in `[-range, range]`.
    #[must_use]
    pub fn symmetric(&mut self, range: u32) -> i32 {
        let span = range.saturating_mul(2).saturating_add(1);
        self.below(span) as i32 - range as i32
    }

    /// A value uniform in `[0, 1)` with 53 random bits.
    #[must_use]
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// A value uniform in `[0, 1)` with 24 random bits, as an f32 draw.
    #[must_use]
    pub fn unit_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1_u32 << 24) as f32
    }

    /// True with the chance `chance`.
    #[must_use]
    pub fn chance(&mut self, chance: f64) -> bool {
        self.unit() < chance
    }

    /// The number of independent trials with the chance `chance` each, up to and including the
    /// first success: 1 or more. A chance of 0 or less gives `u32::MAX`.
    #[must_use]
    pub fn trials_to_success(&mut self, chance: f64) -> u32 {
        if chance >= 1.0 {
            return 1;
        }
        if chance <= 0.0 {
            return u32::MAX;
        }
        let failures = libm::floor(libm::log1p(-self.unit()) / libm::log1p(-chance));
        if failures >= f64::from(u32::MAX - 1) {
            return u32::MAX;
        }
        failures as u32 + 1
    }
}

const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// The finaliser of SplitMix64.
const fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(GOLDEN_GAMMA);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
