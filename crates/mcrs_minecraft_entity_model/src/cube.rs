use glam::{Vec2, Vec3};

use crate::face::Face;

/// A cube of a bone in file space.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Cube {
    pub origin: Vec3,
    /// The extent from `origin`, never negative.
    pub size: Vec3,
    /// The rotation in degrees about `pivot`.
    pub rotation: Vec3,
    /// The centre of the rotation; the centre of the box when absent.
    pub pivot: Option<Vec3>,
    /// The growth on each side; the inflate of the bone when absent.
    pub inflate: Option<f32>,
    /// The mirror of the box UV; the mirror of the bone when absent.
    pub mirror: Option<bool>,
    pub uv: CubeUv,
}

/// The texture coordinates of a cube.
#[derive(Clone, Debug, PartialEq)]
pub enum CubeUv {
    /// The box UV layout from this texel.
    Box(Vec2),
    /// A rectangle for each face. A face without one is not drawn.
    PerFace(PerFaceUv),
}

/// The per-face UV of a cube, indexed by [`Face`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerFaceUv {
    faces: [Option<FaceUv>; 6],
}

/// The rectangle of one face.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct FaceUv {
    pub uv: Vec2,
    /// The size of the rectangle; the size of the face before inflate when absent.
    pub uv_size: Option<Vec2>,
    pub rotation: UvRotation,
}

/// A clockwise turn of a face rectangle in steps of 90 degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UvRotation {
    #[default]
    None,
    Quarter,
    Half,
    ThreeQuarters,
}

impl Cube {
    #[must_use]
    pub fn new(origin: Vec3, size: Vec3, uv: CubeUv) -> Self {
        Self {
            origin,
            size,
            rotation: Vec3::ZERO,
            pivot: None,
            inflate: None,
            mirror: None,
            uv,
        }
    }
}

impl PerFaceUv {
    #[must_use]
    pub fn get(&self, face: Face) -> Option<&FaceUv> {
        self.faces.get(face as usize).and_then(Option::as_ref)
    }

    pub fn set(&mut self, face: Face, uv: FaceUv) {
        if let Some(slot) = self.faces.get_mut(face as usize) {
            *slot = Some(uv);
        }
    }
}

impl FaceUv {
    #[must_use]
    pub fn new(uv: Vec2) -> Self {
        Self {
            uv,
            uv_size: None,
            rotation: UvRotation::None,
        }
    }
}

impl UvRotation {
    /// The rotation of a value in degrees, or `None` when it is not 0, 90, 180 or 270.
    #[must_use]
    pub fn from_degrees(degrees: f64) -> Option<Self> {
        [
            UvRotation::None,
            UvRotation::Quarter,
            UvRotation::Half,
            UvRotation::ThreeQuarters,
        ]
        .into_iter()
        .find(|rotation| f64::from(rotation.steps()) * 90.0 == degrees)
    }

    #[must_use]
    pub fn steps(self) -> u8 {
        self as u8
    }
}
