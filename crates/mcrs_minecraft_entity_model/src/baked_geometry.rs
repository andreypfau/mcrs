use glam::{Vec2, Vec3};

use crate::face::Face;

/// A geometry baked for drawing: the bone table, the faces of each cube in the rest pose of
/// the cube, the locators and the visible bounds. All values are in internal space.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedGeometry {
    pub identifier: String,
    /// The texture size in texels that normalises the texture coordinates.
    pub texture_size: Vec2,
    /// A parent bone comes before each of its child bones.
    pub bones: Vec<BakedBone>,
    /// The cubes in the order of their bones in `bones`, then in file order.
    pub cubes: Vec<BakedCube>,
    pub locators: Vec<BakedLocator>,
    pub visible_bounds: VisibleBounds,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedBone {
    pub name: String,
    /// The index of the parent bone in [`BakedGeometry::bones`].
    pub parent: Option<usize>,
    pub pivot: Vec3,
    /// The rest rotation in degrees, in internal space.
    pub rotation: Vec3,
    /// The rest rotation in degrees as the file states it.
    pub file_rotation: Vec3,
    /// The bone is hidden in all render controllers.
    pub never_render: bool,
    pub binding: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedCube {
    /// The index of the bone in [`BakedGeometry::bones`].
    pub bone: usize,
    pub shape: CuboidShape,
    /// The faces that are drawn, in the order of [`Face::ALL`].
    pub faces: Vec<BakedFace>,
}

/// The box of a cube before the bake turns it into faces.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct CuboidShape {
    /// The low corner before inflate.
    pub origin: Vec3,
    /// The size before inflate.
    pub size: Vec3,
    pub inflate: f32,
    /// The cube rotation in degrees about `pivot`.
    pub rotation: Vec3,
    pub pivot: Vec3,
    /// The box UV, or `None` for a per-face UV.
    pub box_uv: Option<BoxUv>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct BoxUv {
    /// The texel of the layout origin.
    pub origin: Vec2,
    pub mirror: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedFace {
    pub face: Face,
    /// The corners `c0..c3`; [`crate::FACE_TRIANGLES`] makes two triangles of them.
    pub corners: [Vec3; 4],
    pub normal: Vec3,
    /// The texture coordinates of the corners, normalised by the texture size.
    pub uvs: [Vec2; 4],
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BakedLocator {
    pub name: String,
    pub bone: usize,
    pub position: Vec3,
    /// The rotation in degrees.
    pub rotation: Vec3,
    pub ignore_inherited_scale: bool,
}

/// An axis-aligned box in blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl BakedGeometry {
    /// The index of the bone with `name`, compared without regard to letter case.
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
