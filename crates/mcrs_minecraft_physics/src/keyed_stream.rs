use crate::BirthKey;
use crate::birth_key::mix;

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
        for word in key.words() {
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
