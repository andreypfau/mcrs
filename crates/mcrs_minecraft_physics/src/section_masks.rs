use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;

use crate::BlockFlags;
use crate::CollisionTable;
use crate::ShapeClass;

/// Per section, one bit for each brick of 4 × 4 × 4 blocks: a fast test that lets the block scan
/// skip the bricks that cannot give a collider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionMasks {
    /// A state with a collision shape, a context shape, or a moving piston.
    pub solid: u64,
    /// Every block is a full cube.
    pub full: u64,
    /// A state with a large shape.
    pub large: u64,
    /// A state with a step action or an inside action.
    pub action: u64,
    /// A state that can make a tick coarse.
    pub coarse: u64,
    /// The block change counter of the section.
    pub revision: u64,
    /// The number of states with a context shape.
    pub special: u16,
    /// Every block is air.
    pub air_only: bool,
}

impl SectionMasks {
    /// The masks of a section from the state of each block.
    #[must_use]
    pub fn from_states(
        table: &CollisionTable,
        revision: u64,
        state: impl Fn(LocalPos) -> BlockStateId,
    ) -> Self {
        let mut masks = Self {
            full: u64::MAX,
            revision,
            air_only: true,
            ..Self::default()
        };
        for local in LocalPos::all() {
            let id = state(local);
            let data = table.state(id);
            let bit = 1_u64 << brick(local);
            if id != BlockStateId::AIR {
                masks.air_only = false;
            }
            if data.class != ShapeClass::FullCube {
                masks.full &= !bit;
            }
            if data.class != ShapeClass::Empty || data.flags.contains(BlockFlags::MOVING_PISTON) {
                masks.solid |= bit;
            }
            if matches!(
                data.class,
                ShapeClass::Scaffolding { .. } | ShapeClass::PowderSnow
            ) {
                masks.special = masks.special.saturating_add(1);
            }
            if data.flags.contains(BlockFlags::LARGE) {
                masks.large |= bit;
            }
            if data
                .flags
                .intersects(BlockFlags::STEP_ACTION | BlockFlags::INSIDE_ACTION)
            {
                masks.action |= bit;
            }
            if data.flags.intersects(
                BlockFlags::FLUID
                    | BlockFlags::OFF_GRID
                    | BlockFlags::COARSE_NEAR
                    | BlockFlags::COARSE_BUBBLE_COLUMN
                    | BlockFlags::COARSE_POTENT_SULFUR
                    | BlockFlags::COARSE_PISTON,
            ) {
                masks.coarse |= bit;
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
