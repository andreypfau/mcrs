use glam::{Vec2, Vec3};

use crate::face::Face;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedGeometry {
    pub identifier: String,
    pub texture_size: Vec2,
    pub bones: Vec<BakedBone>,
    pub cubes: Vec<BakedCube>,
    pub locators: Vec<BakedLocator>,
    pub visible_bounds: VisibleBounds,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedBone {
    pub name: String,
    pub parent: Option<usize>,
    pub pivot: Vec3,
    pub rotation: Vec3,
    pub file_rotation: Vec3,
    pub never_render: bool,
    pub binding: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedCube {
    pub bone: usize,
    pub shape: CuboidShape,
    pub faces: Vec<BakedFace>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct CuboidShape {
    pub origin: Vec3,
    pub size: Vec3,
    pub inflate: f32,
    pub rotation: Vec3,
    pub pivot: Vec3,
    pub box_uv: Option<BoxUv>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct BoxUv {
    pub origin: Vec2,
    pub mirror: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedFace {
    pub face: Face,
    pub corners: [Vec3; 4],
    pub normal: Vec3,
    pub uvs: [Vec2; 4],
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedLocator {
    pub name: String,
    pub bone: usize,
    pub position: Vec3,
    pub rotation: Vec3,
    pub ignore_inherited_scale: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl BakedGeometry {
    #[must_use]
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|bone| bone.name.eq_ignore_ascii_case(name))
    }

    pub fn faces(&self) -> impl Iterator<Item = &BakedFace> {
        self.cubes.iter().flat_map(|cube| &cube.faces)
    }
}
