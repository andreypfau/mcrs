use crate::BirthSource;
use crate::SourceKind;

/// The order key of an entity: its birth tick, its source and its sequence among the entities of
/// that source and tick. It does not depend on the layout, the thread count or the network id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BirthKey {
    tick: u64,
    kind: SourceKind,
    source: u128,
    sequence: u32,
}

impl BirthKey {
    /// `source` is the packed value of the source key: a block position by `y`, `z`, `x`; a chunk
    /// and an attempt; a chunk and a list position; a sequence; or the digest of the parent key.
    #[must_use]
    pub const fn new(tick: u64, kind: SourceKind, source: u128, sequence: u32) -> Self {
        Self {
            tick,
            kind,
            source,
            sequence,
        }
    }

    #[must_use]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[must_use]
    pub const fn kind(&self) -> SourceKind {
        self.kind
    }

    #[must_use]
    pub const fn source(&self) -> u128 {
        self.source
    }

    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// The key of the entity number `sequence` that `source` brings into existence in `tick`.
    #[must_use]
    pub fn born(tick: u64, source: &BirthSource, sequence: u32) -> Self {
        Self::new(tick, source.kind(), source.packed(), sequence)
    }

    /// A fixed function of 128 bits of the key: the source value of the children of the entity.
    #[must_use]
    pub fn digest(&self) -> u128 {
        let mut high = 0x243F_6A88_85A3_08D3;
        let mut low = 0x1319_8A2E_0370_7344;
        for word in self.words() {
            high = mix(high ^ word);
            low = mix(low ^ word.rotate_left(32)).wrapping_add(high);
        }
        u128::from(mix(high)) << 64 | u128::from(mix(low ^ high))
    }

    /// The phase offset from 0 to 3, which replaces the network id where a rule picks a tick by
    /// it.
    #[must_use]
    pub fn phase_offset(&self) -> u8 {
        (self.digest() & 3) as u8
    }

    #[inline]
    pub(crate) fn words(&self) -> [u64; 5] {
        [
            self.tick,
            self.kind as u64,
            self.source as u64,
            (self.source >> 64) as u64,
            u64::from(self.sequence),
        ]
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
