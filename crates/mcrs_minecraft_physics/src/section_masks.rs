use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;

use crate::BlockFlags;
use crate::CollisionTable;

/// Per section, one bit for each brick of 4 × 4 × 4 blocks: a fast test that lets the block scan
/// skip the bricks that cannot give a collider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionMasks {
    /// A state with a collision shape, a context shape, or a moving piston.
    pub solid: u64,
    /// Every block is air.
    pub air_only: bool,
    /// A state whose shape the block scan reads out of its cell: a large shape, or a moving
    /// piston, whose shape is the moved block at its progress and so can leave the cell.
    pub large: bool,
    /// A state with an inside effect, an inside action or a fall reset.
    pub inside: bool,
}

impl SectionMasks {
    /// The masks of a section of air.
    pub const EMPTY: Self = Self {
        solid: 0,
        air_only: true,
        large: false,
        inside: false,
    };

    /// The masks of a section from the state of each block.
    #[must_use]
    pub fn from_states(table: &CollisionTable, state: impl Fn(LocalPos) -> BlockStateId) -> Self {
        let mut masks = Self::EMPTY;
        for local in LocalPos::all() {
            let id = state(local);
            masks.add(table, id, 1_u64 << brick(local));
        }
        masks
    }

    /// The masks of a section from the distinct states in it: each brick is solid when one of
    /// the states is.
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

    /// True when the brick of the block can give a collider.
    #[must_use]
    pub fn may_collide(&self, at: LocalPos) -> bool {
        self.solid >> brick(at) & 1 == 1
    }
}

/// The index of the brick of a block: `x / 4 + z / 4 × 4 + y / 4 × 16`.
#[must_use]
pub fn brick(at: LocalPos) -> u32 {
    u32::from(at.x() >> 2) | u32::from(at.z() >> 2) << 2 | u32::from(at.y() >> 2) << 4
}
