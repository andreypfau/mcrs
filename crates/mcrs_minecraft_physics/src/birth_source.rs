use mcrs_voxel::BlockPos;

use crate::BirthKey;
use crate::SourceKind;

/// What brings an entity into existence: the source of its birth key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BirthSource {
    /// A player that joins or respawns, in the order of the logins and respawns of the tick.
    Player {
        order: u32,
    },
    Block(BlockPos),
    /// Another entity, by its birth key.
    Entity(BirthKey),
}

impl BirthSource {
    #[must_use]
    pub fn kind(&self) -> SourceKind {
        match self {
            Self::Player { .. } => SourceKind::Player,
            Self::Block(_) => SourceKind::Block,
            Self::Entity(_) => SourceKind::Entity,
        }
    }

    /// The source value of the key, packed so that it compares as an unsigned integer in the
    /// order of the source: a block by `y`, then `z`, then `x`. The parent of an entity gives its
    /// digest.
    #[must_use]
    pub fn packed(&self) -> u128 {
        match *self {
            Self::Player { order } => u128::from(order),
            Self::Block(pos) => {
                u128::from(ordered(pos.y)) << 64
                    | u128::from(ordered(pos.z)) << 32
                    | u128::from(ordered(pos.x))
            }
            Self::Entity(parent) => parent.digest(),
        }
    }
}

/// Maps a signed value to an unsigned one in the same order.
fn ordered(value: i32) -> u32 {
    value.cast_unsigned() ^ 0x8000_0000
}
