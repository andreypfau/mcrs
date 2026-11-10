use crate::BirthSource;

/// The UUID of an entity, also its order key: the order-independent rules order entities by this
/// value as an unsigned integer of 128 bits. It does not depend on the layout, the thread count
/// or the network id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityKey(pub u128);

impl EntityKey {
    /// The UUID of the entity number `sequence` that `source` brings into existence in `tick` in
    /// the dimension with the key `dimension` of the world with `world_seed`: a fixed hash of
    /// these inputs, with the version 4 and the variant of RFC 9562 so that a client takes it as
    /// a random UUID. Two different sets of inputs give two different streams of words to the
    /// hash, because the length of the dimension key comes before its bytes.
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
        Self(hash & !(0xF << 76) & !(0x3 << 62) | 0x4 << 76 | 0x2 << 62)
    }

    /// The phase offset from 0 to 3, which replaces the network id where a rule picks a tick by
    /// it: the two lowest bits, which are random in a UUID of version 4.
    #[inline]
    #[must_use]
    pub const fn phase_offset(&self) -> u8 {
        (self.0 & 3) as u8
    }

    /// The high and then the low 64 bits: the order in which a keyed stream absorbs the key.
    #[inline]
    #[must_use]
    pub const fn words(&self) -> [u64; 2] {
        [(self.0 >> 64) as u64, self.0 as u64]
    }
}

/// Two lanes of 64 bits that absorb words, for a hash of 128 bits.
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

/// The finaliser of SplitMix64.
#[inline]
pub(crate) fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
