pub mod palette;
pub mod ticket;

use crate::entity::ChunkEntities;
use crate::math::BitSize;
use crate::world::block::BlockPos;
use crate::world::chunk::ticket::TicketPlugin;
use crate::world::dimension::InDimension;
use bevy_app::{App, Plugin};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::{Bundle, Component, Entity};
use bevy_math::DVec3;
use bevy_math::prelude::*;
use rustc_hash::FxHashMap;
use std::fmt::Display;
use std::hash::{Hash, Hasher};

pub(crate) struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TicketPlugin);
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Component, Deref, DerefMut)]
pub struct ChunkPos(pub IVec3);

impl Display for ChunkPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}:{}:{})", self.x, self.y, self.z)
    }
}

impl ChunkPos {
    const PACKED_X_LENGTH: usize = 22;
    const PACKED_Z_LENGTH: usize = 22;
    const PACKED_Y_LENGTH: usize = 20;
    const PACKED_X_MASK: u64 = (1 << Self::PACKED_X_LENGTH) - 1;
    const PACKED_Y_MASK: u64 = (1 << Self::PACKED_Y_LENGTH) - 1;
    const PACKED_Z_MASK: u64 = (1 << Self::PACKED_Z_LENGTH) - 1;

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }
}

impl Hash for ChunkPos {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let packed = (self.x as u64 & Self::PACKED_X_MASK) << 42
            | (self.z as u64 & Self::PACKED_Z_MASK) << 20
            | (self.y as u64 & Self::PACKED_Y_MASK);
        packed.hash(state);
    }
}

pub type BLOCKS = BitSize<4>;

impl From<DVec3> for ChunkPos {
    fn from(pos: DVec3) -> Self {
        Self::new(
            (pos.x.floor() as i32) >> BLOCKS::BITS,
            (pos.y.floor() as i32) >> BLOCKS::BITS,
            (pos.z.floor() as i32) >> BLOCKS::BITS,
        )
    }
}

impl From<BlockPos> for ChunkPos {
    fn from(pos: BlockPos) -> Self {
        Self::new(
            pos.x >> BLOCKS::BITS,
            pos.y >> BLOCKS::BITS,
            pos.z >> BLOCKS::BITS,
        )
    }
}

#[derive(Bundle)]
pub struct ChunkBundle {
    pub dimension: InDimension,
    pub pos: ChunkPos,
    pub status: ChunkStatus,
    pub entities: ChunkEntities,
    marker: Chunk,
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Chunk;

impl ChunkBundle {
    pub fn new(dimension: InDimension, chunk_pos: ChunkPos) -> Self {
        Self {
            dimension,
            pos: chunk_pos,
            status: ChunkStatus::default(),
            entities: ChunkEntities::default(),
            marker: Chunk,
        }
    }
}

// TODO: Change from enum to separate marker structures for each status for optimized archetype filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Component)]
pub enum ChunkStatus {
    Unloaded,
    #[default]
    Loading,
    Generating,
    Loaded,
    Unloading,
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
