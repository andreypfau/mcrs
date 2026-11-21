use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::{Res, ResMut};
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity_attribute::{AttributeSet, InstanceChange};

use crate::attribute_target::AttributeTarget;
use crate::row_attribute_changed::RowAttributeChanged;
use crate::row_attribute_edit::RowAttributeEdit;
use crate::row_attributes::RowAttributes;

pub(crate) fn apply_row_attribute_edits(
    mut edits: MessageReader<RowAttributeEdit>,
    mut changed: MessageWriter<RowAttributeChanged>,
    mut rows: ResMut<RowAttributes>,
    mobs: Res<Mobs>,
) {
    for edit in edits.read() {
        let AttributeTarget::Row(id) = edit.target;
        let Some(bases) = mobs
            .0
            .get(&id)
            .and_then(|mob| mcrs_minecraft_entity::attribute_bases(mob.kind.entity_type))
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
            changed.write(RowAttributeChanged {
                target: edit.target,
                attribute: edit.attribute,
            });
        }
    }
}
