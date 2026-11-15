use bevy_ecs::change_detection::{DetectChanges, Ref};
use mcrs_minecraft_entity_visual::Age;

/// Whether the entity ticked since the last run of the reader: the tick writes the age only of an
/// entity that ticks, and the age of an entity that never ticked is 0.
pub(crate) fn ticked(age: &Ref<Age>) -> bool {
    age.is_changed() && age.0 > 0
}
