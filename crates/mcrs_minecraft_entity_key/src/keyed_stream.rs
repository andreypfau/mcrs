use crate::EntityUuid;
use crate::entity_uuid::mix;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyedStream {
    seed: u64,
}

impl KeyedStream {
    #[inline]
    #[must_use]
    pub fn new(uuid: &EntityUuid, tick: u64) -> Self {
        let mut seed = mix(tick ^ 0x6A09_E667_F3BC_C908);
        for word in uuid.words() {
            seed = mix(seed ^ word);
        }
        Self { seed }
    }

    #[inline]
    #[must_use]
    pub fn below(&self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        ((mix(self.seed) >> 32) % u64::from(bound)) as u32
    }

    #[inline]
    #[must_use]
    pub fn unit(&self, draw: u64) -> f64 {
        (mix(self.seed ^ mix(draw)) >> 11) as f64 / (1_u64 << 53) as f64
    }
}
