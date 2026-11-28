use bevy_ecs::lifecycle::Discard;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_engine_entity_motion_replication::MotionId;
use mcrs_minecraft_engine_entity_motion_replication::PublishedTile;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_item_entity::ItemEntity;

use crate::Owner;
use crate::Thrower;
use crate::publish_items::Release;
use crate::publish_items::Released;

pub(crate) fn release_motion(
    replaced: On<Discard<Dropped>>,
    stacks: Query<(&MotionId, &PublishedTile)>,
    mut released: ResMut<Released>,
    mut commands: Commands,
) {
    if let Ok((id, published)) = stacks.get(replaced.entity)
        && let Some(tile) = published.tile()
    {
        released.0.push(Release { id: id.0, tile });
    }
    commands
        .entity(replaced.entity)
        .try_remove::<(MotionState, MotionKind, Appearance)>();
}

pub(crate) fn forget_item_values(removed: On<Remove<Dropped>>, mut commands: Commands) {
    commands
        .entity(removed.entity)
        .try_remove::<(ItemEntity, Owner, Thrower)>();
}
