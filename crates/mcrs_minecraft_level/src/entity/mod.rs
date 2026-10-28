use crate::entity::physics::{OldTransform, Transform};
use crate::entity::player::Player;
use crate::tick::TickSet;
use crate::world::dimension::{Dimension, DimensionPlayers, InDimension, OldInDimension};
use crate::world::storage::section::Section;
use crate::world::storage::section::SectionIndex;
use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Plugin};
use bevy_derive::Deref;
use bevy_ecs::entity::EntityHashSet;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::prelude::{
    Added, Commands, Component, ContainsEntity, DetectChanges, Entity, EntityEvent, Has,
    IntoScheduleConfigs, Local, On, ParallelCommands, Query, Ref, With, Without,
};
use bevy_ecs::relationship::RelationshipSourceCollection;
use bevy_platform::time::Instant;
use mcrs_minecraft_core::SectionPos;
use uuid::Uuid;

pub mod physics;
pub mod player;

pub struct EntityPlugin;

impl Plugin for EntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPreUpdate,
            (add_entity_to_chunk, add_player_synced_entities),
        );
        app.add_systems(FixedUpdate, sync_entities.in_set(TickSet::Project));
        app.add_systems(
            FixedPostUpdate,
            (
                add_old_transform,
                update_chunk_entities.before(crate::world::dimension::update_old_in_dimensions),
                update_old_transforms,
            )
                .chain(),
        );
        app.add_observer(synced_entity_added);
        app.add_observer(synced_entity_removed);
        app.add_observer(forget_removed_entity);
    }
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct EntityNetworkSync;

#[derive(Component, Default, Deref)]
struct PlayerSynchronizedEntities(EntityHashSet);

#[derive(Component, Debug, Default, Deref)]
pub struct SectionEntities(Vec<Entity>);

impl SectionEntities {
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

fn add_entity_to_chunk(
    dim_chunks: Query<&SectionIndex>,
    mut chunks: Query<&mut SectionEntities>,
    entities: Query<
        (Entity, &InDimension, &Transform),
        (Without<Dimension>, Without<Section>, Without<OldTransform>),
    >,
) {
    entities
        .iter()
        .for_each(|(entity, in_dimension, transform)| {
            let Ok(chunk_index) = dim_chunks.get(in_dimension.entity()) else {
                return;
            };
            let chunk_pos = SectionPos::from(transform.translation);
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
    entities: Query<
        (Entity, &Transform),
        (Without<Dimension>, Without<Section>, Without<OldTransform>),
    >,
    mut commands: Commands,
) {
    for (entity, transform) in entities.iter() {
        commands.entity(entity).insert(OldTransform(*transform));
    }
}

fn update_chunk_entities(
    dim_chunks: Query<&SectionIndex>,
    mut chunks: Query<&mut SectionEntities>,
    entities: Query<
        (
            Entity,
            &InDimension,
            Ref<Transform>,
            &OldTransform,
            &OldInDimension,
        ),
        (Without<Dimension>, Without<Section>),
    >,
) {
    for (entity, dimension, transform, old_transform, old_dimension) in entities.iter() {
        let old_dim = old_dimension.entity();
        let new_dim = dimension.entity();

        let dimension_changed = old_dim != new_dim;
        if !dimension_changed && !transform.is_changed() {
            continue;
        }

        let old_pos = SectionPos::from(old_transform.translation);
        let new_pos = SectionPos::from(transform.translation);
        let pos_changed = old_pos != new_pos;

        if (dimension_changed || pos_changed)
            && let Ok(old_index) = dim_chunks.get(old_dim)
            && let Some(old_chunk) = old_index.get(old_pos)
            && let Ok(mut chunk_entities) = chunks.get_mut(old_chunk)
        {
            chunk_entities.remove_entity(entity);
        }

        let Ok(new_index) = dim_chunks.get(new_dim) else {
            continue;
        };
        if let Some(new_chunk) = new_index.get(new_pos)
            && let Ok(mut chunk_entities) = chunks.get_mut(new_chunk)
        {
            chunk_entities.add_entity(entity);
        }
    }
}

fn forget_removed_entity(
    remove: On<Remove, InDimension>,
    entities: Query<(
        &InDimension,
        Option<&OldInDimension>,
        &Transform,
        Option<&OldTransform>,
    )>,
    dims_chunks: Query<&SectionIndex>,
    mut chunks: Query<&mut SectionEntities>,
    mut synced: Query<&mut PlayerSynchronizedEntities>,
) {
    let entity = remove.event().entity;
    for mut synced_entities in &mut synced {
        synced_entities.0.remove(entity);
    }
    let Ok((in_dimension, old_in_dimension, transform, old_transform)) = entities.get(entity)
    else {
        return;
    };
    let dimension = old_in_dimension.map_or(in_dimension.entity(), ContainsEntity::entity);
    let indexed_at = old_transform.map_or(transform, |old| &old.0);
    if let Ok(chunk_index) = dims_chunks.get(dimension)
        && let Some(chunk) = chunk_index.get(SectionPos::from(indexed_at.translation))
        && let Ok(mut chunk_entities) = chunks.get_mut(chunk)
    {
        chunk_entities.remove_entity(entity);
    }
}

fn update_old_transforms(mut entities: Query<(&mut OldTransform, &Transform)>) {
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

fn add_player_synced_entities(
    mut commands: Commands,
    new_players: Query<Entity, (With<Player>, Added<InDimension>)>,
) {
    new_players.iter().for_each(|entity| {
        commands
            .entity(entity)
            .insert(PlayerSynchronizedEntities::default());
    });
}

fn sync_entities(
    dim_players: Query<&DimensionPlayers>,
    player_data: Query<(&Transform, &PlayerSynchronizedEntities), With<Player>>,
    entities: Query<(Entity, &InDimension, Ref<Transform>, Has<EntityNetworkSync>)>,
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
        .for_each(|(entity, in_dimension, transform, need_sync)| {
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
                    if too_far() {
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

#[derive(Debug, Clone, Copy, Component, Deref, PartialEq, Eq)]
pub struct EntityUuid(pub Uuid);

impl Default for EntityUuid {
    fn default() -> Self {
        EntityUuid(Uuid::new_v4())
    }
}
