use mcrs_voxel::BlockPos;

use crate::EntityKey;

/// What brings an entity into existence: an input of [`EntityKey::born`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BirthSource {
    /// A command effect, by the cell of the dimension that runs the command and the index of the
    /// effect in the command effect buffer of that cell in the tick.
    Command {
        cell: u32,
        order: u32,
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

    /// The source key: the cell and the order of a command, a block position by `y`, then `z`, then `x`, or the
    /// UUID of the parent.
    pub(crate) fn packed(&self) -> u128 {
        match *self {
            Self::Command { cell, order } => u128::from(cell) << 32 | u128::from(order),
            Self::Block(pos) => {
                u128::from(pos.y.cast_unsigned()) << 64
                    | u128::from(pos.z.cast_unsigned()) << 32
                    | u128::from(pos.x.cast_unsigned())
            }
            Self::Entity(parent) => parent.0,
        }
    }
}
