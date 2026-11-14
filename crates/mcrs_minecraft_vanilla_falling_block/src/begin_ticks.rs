use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_falling_block::TickStart;

use crate::Falling;

/// Step 1 of FBK-TCK-001 before the motion pass: a falling block of `air` ends with no drop.
pub(crate) fn begin_ticks(mut commands: Commands, falls: Query<(Entity, &Falling)>) {
    for (entity, Falling(fall)) in &falls {
        if fall.begin_tick() == TickStart::Remove {
            commands.entity(entity).despawn();
        }
    }
}
