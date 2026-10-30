use glam::DVec3;

/// The block of a point: the floor of each axis, limited to the range of `i32`.
#[must_use]
pub fn block_floor(point: DVec3) -> [i32; 3] {
    [point.x, point.y, point.z].map(|axis| axis.floor() as i32)
}
