use super::{
    DensityFunctionComponent, DependentDensityFunction, FindTopSurface, IndependentDensityFunction,
    Interpolated, NoiseRouter, Slice, branch_schedule::Step,
};
use crate::density_function::proto::Axis;
use bevy_math::IVec3;

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
    pub fn max_block(&self) -> IVec3 {
        self.min_block + self.size * self.step_block - IVec3::ONE
    }

    #[inline]
    pub fn len(&self) -> usize {
        (self.size.x * self.size.y * self.size.z) as usize
    }

    pub(super) fn positions_into(&self, out: &mut Vec<IVec3>) {
        out.clear();
        out.reserve(self.len());
        for z in 0..self.size.z {
            let bz = self.block_z(z);
            for x in 0..self.size.x {
                let bx = self.block_x(x);
                for y in 0..self.size.y {
                    out.push(IVec3::new(bx, self.block_y(y), bz));
                }
            }
        }
    }
}

/// Reusable buffers for [`NoiseRouter::sample_volume`]: one row per live node
/// over the volume, and the same over the volume's columns.
#[derive(Default)]
pub struct FillScratch {
    pub(super) rows: Vec<f32>,
    pub(super) column_rows: Vec<f32>,
    pub(super) column_positions: Vec<IVec3>,
    pub(super) positions: Vec<IVec3>,
    pub(super) needed: Vec<bool>,
}

impl FillScratch {
    pub fn new() -> Self {
        Self::default()
    }
}
