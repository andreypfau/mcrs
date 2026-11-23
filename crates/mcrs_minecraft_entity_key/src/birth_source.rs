use mcrs_voxel::BlockPos;

use crate::EntityUuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BirthSource {
    Command { spawn: BlockPos, source: BlockPos },
    Block(BlockPos),
    Entity(EntityUuid),
}

impl BirthSource {
    pub(crate) const fn kind(&self) -> u64 {
        match self {
            Self::Command { .. } => 2,
            Self::Block(_) => 4,
            Self::Entity(_) => 5,
        }
    }

    pub(crate) fn words(&self) -> [u64; 4] {
        let [first, second] = match *self {
            Self::Command { spawn, source } => [block(spawn), block(source)],
            Self::Block(pos) => [block(pos), 0],
            Self::Entity(parent) => [parent.as_u128(), 0],
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
