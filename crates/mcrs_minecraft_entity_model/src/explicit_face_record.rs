use bytemuck::{Pod, Zeroable};

use crate::baked_geometry::BakedFace;

pub const EXPLICIT_FACE_MIRROR: u8 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ExplicitFaceRecord {
    pub corner: [f32; 3],
    pub edge_1: [f32; 3],
    pub edge_2: [f32; 3],
    pub uv: [f32; 2],
    pub uv_edge_1: [f32; 2],
    pub uv_edge_2: [f32; 2],
    pub bone: u8,
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
