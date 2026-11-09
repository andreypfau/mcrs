use mcrs_voxel::BlockPos;

use crate::EntityKey;

/// What brings an entity into existence: an input of [`EntityKey::born`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BirthSource {
    /// A command, by the block of the spawn point and the block of the command source. The
    /// column of the source belongs to one cell, which runs the command, so two cells never give
    /// one pair in one tick.
    Command {
        spawn: BlockPos,
        source: BlockPos,
    },
    Block(BlockPos),
    /// Another entity, by its UUID.
    Entity(EntityKey),
}

impl BirthSource {
    /// The number of the kind of the source: 2 for a command, 4 for a block and 5 for an entity.
    pub(crate) const fn kind(&self) -> u64 {
        match self {
            Self::Command { .. } => 2,
            Self::Block(_) => 4,
            Self::Entity(_) => 5,
        }
    }

    /// The source key as four words: block positions by `y`, then `z`, then `x`, or the UUID of
    /// the parent, with zero words after a key of one value.
    pub(crate) fn words(&self) -> [u64; 4] {
        let [first, second] = match *self {
            Self::Command { spawn, source } => [block(spawn), block(source)],
            Self::Block(pos) => [block(pos), 0],
            Self::Entity(parent) => [parent.0, 0],
        };
        [
            (first >> 64) as u64,
            first as u64,
            (second >> 64) as u64,
            second as u64,
        ]
    }
}

fn block(pos: BlockPos) -> u128 {
    u128::from(pos.y.cast_unsigned()) << 64
        | u128::from(pos.z.cast_unsigned()) << 32
        | u128::from(pos.x.cast_unsigned())
}
