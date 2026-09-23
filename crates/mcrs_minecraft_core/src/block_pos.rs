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

    /// `BlockPos.asLong`: 26 bits of x, 26 of z and 12 of y, packed x-z-y.
    pub const fn as_long(self) -> i64 {
        const HORIZONTAL_MASK: i64 = (1 << 26) - 1;
        const Y_MASK: i64 = (1 << 12) - 1;
        ((self.0.x as i64 & HORIZONTAL_MASK) << 38)
            | ((self.0.z as i64 & HORIZONTAL_MASK) << 12)
            | (self.0.y as i64 & Y_MASK)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_long_packs_like_the_reference() {
        assert_eq!(BlockPos::new(0, 0, 0).as_long(), 0);
        assert_eq!(BlockPos::new(1, 2, 3).as_long(), (1 << 38) | (3 << 12) | 2);
        assert_eq!(BlockPos::new(-1, -1, -1).as_long(), -1);
        assert_eq!(BlockPos::new(-16, 62, 25).as_long(), -4398046408642);
    }
}
