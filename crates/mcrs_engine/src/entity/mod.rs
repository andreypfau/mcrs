use crate::entity::physics::{OldTransform, Transform};
use crate::entity::player::Player;
use crate::world::chunk::{Chunk, ChunkIndex, ChunkPos};
use crate::world::dimension::{Dimension, DimensionPlayers, InDimension, OldInDimension};
use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Plugin};
use bevy_derive::Deref;
use bevy_ecs::entity::EntityHashSet;
use bevy_ecs::prelude::{
    Added, Commands, Component, ContainsEntity, DetectChanges, Entity, EntityEvent, Has,
    IntoScheduleConfigs, Local, On, ParallelCommands, Query, Ref, With, Without,
};
use bevy_ecs::relationship::RelationshipSourceCollection;
use std::time::Instant;

pub mod despawn;
pub mod physics;
pub mod player;

pub struct EntityPlugin;

impl Plugin for EntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPreUpdate,
            (add_entity_to_chunk, add_player_synced_entities),
        );
        app.add_systems(FixedUpdate, (tick_chunk_entities, sync_entities));
        app.add_systems(
            FixedPostUpdate,
            (
                remove_entity_despawned,
                add_old_transform,
                update_chunk_entities,
                update_old_transforms,
            )
                .chain(),
        );
        app.add_observer(synced_entity_added);
        app.add_observer(synced_entity_removed);
    }
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct EntityNetworkSync;

#[derive(Component, Default, Deref)]
struct PlayerSynchronizedEntities(EntityHashSet);

#[allow(dead_code)]
#[derive(EntityEvent, Debug)]
struct EntityAddedToChunkEvent {
    pub entity: Entity,
    pub chunk: Entity,
    pub chunk_pos: ChunkPos,
    pub dimension: Entity,
}

#[allow(dead_code)]
#[derive(EntityEvent, Debug)]
struct EntityRemovedFromChunkEvent {
    pub entity: Entity,
    pub chunk: Entity,
    pub chunk_pos: ChunkPos,
    pub dimension: Entity,
}

#[allow(dead_code)]
#[derive(EntityEvent, Debug)]
struct EntityChunkMovedEvent {
    pub entity: Entity,
    pub old_chunk_pos: ChunkPos,
    pub new_chunk_pos: ChunkPos,
    pub dimension: Entity,
}

#[derive(Component, Debug, Default, Deref)]
pub struct ChunkEntities(Vec<Entity>);

impl ChunkEntities {
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        if let Some(pos) = self.0.iter().position(|e| e == entity) {
            self.0.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn add_entity(&mut self, entity: Entity) -> bool {
        if !self.0.contains(&entity) {
            self.0.push(entity);
            true
        } else {
            false
        }
    }
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Despawned;

#[allow(clippy::type_complexity)]
fn add_entity_to_chunk(
    dim_chunks: Query<&ChunkIndex>,
    mut chunks: Query<&mut ChunkEntities>,
    entities: Query<
        (Entity, &InDimension, &Transform),
        (
            Without<Dimension>,
            Without<Chunk>,
            Without<OldTransform>,
            Without<Despawned>,
        ),
    >,
) {
    entities
        .iter()
        .for_each(|(entity, in_dimension, transform)| {
            let Ok(chunk_index) = dim_chunks.get(in_dimension.entity()) else {
                return;
            };
            let chunk_pos = ChunkPos::from(transform.translation);
            let Some(chunk) = chunk_index.get(chunk_pos) else {
                return;
            };
            let Ok(mut chunk_entities) = chunks.get_mut(chunk) else {
                return;
            };
            chunk_entities.add_entity(entity);
        });
}

#[allow(clippy::type_complexity)]
fn add_old_transform(
    entities: Query<
        (Entity, &Transform),
        (
            Without<Dimension>,
            Without<Chunk>,
            Without<OldTransform>,
            Without<Despawned>,
        ),
    >,
    mut commands: Commands,
) {
    for (entity, transform) in entities.iter() {
        commands.entity(entity).insert(OldTransform(*transform));
    }
}

#[allow(clippy::type_complexity)]
fn update_chunk_entities(
    dim_chunks: Query<&ChunkIndex>,
    mut chunks: Query<&mut ChunkEntities>,
    entities: Query<
        (
            Entity,
            &InDimension,
            Ref<Transform>,
            &OldTransform,
            &OldInDimension,
        ),
        (Without<Dimension>, Without<Chunk>, Without<Despawned>),
    >,
    mut commands: Commands,
) {
    for (entity, dimension, transform, old_transform, old_dimension) in entities.iter() {
        let old_dim = old_dimension.entity();
        let new_dim = dimension.entity();

        let dimension_changed = old_dim != new_dim;
        if !dimension_changed && !transform.is_changed() {
            continue;
        }

        let old_pos = ChunkPos::from(old_transform.translation);
        let new_pos = ChunkPos::from(transform.translation);
        let pos_changed = old_pos != new_pos;

        if (dimension_changed || pos_changed)
            && let Ok(old_index) = dim_chunks.get(old_dim)
            && let Some(old_chunk) = old_index.get(old_pos)
            && let Ok(mut chunk_entities) = chunks.get_mut(old_chunk)
            && chunk_entities.remove_entity(entity)
        {
            commands.trigger(EntityRemovedFromChunkEvent {
                entity,
                chunk: old_chunk,
                chunk_pos: old_pos,
                dimension: old_dim,
            });
        }

        let Ok(new_index) = dim_chunks.get(new_dim) else {
            continue;
        };
        if let Some(new_chunk) = new_index.get(new_pos)
            && let Ok(mut chunk_entities) = chunks.get_mut(new_chunk)
            && chunk_entities.add_entity(entity)
        {
            commands.trigger(EntityAddedToChunkEvent {
                entity,
                chunk: new_chunk,
                chunk_pos: new_pos,
                dimension: new_dim,
            });
        }
    }
}

fn remove_entity_despawned(
    dims_chunks: Query<&ChunkIndex>,
    mut chunks: Query<&mut ChunkEntities>,
    despawned: Query<(Entity, &InDimension, &Transform), With<Despawned>>,
    mut commands: Commands,
) {
    despawned
        .iter()
        .for_each(|(entity, in_dimension, transform)| {
            let dimension = in_dimension.entity();
            let Ok(chunk_index) = dims_chunks.get(dimension) else {
                return;
            };
            let chunk_pos = ChunkPos::from(transform.translation);
            let Some(chunk) = chunk_index.get(chunk_pos) else {
                return;
            };
            let Ok(mut chunk_entities) = chunks.get_mut(chunk) else {
                return;
            };
            if chunk_entities.remove_entity(entity) {
                commands.trigger(EntityRemovedFromChunkEvent {
                    entity,
                    chunk,
                    chunk_pos,
                    dimension,
                });
            }
        });
}

#[allow(clippy::type_complexity)]
fn tick_chunk_entities(
    entities: Query<
        (
            Entity,
            &OldTransform,
            Ref<Transform>,
            &InDimension,
            &OldInDimension,
        ),
        Without<Despawned>,
    >,
    mut commands: Commands,
) {
    entities
        .iter()
        .for_each(|(entity, old_transform, transform, dim, old_dim)| {
            if !transform.is_changed() {
                return;
            }
            let dimension = dim.entity();
            if dimension != old_dim.entity() {
                return;
            }
            let old_chunk_pos = ChunkPos::from(old_transform.translation);
            let new_chunk_pos = ChunkPos::from(transform.translation);
            let pos_changed = old_chunk_pos != new_chunk_pos;
            if pos_changed {
                commands.trigger(EntityChunkMovedEvent {
                    entity,
                    old_chunk_pos,
                    new_chunk_pos,
                    dimension,
                });
            }
        });
}

fn update_old_transforms(mut entities: Query<(&mut OldTransform, &Transform), Without<Despawned>>) {
    entities
        .iter_mut()
        .for_each(|(mut old_transform, transform)| {
            old_transform.0 = *transform;
        });
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkSyncEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkAddEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkRemoveEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[allow(clippy::type_complexity)]
fn add_player_synced_entities(
    mut commands: Commands,
    new_players: Query<Entity, (With<Player>, Added<InDimension>, Without<Despawned>)>,
) {
    new_players.iter().for_each(|entity| {
        commands
            .entity(entity)
            .insert(PlayerSynchronizedEntities::default());
    });
}

#[allow(clippy::type_complexity)]
fn sync_entities(
    dim_players: Query<&DimensionPlayers>,
    player_data: Query<
        (&Transform, &PlayerSynchronizedEntities),
        (With<Player>, Without<Despawned>),
    >,
    entities: Query<(
        Entity,
        &InDimension,
        Ref<Transform>,
        Has<Despawned>,
        Has<EntityNetworkSync>,
    )>,
    commands: ParallelCommands,
    mut last_force_sync: Local<Option<Instant>>,
) {
    let need_force_sync = match *last_force_sync {
        Some(last) => last.elapsed().as_secs_f32() > 3.0,
        None => true,
    };
    if need_force_sync {
        *last_force_sync = Some(Instant::now());
    }

    entities
        .par_iter()
        .for_each(|(entity, in_dimension, transform, is_removed, need_sync)| {
            let Ok(players) = dim_players.get(in_dimension.entity()) else {
                return;
            };
            players.iter().for_each(|&player| {
                if player == entity {
                    return;
                }
                let Ok((player_transform, synced_entities)) = player_data.get(player) else {
                    return;
                };
                let has_spawned = synced_entities.contains(&entity);
                let too_far =
                    || (transform.translation - player_transform.translation).length() > 16.0;

                if has_spawned {
                    if is_removed || too_far() {
                        commands.command_scope(|mut cmds| {
                            cmds.trigger(EntityNetworkRemoveEvent { entity, player });
                        });
                    } else if need_force_sync || need_sync || transform.is_changed() {
                        commands.command_scope(|mut cmds| {
                            cmds.trigger(EntityNetworkSyncEvent { entity, player });
                        });
                    }
                } else if !too_far() {
                    commands.command_scope(|mut cmds| {
                        cmds.trigger(EntityNetworkAddEvent { entity, player });
                    });
                }
            });
            if need_sync {
                commands.command_scope(|mut cmds| {
                    cmds.entity(entity).remove::<EntityNetworkSync>();
                });
            }
        });
}

fn synced_entity_added(
    event: On<EntityNetworkAddEvent>,
    mut player_data: Query<&mut PlayerSynchronizedEntities, With<Player>>,
) {
    let Ok(mut synced_entities) = player_data.get_mut(event.player) else {
        return;
    };
    synced_entities.0.add(event.entity);
}

fn synced_entity_removed(
    event: On<EntityNetworkRemoveEvent>,
    mut player_data: Query<&mut PlayerSynchronizedEntities, With<Player>>,
) {
    let Ok(mut synced_entities) = player_data.get_mut(event.player) else {
        return;
    };
    synced_entities.0.remove(event.entity);
}
