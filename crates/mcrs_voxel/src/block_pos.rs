use core::fmt::Display;
use core::hash::{Hash, Hasher};
use core::ops::{Add, AddAssign, Sub, SubAssign};
use glam::DVec3;
use glam::IVec3;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BlockPos(IVec3);

impl Display for BlockPos {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

    #[must_use]
    pub const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(
            self.0.x.wrapping_add(dx),
            self.0.y.wrapping_add(dy),
            self.0.z.wrapping_add(dz),
        )
    }

    #[must_use]
    pub const fn below(self) -> Self {
        self.offset(0, -1, 0)
    }

    /// The centre of the bottom face.
    #[must_use]
    pub fn bottom_centre(self) -> DVec3 {
        DVec3::new(
            f64::from(self.0.x) + 0.5,
            f64::from(self.0.y),
            f64::from(self.0.z) + 0.5,
        )
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
        let value = value.floor();
        BlockPos::new(value.x as i32, value.y as i32, value.z as i32)
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
