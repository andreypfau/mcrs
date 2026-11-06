use crate::BirthKey;

/// A random stream that is a function of a birth key and a tick: one value for each entity and
/// tick, independent of the order of the entities and of other uses of a random source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyedStream {
    seed: u64,
}

impl KeyedStream {
    #[must_use]
    pub fn new(key: &BirthKey, tick: u64) -> Self {
        let mut seed = mix(tick ^ 0x6A09_E667_F3BC_C908);
        for word in [
            key.tick(),
            key.kind() as u64,
            key.source() as u64,
            (key.source() >> 64) as u64,
            u64::from(key.sequence()),
        ] {
            seed = mix(seed ^ word);
        }
        Self { seed }
    }

    /// A value from 0 to `bound - 1`; 0 for a bound of 0.
    #[must_use]
    pub fn below(&self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        ((mix(self.seed) >> 32) % u64::from(bound)) as u32
    }
}

/// The finaliser of SplitMix64.
fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
