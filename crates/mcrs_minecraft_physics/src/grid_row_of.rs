use mcrs_minecraft_entity_key::EntityId;

use crate::EntityState;
use crate::GridFlags;
use crate::GridRow;
use crate::MotionRow;
use crate::PushTargets;
use crate::TeamSlot;

#[must_use]
pub fn grid_row(state: &EntityState, motion: &MotionRow, id: EntityId) -> Option<GridRow> {
    let living = state.living?;
    let dimensions = motion.dimensions.scaled(living.attributes.scale as f32);
    let position = state.body.position;
    let previous = if state.last_path.end == position {
        state.last_path.start
    } else {
        position
    };
    Some(GridRow {
        position,
        previous,
        half_width: dimensions.width / 2.0,
        height: dimensions.height,
        key: state.key,
        id,
        root_vehicle: id,
        team: TeamSlot::default(),
        flags: if living.is_alive() {
            GridFlags::PUSHABLE
        } else {
            GridFlags::empty()
        },
        push_targets: PushTargets::Each,
    })
}
