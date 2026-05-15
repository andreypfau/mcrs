pub use crate::geometry::ChunkPos;
pub use crate::geometry::chunk_pos::BLOCKS;

use crate::entity::ChunkEntities;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::markers::ChunkLoading;
use crate::world::lifecycle::ticket::TicketPlugin;
use bevy_app::{App, Plugin};
use bevy_derive::Deref;
use bevy_ecs::prelude::{Bundle, Component, Entity};
use rustc_hash::FxHashMap;

pub(crate) struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TicketPlugin);
    }
}

#[derive(Bundle)]
pub struct ChunkBundle {
    pub dimension: InDimension,
    pub pos: ChunkPos,
    pub entities: ChunkEntities,
    marker: Chunk,
    chunk_loading: ChunkLoading,
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Chunk;

impl ChunkBundle {
    pub fn new(dimension: InDimension, chunk_pos: ChunkPos) -> Self {
        Self {
            dimension,
            pos: chunk_pos,
            entities: ChunkEntities::default(),
            marker: Chunk,
            chunk_loading: ChunkLoading,
        }
    }
}

#[derive(Component, Debug, Default, Deref)]
pub struct ChunkIndex(FxHashMap<ChunkPos, Entity>);

impl ChunkIndex {
    pub fn new() -> Self {
        Self(FxHashMap::default())
    }

    pub fn get(&self, pos: impl Into<ChunkPos>) -> Option<Entity> {
        self.0.get(&pos.into()).copied()
    }

    pub fn insert(&mut self, pos: ChunkPos, entity: Entity) {
        self.0.insert(pos, entity);
    }

    pub fn remove(&mut self, pos: impl Into<ChunkPos>) -> Option<Entity> {
        self.0.remove(&pos.into())
    }

    pub fn contains(&self, pos: impl Into<ChunkPos>) -> bool {
        self.0.contains_key(&pos.into())
    }
}
