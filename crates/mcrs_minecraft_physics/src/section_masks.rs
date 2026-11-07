use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;

use crate::CollisionTable;

/// Per section, one bit for each brick of 4 × 4 × 4 blocks: a fast test that lets the block scan
/// skip the bricks that cannot give a collider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionMasks {
    /// A state with a collision shape, a context shape, or a moving piston.
    pub solid: u64,
    /// Every block is air.
    pub air_only: bool,
}

impl SectionMasks {
    /// The masks of a section from the state of each block.
    #[must_use]
    pub fn from_states(table: &CollisionTable, state: impl Fn(LocalPos) -> BlockStateId) -> Self {
        let mut masks = Self {
            solid: 0,
            air_only: true,
        };
        for local in LocalPos::all() {
            let id = state(local);
            if id != BlockStateId::AIR {
                masks.air_only = false;
            }
            if table.state(id).is_solid() {
                masks.solid |= 1_u64 << brick(local);
            }
        }
        masks
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
