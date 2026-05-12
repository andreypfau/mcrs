//! Full unit-cube `VoxelShape` singleton — the canonical solid block.

use super::{Aabb, ShapeRepr, VoxelShape};
use bevy_math::Vec3;

pub(super) static BLOCK: VoxelShape = VoxelShape {
    repr: ShapeRepr::Block,
    bounds: Aabb {
        min: Vec3::ZERO,
        max: Vec3::ONE,
    },
    occludes_full_block: true,
    face_cache: [
        &BLOCK, &BLOCK, &BLOCK, &BLOCK, &BLOCK, &BLOCK,
    ],
};

#[inline]
pub fn block_shape() -> &'static VoxelShape {
    &BLOCK
}
