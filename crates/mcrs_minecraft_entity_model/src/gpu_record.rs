use core::fmt;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::baked_geometry::{BakedBone, BakedCube, BakedGeometry};

/// The `parent` of a [`BoneRecord`] of a root bone.
pub const NO_PARENT: u16 = u16::MAX;
/// The step of [`CuboidRecord::inflate`] in model units.
pub const INFLATE_STEP: f32 = 1.0 / 8192.0;
/// The bit of [`CuboidRecord::flags`] for a mirrored box UV. Bits 0 to 5 are the face mask,
/// by [`Face`] index.
pub const CUBOID_MIRROR: u8 = 1 << 6;
/// The bit of [`CuboidRecord::flags`] for a cuboid whose faces are explicit face records.
pub const CUBOID_EXPLICIT: u8 = 1 << 7;
/// The bit of [`BoneRecord::flags`] for a bone that is never drawn.
pub const BONE_NEVER_RENDER: u32 = 1;

/// The 64-byte record of one model in the `models` buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ModelRecord {
    /// The centre of the sphere about the visible bounds, in blocks.
    pub bound_centre: [f32; 3],
    pub bound_radius: f32,
    /// The visible bounds in blocks.
    pub visible_min: [f32; 3],
    pub bone_count: u32,
    pub visible_max: [f32; 3],
    /// The index of the first bone of the model in the `bones` buffer.
    pub first_bone: u32,
    pub texture_size: [f32; 2],
    /// The index of the first cuboid of the model in the `cuboids` buffer.
    pub first_cuboid: u32,
    pub cuboid_count: u32,
}

/// The 48-byte record of one bone in the `bones` buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct BoneRecord {
    pub pivot: [f32; 3],
    /// The model-local index of the parent bone, which comes first, or [`NO_PARENT`].
    pub parent: u16,
    /// The entry of the bone palette of the client entity definition.
    pub palette: u16,
    /// The rest rotation in degrees.
    pub rest_rotation: [f32; 3],
    pub flags: u32,
    pub rest_scale: [f32; 3],
    pub spare: u32,
}

/// The 32-byte record of one cuboid in the `cuboids` buffer. The vertex stage makes the
/// corners and the box UV of each face from it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CuboidRecord {
    /// The low corner before inflate, in the rest pose of the cube.
    pub origin: [f32; 3],
    /// The size before inflate.
    pub size: [f32; 3],
    /// The texel of the box UV origin.
    pub uv: [i16; 2],
    /// The inflate in steps of [`INFLATE_STEP`].
    pub inflate: i16,
    /// The model-local index of the bone.
    pub bone: u8,
    /// The face mask, [`CUBOID_MIRROR`] and [`CUBOID_EXPLICIT`].
    pub flags: u8,
}

const _: () = assert!(size_of::<ModelRecord>() == 64);
const _: () = assert!(size_of::<BoneRecord>() == 48);
const _: () = assert!(size_of::<CuboidRecord>() == 32);

/// A cube that the 32-byte cuboid record cannot hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordError {
    /// The cube has a rotation of its own.
    Rotated,
    /// The cube has a per-face UV.
    PerFaceUv,
    /// The UV origin is not a whole texel in the range of `i16`.
    UvOrigin,
    /// The inflate is outside the range of the record.
    Inflate,
    /// The bone index does not fit in a byte.
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
    /// A record that points to the explicit faces of a cube. The faces of `mask` follow each
    /// other from `first_face`, in face order.
    #[must_use]
    pub fn new_explicit(first_face: u32, mask: u8, bone: u8) -> Self {
        Self {
            origin: [f32::from_bits(first_face), 0.0, 0.0],
            bone,
            flags: mask | CUBOID_EXPLICIT,
            ..Self::default()
        }
    }

    /// The index of the first explicit face of the model that this record points to, or
    /// `None` for a record that holds its box.
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

/// `value` is a whole number in the range of `i16`.
fn whole_i16(value: f32) -> i16 {
    value as i16
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}
