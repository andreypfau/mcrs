use glam::IVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::falling_block_table::FallingBlockTable;
use crate::target_cell::TargetCell;
use crate::water_contact::WaterContact;

const WATER_SIDES: [IVec3; 5] = [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Replaceable {
    Replaceable,
    Occupied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Support {
    Stays,
    Fails,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterBeside {
    Absent,
    Present,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandingSite {
    pub target: BlockStateId,
    pub below: BlockStateId,
    pub replaceable: Replaceable,
    pub support: Support,
    pub water_contact: WaterContact,
    pub water_beside: WaterBeside,
}

impl LandingSite {
    #[must_use]
    pub fn read(
        table: &FallingBlockTable,
        definitions: &BlockDefinitions,
        state_at: impl Fn(BlockPos) -> Option<BlockStateId>,
        cell: TargetCell,
        replaceable: impl FnOnce(BlockStateId) -> Replaceable,
    ) -> Option<Self> {
        let target = state_at(cell.pos)?;
        let below = state_at(cell.pos - IVec3::Y)?;
        let open_water = |offset: IVec3| {
            state_at(cell.pos + offset).is_some_and(|state| {
                table.holds_water(state)
                    && definitions
                        .shape(definitions.state(state).collision_shape)
                        .is_empty()
            })
        };
        let water_beside = if WATER_SIDES.into_iter().any(open_water) {
            WaterBeside::Present
        } else {
            WaterBeside::Absent
        };
        Some(Self {
            target,
            below,
            replaceable: replaceable(target),
            support: Support::Stays,
            water_contact: cell.water,
            water_beside,
        })
    }
}
