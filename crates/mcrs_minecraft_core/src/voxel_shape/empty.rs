use super::{Aabb, FACE_MASK_EMPTY, ShapeRepr, VoxelShape};
use bevy_math::Vec3;

pub(super) static EMPTY: VoxelShape = VoxelShape {
    repr: ShapeRepr::Empty,
    bounds: Aabb {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    },
    occludes_full_block: false,
    face_masks: [FACE_MASK_EMPTY; 6],
};

#[inline]
pub fn empty_shape() -> &'static VoxelShape {
    &EMPTY
}
