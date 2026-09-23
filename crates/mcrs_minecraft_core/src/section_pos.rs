use crate::BlockPos;
use bevy_math::DVec3;
use bevy_math::prelude::*;
use std::fmt::Display;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct SectionPos(pub IVec3);

impl std::ops::Deref for SectionPos {
    type Target = IVec3;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SectionPos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Display for SectionPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}:{}:{})", self.x, self.y, self.z)
    }
}

impl SectionPos {
    pub const BITS: usize = 4;
    pub const SIZE: usize = 1 << Self::BITS;
    pub const MASK: usize = Self::SIZE - 1;
    pub const AREA: usize = Self::SIZE * Self::SIZE;
    pub const VOLUME: usize = Self::AREA * Self::SIZE;

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }
}

impl Hash for SectionPos {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
    }
}

impl From<DVec3> for SectionPos {
    fn from(pos: DVec3) -> Self {
        Self::new(
            (pos.x.floor() as i32) >> SectionPos::BITS,
            (pos.y.floor() as i32) >> SectionPos::BITS,
            (pos.z.floor() as i32) >> SectionPos::BITS,
        )
    }
}

impl From<BlockPos> for SectionPos {
    fn from(pos: BlockPos) -> Self {
        Self::new(
            pos.x >> SectionPos::BITS,
            pos.y >> SectionPos::BITS,
            pos.z >> SectionPos::BITS,
        )
    }
}
