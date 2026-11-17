use glam::DVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_physics::block_at;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::fall::Fall;
use crate::falling_block_table::FallingBlockTable;
use crate::water_contact::WaterContact;
use crate::water_contact::WaterSweep;

const SAMPLES_PER_BLOCK: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetCell {
    pub pos: BlockPos,
    pub water: WaterContact,
}

impl TargetCell {
    #[must_use]
    pub fn find(
        table: &FallingBlockTable,
        definitions: &BlockDefinitions,
        state_at: impl Fn(BlockPos) -> Option<BlockStateId>,
        fall: &Fall,
        start: DVec3,
        end: DVec3,
        velocity: DVec3,
    ) -> Self {
        let cell = block_at(end);
        if !table.is_concrete_powder(fall.carried) {
            return Self {
                pos: cell,
                water: WaterContact::Dry,
            };
        }
        let water = if state_at(cell).is_some_and(|state| table.holds_water(state)) {
            WaterContact::Wet
        } else {
            WaterContact::Dry
        };
        let in_cell = Self { pos: cell, water };
        if WaterSweep::of(velocity) == WaterSweep::Cell {
            return in_cell;
        }
        let samples = (start.distance(end) * SAMPLES_PER_BLOCK).ceil().max(1.0) as u32;
        let mut last = None;
        for sample in 0..=samples {
            let pos = block_at(start.lerp(end, f64::from(sample) / f64::from(samples)));
            if last == Some(pos) {
                continue;
            }
            last = Some(pos);
            let Some(state) = state_at(pos) else {
                continue;
            };
            if definitions
                .state(state)
                .fluid
                .is_some_and(|fluid| fluid.source)
            {
                return if table.holds_water(state) {
                    Self {
                        pos,
                        water: WaterContact::Wet,
                    }
                } else {
                    in_cell
                };
            }
        }
        in_cell
    }
}
