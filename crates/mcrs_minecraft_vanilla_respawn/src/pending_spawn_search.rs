use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::{Commands, Query};
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::{CaptureCause, PersistedError};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::spawn_site::{ArrivedSpawn, SpawnSite};

#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub(crate) struct PendingSpawnSearch;

pub(crate) const PENDING_SPAWN_SEARCH_KEYS: &[&str] = &["mcrs:pending_spawn_search"];

pub(crate) fn capture_pending_spawn_search(
    world: &mut World,
    player: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    Ok(world
        .get::<PendingSpawnSearch>(player)
        .map(|_| NbtTag::Byte(1)))
}

pub(crate) fn apply_pending_spawn_search(
    world: &mut World,
    player: Entity,
    _: &NbtTag,
) -> Result<(), PersistedError> {
    if let Ok(mut player) = world.get_entity_mut(player) {
        player.insert(PendingSpawnSearch);
    }
    Ok(())
}

pub(crate) fn finish_spawn_searches(
    mut players: Query<(Entity, &mut Transform), With<PendingSpawnSearch>>,
    site: SpawnSite,
    mut commands: Commands,
) {
    let arrived = site.arrived_spawn();
    if let ArrivedSpawn::Waiting = arrived {
        return;
    }
    for (player, mut transform) in &mut players {
        if let ArrivedSpawn::At(spawn) = arrived {
            transform.set_if_neq(spawn);
        }
        commands.entity(player).remove::<PendingSpawnSearch>();
    }
}
