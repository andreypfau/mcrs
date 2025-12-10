use bevy_ecs::prelude::*;
use mcrs_protocol::{ChunkPos, Position};
use crate::world::chunk::ChunkIndex;
use crate::world::entity::OldPosition;

pub(super) fn init_old_position(
    mut commands: Commands,
    query: Query<(Entity, &Position), (Added<Position>, Without<OldPosition>)>,
) {
    for (e, pos) in &query {
        commands.entity(e).insert(OldPosition(**pos));
    }
}

pub(super) fn update_old_position(
    mut query: Query<(&Position, &mut OldPosition), (Changed<Position>)>,
) {
    for (pos, mut old_pos) in &mut query {
        old_pos.0 = **pos;
    }
}

pub(super) fn entity_changed_chunk(
    mut chunk_index: ResMut<ChunkIndex>,
    query: Query<(Entity, &Position, &OldPosition), Changed<Position>>,
) {
    for (entity, pos, old_pos) in &query {
        let old_chunk_pos = ChunkPos::from(**old_pos);
        let new_chunk_pos = ChunkPos::from(**pos);
        if old_chunk_pos == new_chunk_pos {
            continue;
        }
        chunk_index.get_mut(old_chunk_pos).map(|chunk_entity| {
            chunk_entity.entities.remove(&entity);
        });
        chunk_index.get_mut(new_chunk_pos).map(|chunk_entity| {
            chunk_entity.entities.insert(entity);
        });
    }
}