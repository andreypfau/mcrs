use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_falling_block::TickStart;

use crate::FallingBlockEntity;

/// Steps 1 and 2 of FBK-TCK-001 before the motion pass: a falling block of `air` ends with no drop,
/// each other one ages by 1.
pub(crate) fn begin_ticks(
    mut commands: Commands,
    mut falls: Query<(Entity, &mut FallingBlockEntity)>,
) {
    for (entity, mut falling_block) in &mut falls {
        if falling_block.0.begin_tick() == TickStart::Remove {
            commands.entity(entity).despawn();
        }
    }
}
