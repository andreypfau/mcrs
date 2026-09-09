use crate::jmath::{floor_div, floor_mod};
use bevy_math::IVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Y,
    Z,
}

/// A strided box of block positions: `size` samples per axis, starting at
/// `min_block`, spaced `step_block` apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume {
    size: IVec3,
    min_block: IVec3,
    step_block: IVec3,
}

#[allow(clippy::len_without_is_empty)]
impl Volume {
    pub fn new(size: IVec3, min_block: IVec3, step_block: IVec3) -> Self {
        assert!(
            size.x > 0 && size.y > 0 && size.z > 0,
            "size must be positive, was: {}x{}x{}",
            size.x,
            size.y,
            size.z
        );
        assert!(
            step_block.x > 0 && step_block.y > 0 && step_block.z > 0,
            "step must be positive, was: {}; {}; {}",
            step_block.x,
            step_block.y,
            step_block.z
        );
        Self {
            size,
            min_block,
            step_block,
        }
    }

    pub fn dense(size: IVec3, min_block: IVec3) -> Self {
        Self::new(size, min_block, IVec3::ONE)
    }

    pub fn point(pos: IVec3) -> Self {
        Self::new(IVec3::ONE, pos, IVec3::ONE)
    }

    #[inline]
    pub fn size(&self) -> IVec3 {
        self.size
    }

    #[inline]
    pub fn min_block(&self) -> IVec3 {
        self.min_block
    }

    #[inline]
    pub fn step_block(&self) -> IVec3 {
        self.step_block
    }

    /// The last block the volume spans, which is not the last block it samples
    /// unless every step is 1.
    #[inline]
    pub fn max_block(&self) -> IVec3 {
        self.min_block + self.size * self.step_block - IVec3::ONE
    }

    /// Y is the fastest axis. Changing this breaks every `Slice` broadcast into
    /// a strided write and invalidates the binary parity fixtures.
    #[inline]
    pub fn index_unchecked(&self, x: i32, y: i32, z: i32) -> usize {
        (y + (x + z * self.size.x) * self.size.y) as usize
    }

    #[inline]
    pub fn block_x(&self, x: i32) -> i32 {
        self.min_block.x + x * self.step_block.x
    }

    #[inline]
    pub fn block_y(&self, y: i32) -> i32 {
        self.min_block.y + y * self.step_block.y
    }

    #[inline]
    pub fn block_z(&self, z: i32) -> i32 {
        self.min_block.z + z * self.step_block.z
    }

    #[inline]
    pub fn len(&self) -> usize {
        (self.size.x * self.size.y * self.size.z) as usize
    }

    /// `None` when the block lies outside the volume, or inside its span but off
    /// the step lattice.
    pub fn index_of_block(&self, bx: i32, by: i32, bz: i32) -> Option<usize> {
        let r = IVec3::new(bx, by, bz) - self.min_block;
        if self.step_block == IVec3::ONE {
            let inside = r.x >= 0
                && r.y >= 0
                && r.z >= 0
                && r.x < self.size.x
                && r.y < self.size.y
                && r.z < self.size.z;
            return inside.then(|| self.index_unchecked(r.x, r.y, r.z));
        }
        let span = self.size * self.step_block;
        let inside = r.x >= 0
            && r.y >= 0
            && r.z >= 0
            && r.x < span.x
            && r.y < span.y
            && r.z < span.z
            && floor_mod(r.x, self.step_block.x) == 0
            && floor_mod(r.y, self.step_block.y) == 0
            && floor_mod(r.z, self.step_block.z) == 0;
        inside.then(|| {
            self.index_unchecked(
                floor_div(r.x, self.step_block.x),
                floor_div(r.y, self.step_block.y),
                floor_div(r.z, self.step_block.z),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_is_y_fastest() {
        let v = Volume::dense(IVec3::new(2, 3, 4), IVec3::ZERO);
        assert_eq!(v.index_unchecked(0, 0, 0), 0);
        assert_eq!(v.index_unchecked(0, 1, 0), 1, "Y is contiguous");
        assert_eq!(v.index_unchecked(1, 0, 0), 3);
        assert_eq!(v.index_unchecked(0, 0, 1), 6);
        assert_eq!(v.len(), 24);
    }

    #[test]
    fn index_of_block_dense() {
        let v = Volume::dense(IVec3::new(2, 3, 4), IVec3::new(10, -64, 5));
        assert_eq!(v.index_of_block(10, -64, 5), Some(0));
        assert_eq!(
            v.index_of_block(11, -62, 8),
            Some(v.index_unchecked(1, 2, 3))
        );
        assert_eq!(v.index_of_block(9, -64, 5), None);
        assert_eq!(v.index_of_block(12, -64, 5), None);
    }

    #[test]
    fn index_of_block_rejects_off_lattice() {
        let v = Volume::new(IVec3::new(2, 2, 2), IVec3::ZERO, IVec3::new(4, 8, 4));
        assert_eq!(v.index_of_block(0, 0, 0), Some(0));
        assert_eq!(v.index_of_block(4, 8, 4), Some(v.index_unchecked(1, 1, 1)));
        assert_eq!(v.index_of_block(1, 0, 0), None, "off the step lattice");
        assert_eq!(v.index_of_block(8, 0, 0), None, "past the span");
    }

    #[test]
    fn index_of_block_handles_negative_relative_coords() {
        // floor_mod, not `%`: a truncating remainder would accept -4 here.
        let v = Volume::new(
            IVec3::new(2, 1, 1),
            IVec3::new(0, 0, 0),
            IVec3::new(4, 1, 1),
        );
        assert_eq!(v.index_of_block(-4, 0, 0), None);
    }
}
