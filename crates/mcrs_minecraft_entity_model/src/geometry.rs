use glam::Vec3;

use crate::cube::Cube;
use crate::format_version::FormatVersion;

/// One geometry of a geometry file, with the values of the file in file space.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Geometry {
    pub identifier: String,
    /// The parent geometry of the legacy shape, until [`crate::resolve_parents`] merges it.
    pub parent: Option<String>,
    /// The format version of the file that defines the geometry.
    pub format: FormatVersion,
    pub texture_width: Option<f32>,
    pub texture_height: Option<f32>,
    pub visible_bounds_width: Option<f32>,
    pub visible_bounds_height: Option<f32>,
    pub visible_bounds_offset: Option<Vec3>,
    pub bones: Vec<Bone>,
}

/// A bone of a geometry in file space.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Bone {
    pub name: String,
    pub parent: Option<String>,
    pub pivot: Vec3,
    /// The rest rotation in degrees.
    pub rotation: Vec3,
    pub mirror: bool,
    pub inflate: f32,
    pub cubes: Vec<Cube>,
    pub locators: Vec<Locator>,
    /// The bone is hidden in all render controllers.
    pub never_render: bool,
    /// The Molang expression that names the bone of another model to attach to.
    pub binding: Option<String>,
}

/// A locator of a bone in file space.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Locator {
    pub name: String,
    pub offset: Vec3,
    /// The rotation in degrees.
    pub rotation: Vec3,
    pub ignore_inherited_scale: bool,
}

impl Geometry {
    #[must_use]
    pub fn new(identifier: String, format: FormatVersion) -> Self {
        Self {
            identifier,
            parent: None,
            format,
            texture_width: None,
            texture_height: None,
            visible_bounds_width: None,
            visible_bounds_height: None,
            visible_bounds_offset: None,
            bones: Vec::new(),
        }
    }
}

impl Bone {
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            parent: None,
            pivot: Vec3::ZERO,
            rotation: Vec3::ZERO,
            mirror: false,
            inflate: 0.0,
            cubes: Vec::new(),
            locators: Vec::new(),
            never_render: false,
            binding: None,
        }
    }
}

impl Locator {
    #[must_use]
    pub fn new(name: String, offset: Vec3) -> Self {
        Self {
            name,
            offset,
            rotation: Vec3::ZERO,
            ignore_inherited_scale: false,
        }
    }
}
