use glam::{Affine3A, Vec2, Vec3};

use crate::baked_geometry::{
    BakedBone, BakedCube, BakedFace, BakedGeometry, BakedLocator, BoxUv, CuboidShape, VisibleBounds,
};
use crate::cube::{Cube, CubeUv};
use crate::diagnostic::Diagnostic;
use crate::face::Face;
use crate::geometry::{Bone, Geometry, Locator};
use crate::geometry_error::GeometryError;
use crate::space;
use crate::uv_rect::{self, UvRect};

/// The most bones of one geometry.
pub const MAX_BONES: usize = 128;
/// The most bones of a chain from a root bone to a bone.
pub const MAX_DEPTH: usize = 16;
/// The most cubes of one geometry.
pub const MAX_CUBES: usize = 2048;

const DEFAULT_TEXTURE_SIZE: f32 = 16.0;
const MODEL_UNITS_PER_BLOCK: f32 = 16.0;
const COMPUTED_BOUNDS_GROWTH: f32 = 1.5;

impl Geometry {
    /// Bakes the geometry. A content error that does not reject the geometry goes to
    /// `diagnostics`. The legacy parent must be resolved first ([`crate::resolve_parents`]).
    pub fn bake(&self, diagnostics: &mut Vec<Diagnostic>) -> Result<BakedGeometry, GeometryError> {
        check_limits(self)?;
        let parents = file_parents(self, diagnostics)?;
        check_chains(self, &parents)?;
        let order = parent_first_order(&parents);
        let mut baked_index = vec![0; order.len()];
        for (index, &file_index) in order.iter().enumerate() {
            if let Some(slot) = baked_index.get_mut(file_index) {
                *slot = index;
            }
        }
        let texture_size = self.texture_size(diagnostics);
        let mut bones = Vec::with_capacity(order.len());
        let mut cubes = Vec::new();
        let mut locators = Vec::new();
        for (index, &file_index) in order.iter().enumerate() {
            let Some(bone) = self.bones.get(file_index) else {
                continue;
            };
            let parent = parents
                .get(file_index)
                .copied()
                .flatten()
                .and_then(|parent| baked_index.get(parent).copied());
            bones.push(baked_bone(bone, parent));
            for cube in &bone.cubes {
                cubes.push(bake_cube(bone, cube, index, texture_size));
            }
            for locator in &bone.locators {
                push_locator(&mut locators, locator, index, &self.identifier, diagnostics);
            }
        }
        let mut baked = BakedGeometry {
            identifier: self.identifier.clone(),
            texture_size,
            bones,
            cubes,
            locators,
            visible_bounds: VisibleBounds {
                min: Vec3::ZERO,
                max: Vec3::ZERO,
            },
        };
        baked.visible_bounds = self.visible_bounds(&baked);
        Ok(baked)
    }

    fn texture_size(&self, diagnostics: &mut Vec<Diagnostic>) -> Vec2 {
        if let (Some(width), Some(height)) = (self.texture_width, self.texture_height) {
            return Vec2::new(width, height);
        }
        diagnostics.push(Diagnostic::DefaultTextureSize {
            geometry: self.identifier.clone(),
        });
        Vec2::splat(DEFAULT_TEXTURE_SIZE)
    }

    fn visible_bounds(&self, baked: &BakedGeometry) -> VisibleBounds {
        let offset = space::internal_point(self.visible_bounds_offset.unwrap_or(Vec3::ZERO));
        if let (Some(width), Some(height)) = (self.visible_bounds_width, self.visible_bounds_height)
        {
            let half = Vec3::new(width, height, width) / 2.0;
            return VisibleBounds {
                min: offset - half,
                max: offset + half,
            };
        }
        let transforms = baked.model_transforms(&[]);
        let mut min = Vec3::INFINITY;
        let mut max = Vec3::NEG_INFINITY;
        for cube in &baked.cubes {
            let transform = transforms
                .get(cube.bone)
                .copied()
                .unwrap_or(Affine3A::IDENTITY);
            for corner in cube.faces.iter().flat_map(|face| face.corners) {
                let placed = transform.transform_point3(corner);
                min = min.min(placed);
                max = max.max(placed);
            }
        }
        if min.x > max.x {
            return VisibleBounds {
                min: Vec3::ZERO,
                max: Vec3::ZERO,
            };
        }
        let centre = (min + max) / 2.0;
        let half = (max - min) / 2.0 * COMPUTED_BOUNDS_GROWTH;
        VisibleBounds {
            min: (centre - half) / MODEL_UNITS_PER_BLOCK,
            max: (centre + half) / MODEL_UNITS_PER_BLOCK,
        }
    }
}

fn check_limits(geometry: &Geometry) -> Result<(), GeometryError> {
    let bone_count = geometry.bones.len();
    if bone_count == 0 {
        return Err(GeometryError::NoBones);
    }
    if bone_count > MAX_BONES {
        return Err(GeometryError::TooManyBones { count: bone_count });
    }
    let cube_count: usize = geometry.bones.iter().map(|bone| bone.cubes.len()).sum();
    if cube_count > MAX_CUBES {
        return Err(GeometryError::TooManyCubes { count: cube_count });
    }
    Ok(())
}

/// The file index of the parent of each bone.
fn file_parents(
    geometry: &Geometry,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<Option<usize>>, GeometryError> {
    let bones = &geometry.bones;
    let find = |name: &str| {
        bones
            .iter()
            .position(|bone| bone.name.eq_ignore_ascii_case(name))
    };
    let mut parents = Vec::with_capacity(bones.len());
    for (index, bone) in bones.iter().enumerate() {
        if find(&bone.name) != Some(index) {
            return Err(GeometryError::DuplicateBone {
                name: bone.name.clone(),
            });
        }
        let parent = bone.parent.as_deref().and_then(|name| {
            let found = find(name);
            if found.is_none() {
                diagnostics.push(Diagnostic::MissingParent {
                    bone: bone.name.clone(),
                    parent: name.to_owned(),
                });
            }
            found
        });
        parents.push(parent);
    }
    Ok(parents)
}

fn check_chains(geometry: &Geometry, parents: &[Option<usize>]) -> Result<(), GeometryError> {
    for (index, bone) in geometry.bones.iter().enumerate() {
        let mut length = 1;
        let mut at = index;
        while let Some(Some(parent)) = parents.get(at) {
            if *parent == index || length > parents.len() {
                return Err(GeometryError::ParentCycle {
                    bone: bone.name.clone(),
                });
            }
            length += 1;
            at = *parent;
        }
        if length > MAX_DEPTH {
            return Err(GeometryError::TooDeep {
                bone: bone.name.clone(),
            });
        }
    }
    Ok(())
}

/// The file indices in an order where each parent comes before its children. Of the bones
/// whose parent is placed, the one first in the file comes next.
fn parent_first_order(parents: &[Option<usize>]) -> Vec<usize> {
    let mut placed = vec![false; parents.len()];
    let mut order = Vec::with_capacity(parents.len());
    while order.len() < parents.len() {
        let next = parents.iter().enumerate().position(|(index, parent)| {
            !placed.get(index).copied().unwrap_or(true)
                && parent.is_none_or(|parent| placed.get(parent).copied().unwrap_or(false))
        });
        let Some(next) = next else {
            break;
        };
        if let Some(slot) = placed.get_mut(next) {
            *slot = true;
        }
        order.push(next);
    }
    order
}

fn baked_bone(bone: &Bone, parent: Option<usize>) -> BakedBone {
    BakedBone {
        name: bone.name.clone(),
        parent,
        pivot: space::internal_point(bone.pivot),
        rotation: space::internal_rotation(bone.rotation),
        file_rotation: bone.rotation,
        never_render: bone.never_render,
        binding: bone.binding.clone(),
    }
}

fn push_locator(
    locators: &mut Vec<BakedLocator>,
    locator: &Locator,
    bone: usize,
    geometry: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if locators
        .iter()
        .any(|kept| kept.name.eq_ignore_ascii_case(&locator.name))
    {
        diagnostics.push(Diagnostic::DuplicateLocator {
            geometry: geometry.to_owned(),
            locator: locator.name.clone(),
        });
        return;
    }
    locators.push(BakedLocator {
        name: locator.name.clone(),
        bone,
        position: space::internal_point(locator.offset),
        rotation: space::internal_rotation(locator.rotation),
        ignore_inherited_scale: locator.ignore_inherited_scale,
    });
}

fn bake_cube(bone: &Bone, cube: &Cube, bone_index: usize, texture_size: Vec2) -> BakedCube {
    let [lo, hi] = space::internal_box(cube.origin, cube.size);
    let inflate = cube.inflate.unwrap_or(bone.inflate);
    let mut drawn_lo = lo - Vec3::splat(inflate);
    let mut drawn_hi = hi + Vec3::splat(inflate);
    let flat = drawn_lo.cmpgt(drawn_hi);
    let middle = (lo + hi) / 2.0;
    drawn_lo = Vec3::select(flat, middle, drawn_lo);
    drawn_hi = Vec3::select(flat, middle, drawn_hi);
    let pivot = cube.pivot.map_or(middle, space::internal_point);
    let rotation = space::internal_rotation(cube.rotation);
    let turn = space::euler(rotation);
    let transform = space::about(pivot, Affine3A::from_mat3(turn));
    let mirror = cube.mirror.unwrap_or(bone.mirror);
    let mut faces = Vec::with_capacity(Face::ALL.len());
    for face in Face::ALL {
        let texels = match &cube.uv {
            CubeUv::Box(origin) => {
                UvRect::of_box(face, *origin, cube.size.floor(), mirror).corners()
            }
            CubeUv::PerFace(per_face) => {
                let Some(face_uv) = per_face.get(face) else {
                    continue;
                };
                let rect = UvRect::of_face(face, face_uv, cube.size);
                uv_rect::rotate_corners(rect.corners(), face_uv.rotation)
            }
        };
        faces.push(BakedFace {
            face,
            corners: face
                .corners(drawn_lo, drawn_hi)
                .map(|corner| transform.transform_point3(corner)),
            normal: turn * face.normal(),
            uvs: texels.map(|texel| texel / texture_size),
        });
    }
    BakedCube {
        bone: bone_index,
        shape: CuboidShape {
            origin: lo,
            size: cube.size,
            inflate,
            rotation,
            pivot,
            box_uv: match cube.uv {
                CubeUv::Box(origin) => Some(BoxUv { origin, mirror }),
                CubeUv::PerFace(_) => None,
            },
        },
        faces,
    }
}
