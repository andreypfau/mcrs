use super::{Aabb, FACE_MASK_FULL, ShapeRepr, VoxelShape};
use bevy_math::Vec3;

pub(super) static BLOCK: VoxelShape = VoxelShape {
    repr: ShapeRepr::Block,
    bounds: Aabb {
        min: Vec3::ZERO,
        max: Vec3::ONE,
    },
    occludes_full_block: true,
    face_masks: [FACE_MASK_FULL; 6],
};

#[inline]
pub fn block_shape() -> &'static VoxelShape {
    &BLOCK
}
