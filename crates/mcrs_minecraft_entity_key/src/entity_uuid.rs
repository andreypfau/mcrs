use uuid::Uuid;

use crate::BirthSource;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct EntityUuid(pub Uuid);

impl EntityUuid {
    #[inline]
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }

    #[inline]
    #[must_use]
    pub const fn as_u128(&self) -> u128 {
        self.0.as_u128()
    }

    #[must_use]
    pub fn born(
        world_seed: u64,
        dimension: &str,
        tick: u64,
        source: &BirthSource,
        sequence: u32,
    ) -> Self {
        let mut lanes = Lanes::new();
        lanes.push(world_seed);
        lanes.push(dimension.len() as u64);
        for chunk in dimension.as_bytes().chunks(8) {
            let mut bytes = [0; 8];
            for (slot, byte) in bytes.iter_mut().zip(chunk) {
                *slot = *byte;
            }
            lanes.push(u64::from_le_bytes(bytes));
        }
        lanes.push(tick);
        lanes.push(source.kind());
        for word in source.words() {
            lanes.push(word);
        }
        lanes.push(u64::from(sequence));
        let Lanes { high, low } = lanes;
        let hash = u128::from(mix(high)) << 64 | u128::from(mix(low ^ high));
        Self::from_u128(hash & !(0xF << 76) & !(0x3 << 62) | 0x4 << 76 | 0x2 << 62)
    }

    #[inline]
    #[must_use]
    pub const fn phase_offset(&self) -> u8 {
        (self.as_u128() & 3) as u8
    }

    #[inline]
    #[must_use]
    pub const fn words(&self) -> [u64; 2] {
        let value = self.as_u128();
        [(value >> 64) as u64, value as u64]
    }
}

struct Lanes {
    high: u64,
    low: u64,
}

impl Lanes {
    fn new() -> Self {
        Self {
            high: 0x243F_6A88_85A3_08D3,
            low: 0x1319_8A2E_0370_7344,
        }
    }

    fn push(&mut self, word: u64) {
        self.high = mix(self.high ^ word);
        self.low = mix(self.low ^ word.rotate_left(32)).wrapping_add(self.high);
    }
}

#[inline]
pub(crate) fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
