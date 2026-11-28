use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Slots;

pub(crate) fn clear_first_slot(
    holder: Entity,
    holders: &Query<&Slots>,
    mut commands: Commands<'_, '_>,
) {
    if let Some(stack) = holders.get(holder).ok().and_then(|slots| slots.get(0)) {
        commands.entity(stack).despawn();
    }
}
