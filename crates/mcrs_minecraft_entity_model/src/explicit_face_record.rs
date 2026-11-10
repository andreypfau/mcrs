use bytemuck::{Pod, Zeroable};

use crate::baked_geometry::BakedFace;

/// The bit of [`ExplicitFaceRecord::flags`] for a face of a mirrored box UV.
pub const EXPLICIT_FACE_MIRROR: u8 = 1;

/// The 64-byte record of one face of a cube that a [`crate::CuboidRecord`] cannot hold. It
/// shares the `cuboids` buffer and takes two 32-byte slots.
///
/// The corners `c0`, `c1`, `c2` and `c3` of the face ([`crate::Face::corners`]) are
/// `corner`, `corner + edge_2`, `corner + edge_1` and `corner + edge_1 + edge_2`, in the rest
/// space of the bone with the cube rotation applied. The outward normal is
/// `normalize(edge_1 × edge_2)`. The texture coordinates follow the same rule with `uv`,
/// `uv_edge_1` and `uv_edge_2`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ExplicitFaceRecord {
    /// The corner `c0`.
    pub corner: [f32; 3],
    /// From `c0` to `c2`.
    pub edge_1: [f32; 3],
    /// From `c0` to `c1`.
    pub edge_2: [f32; 3],
    /// The texture coordinate of `c0`, normalised by the texture size.
    pub uv: [f32; 2],
    pub uv_edge_1: [f32; 2],
    pub uv_edge_2: [f32; 2],
    /// The model-local index of the bone.
    pub bone: u8,
    /// [`EXPLICIT_FACE_MIRROR`].
    pub flags: u8,
    pub padding: [u8; 2],
}

const _: () = assert!(size_of::<ExplicitFaceRecord>() == 64);

impl ExplicitFaceRecord {
    #[must_use]
    pub fn new(face: &BakedFace, bone: u8, flags: u8) -> Self {
        let [c0, c1, c2, _] = face.corners;
        let [uv0, uv1, uv2, _] = face.uvs;
        Self {
            corner: c0.to_array(),
            edge_1: (c2 - c0).to_array(),
            edge_2: (c1 - c0).to_array(),
            uv: uv0.to_array(),
            uv_edge_1: (uv2 - uv0).to_array(),
            uv_edge_2: (uv1 - uv0).to_array(),
            bone,
            flags,
            padding: [0; 2],
        }
    }
}
