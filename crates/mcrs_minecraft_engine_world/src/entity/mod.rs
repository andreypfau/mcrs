use crate::entity::physics::{OldTransform, Transform};
use crate::world::dimension::{Dimension, InDimension, OldInDimension};
use crate::world::storage::section::Section;
use crate::world::storage::section::SectionIndex;
use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_derive::Deref;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::prelude::{
    Commands, Component, ContainsEntity, DetectChanges, Entity, IntoScheduleConfigs, On, Query,
    Ref, Without,
};
use mcrs_voxel::SectionPos;
use uuid::Uuid;

pub mod physics;
pub mod player;

pub struct EntityPlugin;

impl Plugin for EntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedPreUpdate, add_entity_to_chunk);
        app.add_systems(
            FixedPostUpdate,
            (
                add_old_transform,
                update_chunk_entities.before(crate::world::dimension::update_old_in_dimensions),
                update_old_transforms,
            )
                .chain(),
        );
        app.add_observer(forget_removed_entity);
    }
}

#[derive(Component, Debug, Default, Deref)]
pub struct SectionEntities(Vec<Entity>);

impl SectionEntities {
    pub fn remove_entity(&mut self, entity: Entity) -> bool {
        if let Some(pos) = self.0.iter().position(|&e| e == entity) {
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

/// Enters each entity that has no old transform yet in the entities of its section.
pub fn add_entity_to_chunk(
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
    remove: On<Remove<InDimension>>,
    entities: Query<(
        &InDimension,
        Option<&OldInDimension>,
        &Transform,
        Option<&OldTransform>,
    )>,
    dims_chunks: Query<&SectionIndex>,
    mut chunks: Query<&mut SectionEntities>,
) {
    let entity = remove.event().entity;
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

#[derive(Debug, Clone, Copy, Component, Deref, PartialEq, Eq)]
pub struct EntityUuid(pub Uuid);

impl Default for EntityUuid {
    fn default() -> Self {
        EntityUuid(Uuid::new_v4())
    }
}
