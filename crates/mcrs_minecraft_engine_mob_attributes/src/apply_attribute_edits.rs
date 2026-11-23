use bevy_ecs::change_detection::{DetectChangesMut, Mut};
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_entity_attribute::{AttributeInstances, InstanceChange};

use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::mob_attributes::MobAttributes;

pub(crate) fn apply_attribute_edits(
    mut edits: MessageReader<AttributeEdit>,
    mut changed: MessageWriter<AttributeChanged>,
    mut mobs: Query<(&Mob, &mut MobAttributes)>,
    mut instances: Query<&mut AttributeInstances>,
) {
    for edit in edits.read() {
        let change = if let Ok((mob, attributes)) = mobs.get_mut(edit.target) {
            edit_mob(mob, attributes, edit)
        } else if let Ok(target) = instances.get_mut(edit.target) {
            edit_instances(target, edit)
        } else {
            continue;
        };
        if change == InstanceChange::Changed {
            changed.write(AttributeChanged {
                target: edit.target,
                attribute: edit.attribute,
            });
        }
    }
}

fn edit_mob(mob: &Mob, mut attributes: Mut<MobAttributes>, edit: &AttributeEdit) -> InstanceChange {
    let Some(bases) = mcrs_minecraft_entity::attribute_bases(mob.kind.entity_type) else {
        return InstanceChange::Unchanged;
    };
    let change = edit.edit.apply(
        &mut attributes.bypass_change_detection().0,
        bases,
        edit.attribute,
    );
    if change == InstanceChange::Changed {
        attributes.set_changed();
    }
    change
}

fn edit_instances(mut instances: Mut<AttributeInstances>, edit: &AttributeEdit) -> InstanceChange {
    let change = instances
        .bypass_change_detection()
        .apply(edit.attribute, &edit.edit);
    if change == InstanceChange::Changed {
        instances.set_changed();
    }
    change
}
