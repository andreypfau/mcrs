use glam::{Affine3A, Mat3, Vec3};

#[must_use]
pub fn internal_point(file: Vec3) -> Vec3 {
    Vec3::new(-file.x, file.y, file.z)
}

#[must_use]
pub fn internal_rotation(file: Vec3) -> Vec3 {
    Vec3::new(-file.x, -file.y, file.z)
}

#[must_use]
pub fn internal_box(origin: Vec3, size: Vec3) -> [Vec3; 2] {
    [
        Vec3::new(-(origin.x + size.x), origin.y, origin.z),
        Vec3::new(-origin.x, origin.y + size.y, origin.z + size.z),
    ]
}

#[must_use]
pub fn euler(degrees: Vec3) -> Mat3 {
    Mat3::from_rotation_z(degrees.z.to_radians())
        * Mat3::from_rotation_y(degrees.y.to_radians())
        * Mat3::from_rotation_x(degrees.x.to_radians())
}

#[must_use]
pub fn about(pivot: Vec3, transform: Affine3A) -> Affine3A {
    Affine3A::from_translation(pivot) * transform * Affine3A::from_translation(-pivot)
}
