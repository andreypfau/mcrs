use glam::{Vec2, Vec3};

use crate::cube::{FaceUv, UvRotation};
use crate::face::Face;

/// A texture rectangle `[u1, v1, u2, v2]` in texels. `u1 > u2` or `v1 > v2` flips the image.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvRect {
    pub u1: f32,
    pub v1: f32,
    pub u2: f32,
    pub v2: f32,
}

impl UvRect {
    #[must_use]
    pub const fn new(u1: f32, v1: f32, u2: f32, v2: f32) -> Self {
        Self { u1, v1, u2, v2 }
    }

    /// The box UV rectangle of `face`, with the layout sizes `layout` (`w`, `h`, `d`).
    #[must_use]
    pub fn of_box(face: Face, origin: Vec2, layout: Vec3, mirror: bool) -> Self {
        let Vec2 { x: u, y: v } = origin;
        let Vec3 { x: w, y: h, z: d } = layout;
        let laid_out = match (mirror, face) {
            (true, Face::East) => Face::West,
            (true, Face::West) => Face::East,
            (_, face) => face,
        };
        let rect = match laid_out {
            Face::East => Self::new(u, v + d, u + d, v + d + h),
            Face::North => Self::new(u + d, v + d, u + d + w, v + d + h),
            Face::West => Self::new(u + d + w, v + d, u + 2.0 * d + w, v + d + h),
            Face::South => Self::new(u + 2.0 * d + w, v + d, u + 2.0 * d + 2.0 * w, v + d + h),
            Face::Up => Self::new(u + d + w, v + d, u + d, v),
            Face::Down => Self::new(u + d + 2.0 * w, v, u + d + w, v + d),
        };
        if mirror {
            Self::new(rect.u2, rect.v1, rect.u1, rect.v2)
        } else {
            rect
        }
    }

    /// The per-face rectangle of `face`, with `size` the size of the box before inflate.
    #[must_use]
    pub fn of_face(face: Face, uv: &FaceUv, size: Vec3) -> Self {
        let Vec2 { x, y } = uv.uv;
        let extent = uv.uv_size.unwrap_or(match face {
            Face::North | Face::South => Vec2::new(size.x, size.y),
            Face::East | Face::West => Vec2::new(size.z, size.y),
            Face::Up | Face::Down => Vec2::new(size.x, size.z),
        });
        match face {
            Face::Up | Face::Down => Self::new(x + extent.x, y + extent.y, x, y),
            _ => Self::new(x, y, x + extent.x, y + extent.y),
        }
    }

    /// The texels of the corners `c0`, `c1`, `c2` and `c3`.
    #[must_use]
    pub fn corners(self) -> [Vec2; 4] {
        [
            Vec2::new(self.u1, self.v1),
            Vec2::new(self.u2, self.v1),
            Vec2::new(self.u1, self.v2),
            Vec2::new(self.u2, self.v2),
        ]
    }
}

/// Turns the texels of the corners clockwise. One step moves the texel of `c2` to `c0`, of `c3`
/// to `c2`, of `c1` to `c3` and of `c0` to `c1`.
#[must_use]
pub fn rotate_corners(texels: [Vec2; 4], rotation: UvRotation) -> [Vec2; 4] {
    let mut turned = texels;
    for _ in 0..rotation.steps() {
        let [c0, c1, c2, c3] = turned;
        turned = [c2, c0, c3, c1];
    }
    turned
}
