use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::Query;
use mcrs_minecraft_entity_attribute::InstanceChange;

use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::attribute_target::AttributeTarget;
use crate::attributes::Attributes;

/// Applies each edit of a player in arrival order and announces each instance that it changed.
/// A system that applies the edits of another target writes [`AttributeChanged`] too, so it runs
/// after this one.
pub(crate) fn apply_player_attribute_edits(
    mut edits: MessageReader<AttributeEdit>,
    mut changed: MessageWriter<AttributeChanged>,
    mut players: Query<&mut Attributes>,
) {
    for edit in edits.read() {
        let AttributeTarget::Player(entity) = edit.target else {
            continue;
        };
        let Ok(mut attributes) = players.get_mut(entity) else {
            continue;
        };
        let change = attributes
            .bypass_change_detection()
            .instances
            .apply(edit.attribute, &edit.edit);
        if change == InstanceChange::Changed {
            attributes.set_changed();
            changed.write(AttributeChanged {
                target: edit.target,
                attribute: edit.attribute,
            });
        }
    }
}
