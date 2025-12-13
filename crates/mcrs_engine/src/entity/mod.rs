use crate::entity::physics::{OldTransform, Transform};
use crate::world::chunk::{Chunk, ChunkIndex, ChunkPos};
use crate::world::dimension::{Dimension, InDimension, OldInDimension};
use bevy::app::FixedPostUpdate;
use bevy::prelude::{
    Component, ContainsEntity, Deref, DetectChanges, Entity, IntoScheduleConfigs, Query, Ref,
    Without,
};

pub mod despawn;
pub mod physics;
pub mod player;

pub struct EntityPlugin;

impl bevy::app::Plugin for EntityPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(bevy::prelude::FixedPreUpdate, (add_entity_to_chunk,));
        app.add_systems(
            FixedPostUpdate,
            (add_old_transform, update_chunk_entities).chain(),
        );
    }
}

#[derive(Component, Debug, Default, Deref)]
pub struct ChunkEntities(Vec<Entity>);

#[derive(Component, Debug, Default, Deref)]
pub struct EntityObservers(Vec<Entity>);

impl EntityObservers {
    pub fn new(entities: Vec<Entity>) -> Self {
        Self(entities)
    }
}

fn add_entity_to_chunk(
    dim_chunks: Query<&ChunkIndex>,
    mut chunks: Query<&mut ChunkEntities>,
    entities: Query<
        (Entity, &InDimension, &Transform),
        (Without<Dimension>, Without<Chunk>, Without<OldTransform>),
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
            chunk_entities.0.push(entity);
        });
}

fn add_old_transform(
    mut entities: Query<
        (Entity, &Transform),
        (Without<Dimension>, Without<Chunk>, Without<OldTransform>),
    >,
    mut commands: bevy::prelude::Commands,
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
        (Without<Dimension>, Without<Chunk>),
    >,
) {
    for (entity, dimension, transform, old_transform, old_dimension) in entities.iter() {
        if !transform.is_changed() {
            continue;
        }
        let old_chunk_pos = ChunkPos::from(old_transform.translation);
        let new_chunk_pos = ChunkPos::from(transform.translation);

        if old_chunk_pos == new_chunk_pos && dimension.entity() == old_dimension.entity() {
            continue;
        }
        let Some(old_chunk_index) = dim_chunks.get(old_dimension.entity()).ok() else {
            continue;
        };
        if let Some(old_chunk) = old_chunk_index.get(old_chunk_pos) {
            chunks.get_mut(old_chunk).ok().map(|mut chunk_entities| {
                chunk_entities
                    .0
                    .iter()
                    .position(|&e| e == entity)
                    .map(|p| chunk_entities.0.swap_remove(p));
            });
        }
        let Some(chunk_index) = dim_chunks.get(dimension.entity()).ok() else {
            continue;
        };
        if let Some(new_chunk) = chunk_index.get(new_chunk_pos) {
            if let Ok(mut chunk_entities) = chunks.get_mut(new_chunk) {
                chunk_entities.0.push(entity);
            }
        }
    }
}
