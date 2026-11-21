use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_entity_attribute::InstanceChange;

use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::mob_attributes::MobAttributes;

pub(crate) fn apply_attribute_edits(
    mut edits: MessageReader<AttributeEdit>,
    mut changed: MessageWriter<AttributeChanged>,
    mut mobs: Query<(&Mob, &mut MobAttributes)>,
) {
    for edit in edits.read() {
        let Ok((mob, mut attributes)) = mobs.get_mut(edit.target) else {
            continue;
        };
        let Some(bases) = mcrs_minecraft_entity::attribute_bases(mob.kind.entity_type) else {
            continue;
        };
        let set = &mut attributes.bypass_change_detection().0;
        if edit.edit.apply(set, bases, edit.attribute) == InstanceChange::Changed {
            attributes.set_changed();
            changed.write(AttributeChanged {
                target: edit.target,
                attribute: edit.attribute,
            });
        }
    }
}
