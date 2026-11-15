use bevy_ecs::message::MessageReader;
use bevy_ecs::system::{Res, ResMut};
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity_rows::{Placed, RowStore};
use mcrs_minecraft_physics::Rest;

use crate::attribute_changed::AttributeChanged;
use crate::attribute_target::AttributeTarget;
use crate::motion_field::motion_field;
use crate::row_attributes::RowAttributes;

/// Writes the value of each changed motion instance of a mob row into the motion attributes of
/// its row. A row that left the world is skipped.
pub(crate) fn project_row_motion<M: Send + Sync + 'static>(
    mut changed: MessageReader<AttributeChanged>,
    attributes: Res<RowAttributes>,
    mobs: Res<Mobs>,
    mut rows: ResMut<RowStore<M>>,
) {
    for change in changed.read() {
        let AttributeTarget::Row(id) = change.target;
        let Some(field) = motion_field(change.attribute) else {
            continue;
        };
        let Some(value) = mobs
            .0
            .get(&id)
            .and_then(|mob| attributes.value(id, mob.kind.entity_type, change.attribute))
        else {
            continue;
        };
        let Some(tile) = rows.tile_of(id) else {
            continue;
        };
        let Some(state) = rows.state_mut(Placed { tile, id }) else {
            continue;
        };
        let Some(living) = state.living.as_mut() else {
            continue;
        };
        let slot = field(&mut living.attributes);
        if *slot != value {
            *slot = value;
            // A resting row skips its tick only while its inputs stay the same; a new gravity,
            // scale or step height can move it.
            state.rest = Rest::Awake;
        }
    }
}
