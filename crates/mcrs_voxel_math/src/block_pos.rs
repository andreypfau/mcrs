use bevy_math::DVec3;
use bevy_math::prelude::*;
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BlockPos(IVec3);

impl Display for BlockPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl ::core::ops::Deref for BlockPos {
    type Target = IVec3;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ::core::ops::DerefMut for BlockPos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    pub const fn as_ivec3(self) -> IVec3 {
        self.0
    }
}

impl Add<IVec3> for BlockPos {
    type Output = Self;

    fn add(self, offset: IVec3) -> Self {
        Self(self.0 + offset)
    }
}

impl Sub<IVec3> for BlockPos {
    type Output = Self;

    fn sub(self, offset: IVec3) -> Self {
        Self(self.0 - offset)
    }
}

impl AddAssign<IVec3> for BlockPos {
    fn add_assign(&mut self, offset: IVec3) {
        self.0 += offset;
    }
}

impl SubAssign<IVec3> for BlockPos {
    fn sub_assign(&mut self, offset: IVec3) {
        self.0 -= offset;
    }
}

impl Sub for BlockPos {
    type Output = IVec3;

    fn sub(self, other: Self) -> IVec3 {
        self.0 - other.0
    }
}

impl Hash for BlockPos {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
    }
}

impl From<IVec3> for BlockPos {
    fn from(value: IVec3) -> Self {
        Self(value)
    }
}

impl From<DVec3> for BlockPos {
    fn from(value: DVec3) -> Self {
        BlockPos::new(
            value.x.floor() as i32,
            value.y.floor() as i32,
            value.z.floor() as i32,
        )
    }
}

impl From<(i32, i32, i32)> for BlockPos {
    fn from((x, y, z): (i32, i32, i32)) -> Self {
        BlockPos::new(x, y, z)
    }
}

impl From<BlockPos> for (i32, i32, i32) {
    fn from(pos: BlockPos) -> Self {
        (pos.x, pos.y, pos.z)
    }
}

impl From<[i32; 3]> for BlockPos {
    fn from([x, y, z]: [i32; 3]) -> Self {
        BlockPos::new(x, y, z)
    }
}

impl From<BlockPos> for [i32; 3] {
    fn from(pos: BlockPos) -> Self {
        [pos.x, pos.y, pos.z]
    }
}
