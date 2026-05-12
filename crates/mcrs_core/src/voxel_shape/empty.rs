//! Empty `VoxelShape` singleton — occupies no volume, occludes nothing.

use super::{Aabb, ShapeRepr, VoxelShape};
use bevy_math::Vec3;

pub(super) static EMPTY: VoxelShape = VoxelShape {
    repr: ShapeRepr::Empty,
    bounds: Aabb {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    },
    occludes_full_block: false,
    face_cache: [
        &EMPTY, &EMPTY, &EMPTY, &EMPTY, &EMPTY, &EMPTY,
    ],
};

#[inline]
pub fn empty_shape() -> &'static VoxelShape {
    &EMPTY
}
