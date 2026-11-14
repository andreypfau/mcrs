use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::system::{Query, Res, ResMut};
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::{AttributeBases, AttributeSet, InstanceChange};

use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::attribute_target::AttributeTarget;
use crate::attributes::{Attributes, player_bases};
use crate::instance_edit::InstanceEdit;
use crate::row_attributes::RowAttributes;

/// Applies each edit in arrival order and announces each instance that it changed.
pub(crate) fn apply_attribute_edits(
    mut edits: MessageReader<AttributeEdit>,
    mut changed: MessageWriter<AttributeChanged>,
    mut players: Query<&mut Attributes>,
    mut rows: ResMut<RowAttributes>,
    mobs: Res<Mobs>,
) {
    for edit in edits.read() {
        let change = match edit.target {
            AttributeTarget::Player(entity) => {
                let (Some(bases), Ok(mut attributes)) = (player_bases(), players.get_mut(entity))
                else {
                    continue;
                };
                let change = apply(
                    &mut attributes.bypass_change_detection().set,
                    bases,
                    edit.attribute,
                    &edit.edit,
                );
                if change == InstanceChange::Changed {
                    attributes.set_changed();
                }
                change
            }
            AttributeTarget::Row(id) => {
                let Some(bases) = mobs
                    .0
                    .get(&id)
                    .and_then(|mob| AttributeBases::of(mob.kind.entity_type))
                else {
                    continue;
                };
                if let Some(set) = rows.sets.get_mut(&id) {
                    let change = apply(set, bases, edit.attribute, &edit.edit);
                    if set.instances().next().is_none() {
                        rows.sets.remove(&id);
                    }
                    change
                } else {
                    let mut set = AttributeSet::new();
                    let change = apply(&mut set, bases, edit.attribute, &edit.edit);
                    if change == InstanceChange::Changed {
                        rows.sets.insert(id, set);
                    }
                    change
                }
            }
        };
        if change == InstanceChange::Changed {
            changed.write(AttributeChanged {
                target: edit.target,
                attribute: edit.attribute,
            });
        }
    }
}

/// An edit of an attribute outside the set of the type leaves the instances as they are
/// (ATR-SRC-001).
fn apply(
    set: &mut AttributeSet,
    bases: AttributeBases,
    attribute: Attribute,
    edit: &InstanceEdit,
) -> InstanceChange {
    let result = match edit {
        InstanceEdit::SetBase(base) => set.set_base(bases, attribute, *base),
        InstanceEdit::Put(modifier) => set.set_modifier(bases, attribute, modifier.clone()),
        InstanceEdit::Remove(id) => set.remove_modifier(bases, attribute, id.as_location()),
    };
    result.unwrap_or(InstanceChange::Unchanged)
}
