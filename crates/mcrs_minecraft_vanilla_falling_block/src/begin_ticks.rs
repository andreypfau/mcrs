use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_falling_block::TickStart;

use crate::Falling;

pub(crate) fn begin_ticks(mut commands: Commands, falls: Query<(Entity, &Falling)>) {
    for (entity, Falling(fall)) in &falls {
        if fall.begin_tick() == TickStart::Remove {
            commands.entity(entity).despawn();
        }
    }
}
