use glam::{BVec3, DVec3};

/// Each axis of `axes` takes the block floor of its value.
#[must_use]
pub fn align(axes: BVec3, point: DVec3) -> DVec3 {
    DVec3::select(axes, point.floor().as_ivec3().as_dvec3(), point)
}
