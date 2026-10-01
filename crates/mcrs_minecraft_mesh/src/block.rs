use bevy_math::Vec3;
use mcrs_minecraft_core::Direction;

use crate::ambient::Neighbour;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fluid {
    pub lava: bool,
    pub amount: u8,
    pub still: u16,
    pub flow: u16,
    pub overlay: Option<u16>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Pass {
    Solid = 0,
    Cutout = 1,
    Translucent = 2,
}

impl Pass {
    pub const COUNT: usize = 3;

    pub const ALL: [Pass; Pass::COUNT] = [Pass::Solid, Pass::Cutout, Pass::Translucent];

    pub const fn label(self) -> &'static str {
        match self {
            Pass::Solid => "solid",
            Pass::Cutout => "cutout",
            Pass::Translucent => "translucent",
        }
    }

    pub const fn translucent(self) -> bool {
        matches!(self, Pass::Translucent)
    }

    pub const fn writes_depth(self) -> bool {
        !self.translucent()
    }

    pub const fn from_index(index: usize) -> Pass {
        match index {
            0 => Pass::Solid,
            1 => Pass::Cutout,
            _ => Pass::Translucent,
        }
    }
}

use crate::tint::Tint;

#[derive(Copy, Clone, Default)]
pub struct CubeFace {
    pub sprite: u16,
    pub pass: u8,
    pub tint: Tint,
}

#[derive(Clone)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub cull: Option<Direction>,
    pub facing: Direction,
    pub face: Option<u8>,
    pub sprite: u16,
    pub pass: Pass,
    pub shade: f32,
    pub tint: Tint,
}

/// Rows of a side on a sixteenth grid, its two tangent axes taken in axis order.
pub type SideCells = [u16; 16];

/// Vanilla's face occlusion shapes: per side, the cells of that side the block's occlusion shape
/// reaches. `outer` rounds out, and is what a face of this block needs covered to be hidden;
/// `inner` rounds in, and is what this block covers of the face across from it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FaceShapes {
    pub outer: [SideCells; 6],
    pub inner: [SideCells; 6],
}

/// Vanilla's `Block.shouldRenderFace`, answered the other way round: whether the face this block
/// shows towards `side` is hidden by the neighbour across it.
pub fn face_hidden(own: &BlockInfo, neighbour: &BlockInfo, side: usize) -> bool {
    if neighbour.occludes {
        return true;
    }
    let (Some(own), Some(across)) = (own.faces.as_deref(), neighbour.faces.as_deref()) else {
        return false;
    };
    let face = &own.outer[side];
    let cover = &across.inner[side ^ 1];
    face.iter().any(|row| *row != 0) && face.iter().zip(cover).all(|(f, c)| f & !c == 0)
}

#[derive(Clone, Default)]
pub struct BlockInfo {
    pub cube: Option<[CubeFace; 6]>,
    /// `None` where the occlusion shape is empty, which hides no face and needs none hidden.
    pub faces: Option<Box<FaceShapes>>,
    /// How far the top of a box-shaped block sits below its cell's, in `MODEL_STEPS`, when its
    /// `cube` is a lowered box rather than a full cube: its bottom and top are then drawn as cube
    /// faces, the top dropped by this much, and its sides stay among its `quads`.
    pub drop: u8,
    pub quads: Vec<ModelQuad>,
    pub occludes: bool,
    pub self_culls: bool,
    pub sturdy: u8,
    pub emission: u8,
    pub emissive: bool,
    pub fluid: Option<Fluid>,
    pub neighbour: Neighbour,
    pub ambient_occlusion: bool,
}

pub const FACE_AXES: [[u8; 6]; 6] = [
    [1, 0, 0, 1, 2, 0],
    [1, 1, 0, 1, 2, 1],
    [2, 0, 0, 0, 1, 0],
    [2, 1, 0, 1, 1, 0],
    [0, 0, 2, 1, 1, 0],
    [0, 1, 2, 0, 1, 0],
];

pub const CORNER_UV: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
