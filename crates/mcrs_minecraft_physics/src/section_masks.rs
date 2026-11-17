use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;

use crate::BlockFlags;
use crate::CollisionTable;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionMasks {
    pub solid: u64,
    pub air_only: bool,
    pub large: bool,
    pub inside: bool,
}

impl SectionMasks {
    pub const EMPTY: Self = Self {
        solid: 0,
        air_only: true,
        large: false,
        inside: false,
    };

    #[must_use]
    pub fn from_states(table: &CollisionTable, state: impl Fn(LocalPos) -> BlockStateId) -> Self {
        let mut masks = Self::EMPTY;
        for local in LocalPos::all() {
            let id = state(local);
            masks.add(table, id, 1_u64 << brick(local));
        }
        masks
    }

    #[must_use]
    pub fn from_palette(
        table: &CollisionTable,
        states: impl IntoIterator<Item = BlockStateId>,
    ) -> Self {
        let mut masks = Self::EMPTY;
        for id in states {
            masks.add(table, id, u64::MAX);
        }
        masks
    }

    fn add(&mut self, table: &CollisionTable, id: BlockStateId, bricks: u64) {
        let data = table.state(id);
        self.air_only &= id == BlockStateId::AIR;
        if data.is_solid() {
            self.solid |= bricks;
        }
        self.large |= data
            .flags
            .intersects(BlockFlags::LARGE | BlockFlags::MOVING_PISTON);
        self.inside |= data.acts_inside();
    }

    #[must_use]
    pub fn may_collide(&self, at: LocalPos) -> bool {
        self.solid >> brick(at) & 1 == 1
    }
}

#[must_use]
pub fn brick(at: LocalPos) -> u32 {
    u32::from(at.x() >> 2) | u32::from(at.z() >> 2) << 2 | u32::from(at.y() >> 2) << 4
}
