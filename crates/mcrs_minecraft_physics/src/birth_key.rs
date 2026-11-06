/// The order key of an entity: its birth tick, its source and its sequence among the entities of
/// that source and tick. It does not depend on the layout, the thread count or the network id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BirthKey {
    tick: u64,
    kind: SourceKind,
    source: u128,
    sequence: u32,
}

/// The kind of the source of a birth, in the order of the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceKind {
    Load,
    Player,
    Command,
    Spawn,
    Block,
    Entity,
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
}
