use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::{Res, ResMut};
use mcrs_minecraft_engine_attributes::{AttributeChanged, AttributeEdit, AttributeTarget};
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity::{AttributeBases, AttributeSet, InstanceChange};

use crate::row_attributes::RowAttributes;

/// Applies each edit of a mob row in arrival order and announces each instance that it changed.
pub(crate) fn apply_row_attribute_edits(
    mut edits: MessageReader<AttributeEdit>,
    mut changed: MessageWriter<AttributeChanged>,
    mut rows: ResMut<RowAttributes>,
    mobs: Res<Mobs>,
) {
    for edit in edits.read() {
        let AttributeTarget::Row(id) = edit.target else {
            continue;
        };
        let Some(bases) = mobs
            .0
            .get(&id)
            .and_then(|mob| AttributeBases::of(mob.kind.entity_type))
        else {
            continue;
        };
        let change = if let Some(set) = rows.sets.get_mut(&id) {
            let change = edit.edit.apply(set, bases, edit.attribute);
            if set.instances().next().is_none() {
                rows.sets.remove(&id);
            }
            change
        } else {
            let mut set = AttributeSet::new();
            let change = edit.edit.apply(&mut set, bases, edit.attribute);
            if change == InstanceChange::Changed {
                rows.sets.insert(id, set);
            }
            change
        };
        if change == InstanceChange::Changed {
            changed.write(AttributeChanged {
                target: edit.target,
                attribute: edit.attribute,
            });
        }
    }
}
