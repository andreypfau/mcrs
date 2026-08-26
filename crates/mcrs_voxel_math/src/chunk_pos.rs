use crate::BitSize;
use crate::BlockPos;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::Component;
use bevy_math::DVec3;
use bevy_math::prelude::*;
use std::fmt::Display;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Component, Deref, DerefMut)]
pub struct ChunkPos(pub IVec3);

impl Display for ChunkPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}:{}:{})", self.x, self.y, self.z)
    }
}

impl ChunkPos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }
}

impl Hash for ChunkPos {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
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
