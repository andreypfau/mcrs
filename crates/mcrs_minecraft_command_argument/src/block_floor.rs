use glam::DVec3;

#[must_use]
pub fn block_floor(point: DVec3) -> [i32; 3] {
    [point.x, point.y, point.z].map(|axis| axis.floor() as i32)
}
