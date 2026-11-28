use bevy_ecs::change_detection::{DetectChanges, Ref};
use mcrs_minecraft_entity_visual::Age;

pub(crate) fn ticked(age: &Ref<Age>) -> bool {
    age.is_changed() && age.0 > 0
}
