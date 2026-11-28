use glam::{Vec2, Vec3};

use crate::face::Face;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Cube {
    pub origin: Vec3,
    pub size: Vec3,
    pub rotation: Vec3,
    pub pivot: Option<Vec3>,
    pub inflate: Option<f32>,
    pub mirror: Option<bool>,
    pub uv: CubeUv,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CubeUv {
    Box(Vec2),
    PerFace(PerFaceUv),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerFaceUv {
    faces: [Option<FaceUv>; 6],
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct FaceUv {
    pub uv: Vec2,
    pub uv_size: Option<Vec2>,
    pub rotation: UvRotation,
}

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
