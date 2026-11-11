use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Query, ResMut, Without};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity_ids::{CellEntityIds, GlobalEntityId};

use crate::minecraft_entity::MinecraftEntity;

/// Gives a new entity that is not a player a global id from the ranges of the cell. An entity
/// that arrives from another cell brings its id and keeps it.
pub(crate) fn assign_global_entity_id(
    add: On<Add<MinecraftEntity>>,
    without_id: Query<(), (Without<Player>, Without<GlobalEntityId>)>,
    mut ids: ResMut<CellEntityIds>,
    mut commands: Commands,
) {
    let entity = add.event().entity;
    if !without_id.contains(entity) {
        return;
    }
    let Some(id) = ids.allocate() else {
        tracing::warn!(
            ?entity,
            "the cell used all its entity ids before the next range arrived; the entity has no global id"
        );
        return;
    };
    commands.entity(entity).insert(GlobalEntityId(id));
}
