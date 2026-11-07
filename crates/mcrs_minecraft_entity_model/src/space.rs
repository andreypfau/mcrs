use glam::{Affine3A, Mat3, Vec3};

/// A point of file space in internal space: a pivot, a locator position, a bounds offset or an
/// animation position.
#[must_use]
pub fn internal_point(file: Vec3) -> Vec3 {
    Vec3::new(-file.x, file.y, file.z)
}

/// A rotation of file space in internal space: a bone, cube, locator or animation rotation.
#[must_use]
pub fn internal_rotation(file: Vec3) -> Vec3 {
    Vec3::new(-file.x, -file.y, file.z)
}

/// The internal low and high corners of a cube box with a file `origin` and `size`.
#[must_use]
pub fn internal_box(origin: Vec3, size: Vec3) -> [Vec3; 2] {
    [
        Vec3::new(-(origin.x + size.x), origin.y, origin.z),
        Vec3::new(-origin.x, origin.y + size.y, origin.z + size.z),
    ]
}

/// The Euler rotation `Rz · Ry · Rx` of angles in degrees. A vertex turns about X first.
#[must_use]
pub fn euler(degrees: Vec3) -> Mat3 {
    Mat3::from_rotation_z(degrees.z.to_radians())
        * Mat3::from_rotation_y(degrees.y.to_radians())
        * Mat3::from_rotation_x(degrees.x.to_radians())
}

/// `T(pivot) · M · T(−pivot)`.
#[must_use]
pub fn about(pivot: Vec3, transform: Affine3A) -> Affine3A {
    Affine3A::from_translation(pivot) * transform * Affine3A::from_translation(-pivot)
}
