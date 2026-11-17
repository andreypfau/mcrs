use core::fmt;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::baked_geometry::{BakedBone, BakedCube, BakedGeometry};

pub const NO_PARENT: u16 = u16::MAX;
pub const INFLATE_STEP: f32 = 1.0 / 8192.0;
pub const CUBOID_MIRROR: u8 = 1 << 6;
pub const CUBOID_EXPLICIT: u8 = 1 << 7;
pub const BONE_NEVER_RENDER: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ModelRecord {
    pub bound_centre: [f32; 3],
    pub bound_radius: f32,
    pub visible_min: [f32; 3],
    pub bone_count: u32,
    pub visible_max: [f32; 3],
    pub first_bone: u32,
    pub texture_size: [f32; 2],
    pub first_cuboid: u32,
    pub cuboid_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct BoneRecord {
    pub pivot: [f32; 3],
    pub parent: u16,
    pub palette: u16,
    pub rest_rotation: [f32; 3],
    pub flags: u32,
    pub rest_scale: [f32; 3],
    pub spare: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CuboidRecord {
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub uv: [i16; 2],
    pub inflate: i16,
    pub bone: u8,
    pub flags: u8,
}

const _: () = assert!(size_of::<ModelRecord>() == 64);
const _: () = assert!(size_of::<BoneRecord>() == 48);
const _: () = assert!(size_of::<CuboidRecord>() == 32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordError {
    Rotated,
    PerFaceUv,
    UvOrigin,
    Inflate,
    Bone,
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            RecordError::Rotated => "the cube has a rotation",
            RecordError::PerFaceUv => "the cube has a per-face UV",
            RecordError::UvOrigin => "the UV origin is not a whole texel in range",
            RecordError::Inflate => "the inflate is out of range",
            RecordError::Bone => "the bone index does not fit in a byte",
        })
    }
}

impl core::error::Error for RecordError {}

impl ModelRecord {
    #[must_use]
    pub fn new(model: &BakedGeometry, first_bone: u32, first_cuboid: u32) -> Self {
        let bounds = model.visible_bounds;
        let centre = (bounds.min + bounds.max) / 2.0;
        Self {
            bound_centre: centre.to_array(),
            bound_radius: (bounds.max - centre).length(),
            visible_min: bounds.min.to_array(),
            bone_count: count(model.bones.len()),
            visible_max: bounds.max.to_array(),
            first_bone,
            texture_size: model.texture_size.to_array(),
            first_cuboid,
            cuboid_count: count(model.cubes.len()),
        }
    }
}

impl BoneRecord {
    #[must_use]
    pub fn new(bone: &BakedBone, palette: u16) -> Self {
        Self {
            pivot: bone.pivot.to_array(),
            parent: bone
                .parent
                .and_then(|parent| u16::try_from(parent).ok())
                .unwrap_or(NO_PARENT),
            palette,
            rest_rotation: bone.rotation.to_array(),
            flags: if bone.never_render {
                BONE_NEVER_RENDER
            } else {
                0
            },
            rest_scale: Vec3::ONE.to_array(),
            spare: 0,
        }
    }
}

impl CuboidRecord {
    #[must_use]
    pub fn new_explicit(first_face: u32, mask: u8, bone: u8) -> Self {
        Self {
            origin: [f32::from_bits(first_face), 0.0, 0.0],
            bone,
            flags: mask | CUBOID_EXPLICIT,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn first_explicit_face(&self) -> Option<u32> {
        (self.flags & CUBOID_EXPLICIT != 0).then(|| self.origin[0].to_bits())
    }
}

impl TryFrom<&BakedCube> for CuboidRecord {
    type Error = RecordError;

    fn try_from(cube: &BakedCube) -> Result<Self, Self::Error> {
        let shape = &cube.shape;
        if shape.rotation != Vec3::ZERO {
            return Err(RecordError::Rotated);
        }
        let box_uv = shape.box_uv.ok_or(RecordError::PerFaceUv)?;
        let texel = |value: f32| {
            let whole = value.round();
            (whole == value && whole.abs() <= f32::from(i16::MAX))
                .then_some(whole)
                .ok_or(RecordError::UvOrigin)
        };
        let steps = (shape.inflate / INFLATE_STEP).round();
        if steps.abs() > f32::from(i16::MAX) {
            return Err(RecordError::Inflate);
        }
        let mut flags = cube
            .faces
            .iter()
            .fold(0, |mask, face| mask | face.face.mask_bit());
        if box_uv.mirror {
            flags |= CUBOID_MIRROR;
        }
        Ok(Self {
            origin: shape.origin.to_array(),
            size: shape.size.to_array(),
            uv: [
                whole_i16(texel(box_uv.origin.x)?),
                whole_i16(texel(box_uv.origin.y)?),
            ],
            inflate: whole_i16(steps),
            bone: u8::try_from(cube.bone).map_err(|_| RecordError::Bone)?,
            flags,
        })
    }
}

fn whole_i16(value: f32) -> i16 {
    value as i16
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}
