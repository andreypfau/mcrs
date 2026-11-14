use glam::IVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::falling_block_table::FallingBlockTable;
use crate::target_cell::TargetCell;
use crate::water_contact::WaterContact;

/// The neighbours of a target cell whose water hardens a concrete powder (FBK-LND-004).
const WATER_SIDES: [IVec3; 5] = [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

/// Whether the state at the target cell is replaceable at a landing (FBK-LND-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Replaceable {
    Replaceable,
    Occupied,
}

/// Whether the carried state can stay at the target cell by its support predicate (FBK-LND-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Support {
    Stays,
    Fails,
}

/// Whether a neighbour above the target cell or on one of its four horizontal sides holds water of
/// the tag `minecraft:water`, with a face toward the target cell that is not sturdy (FBK-LND-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterBeside {
    Absent,
    Present,
}

/// What the host reads from the world at the target cell for a landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandingSite {
    /// The state at the target cell.
    pub target: BlockStateId,
    /// The state below the target cell.
    pub below: BlockStateId,
    pub replaceable: Replaceable,
    pub support: Support,
    pub water_contact: WaterContact,
    pub water_beside: WaterBeside,
}

impl LandingSite {
    /// What a landing at `cell` reads through `state_at`, or `None` when a block it reads is not
    /// loaded. `replaceable` is the test of FBK-LND-003 for the state at the target cell.
    // chisle: each carried state stays, which a support predicate of SUP-PRED-001 lifts. Only a
    // neighbour with no collision shape counts as water beside, so a waterlogged bottom slab beside
    // the target does not harden a powder; a table of the full face test of SUP-FACE-001 over a
    // state and a direction, in a crate below this one, lifts it.
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
