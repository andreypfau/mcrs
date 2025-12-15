use crate::entity::physics::{OldTransform, Transform};
use crate::entity::player::Player;
use crate::world::chunk::{Chunk, ChunkIndex, ChunkPos};
use crate::world::dimension::{Dimension, DimensionPlayers, InDimension, OldInDimension};
use bevy::app::{FixedPostUpdate, FixedUpdate};
use bevy::ecs::entity::EntityHashSet;
use bevy::ecs::relationship::RelationshipSourceCollection;
use bevy::prelude::{
    Added, Commands, Component, ContainsEntity, Deref, DetectChanges, Entity, EntityEvent, Has,
    IntoScheduleConfigs, Message, On, ParallelCommands, Query, Ref, With, Without,
};

pub mod despawn;
pub mod physics;
pub mod player;

pub struct EntityPlugin;

impl bevy::app::Plugin for EntityPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(
            bevy::prelude::FixedPreUpdate,
            (add_entity_to_chunk, add_player_synced_entities),
        );
        app.add_systems(FixedUpdate, (tick_chunk_entities, sync_entities));
        app.add_systems(
            FixedPostUpdate,
            (
                remove_entity_despawned,
                add_old_transform,
                update_chunk_entities,
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

#[derive(EntityEvent, Debug)]
struct EntityAddedToChunkEvent {
    pub entity: Entity,
    pub chunk: Entity,
    pub chunk_pos: ChunkPos,
    pub dimension: Entity,
}

#[derive(EntityEvent, Debug)]
struct EntityRemovedFromChunkEvent {
    pub entity: Entity,
    pub chunk: Entity,
    pub chunk_pos: ChunkPos,
    pub dimension: Entity,
}

#[derive(EntityEvent, Debug)]
struct EntityChunkMovedEvent {
    pub entity: Entity,
    pub old_chunk_pos: ChunkPos,
    pub new_chunk_pos: ChunkPos,
    pub dimension: Entity,
}

#[derive(EntityEvent, Debug)]
struct SendSpawnPacketEvent {
    entity: Entity,
    player: Entity,
}

#[derive(EntityEvent, Debug)]
struct SendDespawnPacketEvent {
    entity: Entity,
    player: Entity,
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
            let Some(chunk_index) = dim_chunks.get(in_dimension.entity()).ok() else {
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

fn add_old_transform(
    mut entities: Query<
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
    for (entity, transform) in entities.iter_mut() {
        commands.entity(entity).insert(OldTransform(*transform));
    }
}

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

        if dimension_changed || pos_changed {
            if let Ok(old_index) = dim_chunks.get(old_dim) {
                if let Some(old_chunk) = old_index.get(old_pos) {
                    if let Ok(mut chunk_entities) = chunks.get_mut(old_chunk) {
                        if chunk_entities.remove_entity(entity) {
                            commands.trigger(EntityRemovedFromChunkEvent {
                                entity,
                                chunk: old_chunk,
                                chunk_pos: old_pos,
                                dimension: old_dim,
                            });
                        }
                    }
                }
            }
        }

        let Ok(new_index) = dim_chunks.get(new_dim) else {
            continue;
        };
        if let Some(new_chunk) = new_index.get(new_pos) {
            if let Ok(mut chunk_entities) = chunks.get_mut(new_chunk) {
                if chunk_entities.add_entity(entity) {
                    commands.trigger(EntityAddedToChunkEvent {
                        entity,
                        chunk: new_chunk,
                        chunk_pos: new_pos,
                        dimension: new_dim,
                    });
                }
            }
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
            let Some(chunk_index) = dims_chunks.get(dimension).ok() else {
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

fn tick_chunk_entities(
    entities: Query<
        (
            Entity,
            &OldTransform,
            Ref<Transform>,
            &InDimension,
            &OldInDimension,
        ),
        (Without<Despawned>),
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

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkSyncEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkSyncedEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkAddEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkAddedEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkRemoveEvent {
    pub entity: Entity,
    pub player: Entity,
}

#[derive(EntityEvent, Debug)]
pub struct EntityNetworkRemovedEvent {
    pub entity: Entity,
    pub player: Entity,
}

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
) {
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
                    } else if need_sync || transform.is_changed() {
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
    event: On<EntityNetworkAddedEvent>,
    mut player_data: Query<&mut PlayerSynchronizedEntities, With<Player>>,
) {
    let Ok(mut synced_entities) = player_data.get_mut(event.player) else {
        return;
    };
    synced_entities.0.add(event.entity);
}

fn synced_entity_removed(
    event: On<EntityNetworkRemovedEvent>,
    mut player_data: Query<&mut PlayerSynchronizedEntities, With<Player>>,
) {
    let Ok(mut synced_entities) = player_data.get_mut(event.player) else {
        return;
    };
    synced_entities.0.remove(event.entity);
}
