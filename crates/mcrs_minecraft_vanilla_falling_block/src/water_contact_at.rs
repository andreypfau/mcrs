use glam::DVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_falling_block::FallingBlockTable;
use mcrs_minecraft_falling_block::WaterContact;
use mcrs_minecraft_falling_block::WaterSweep;
use mcrs_minecraft_physics::block_at;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

/// Samples of the segment in each block of its length.
const SAMPLES_PER_BLOCK: f64 = 8.0;

/// The water contact of a falling block of a concrete powder and its target cell (FBK-TCK-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Contact {
    pub(crate) water: WaterContact,
    pub(crate) target: BlockPos,
}

/// The contact of a falling block that moved from `start` to `end` with `velocity` after the move.
/// `state_at` gives `None` for a block that is not loaded.
// chisle: the segment is sampled by cells and stops at any fluid source; it does not test block
// outlines or the fluid height of PRJ-TST-001. A segment test of PRJ-TST-001 in a rules crate
// lifts it.
pub(crate) fn water_contact_at(
    table: &FallingBlockTable,
    definitions: &BlockDefinitions,
    state_at: impl Fn(BlockPos) -> Option<BlockStateId>,
    start: DVec3,
    end: DVec3,
    velocity: DVec3,
) -> Contact {
    let cell = block_at(end);
    let water = if state_at(cell).is_some_and(|state| table.holds_water(state)) {
        WaterContact::Wet
    } else {
        WaterContact::Dry
    };
    let in_cell = Contact {
        water,
        target: cell,
    };
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
                Contact {
                    water: WaterContact::Wet,
                    target: pos,
                }
            } else {
                in_cell
            };
        }
    }
    in_cell
}
