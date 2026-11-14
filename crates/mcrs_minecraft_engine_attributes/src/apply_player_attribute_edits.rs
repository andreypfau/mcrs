use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::Query;
use mcrs_minecraft_entity::InstanceChange;

use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::attribute_target::AttributeTarget;
use crate::attributes::{Attributes, player_bases};

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
        let (Some(bases), Ok(mut attributes)) = (player_bases(), players.get_mut(entity)) else {
            continue;
        };
        let change = edit.edit.apply(
            &mut attributes.bypass_change_detection().set,
            bases,
            edit.attribute,
        );
        if change == InstanceChange::Changed {
            attributes.set_changed();
            changed.write(AttributeChanged {
                target: edit.target,
                attribute: edit.attribute,
            });
        }
    }
}
