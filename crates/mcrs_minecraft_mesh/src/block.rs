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

#[derive(Clone, Default)]
pub struct BlockInfo {
    pub cube: Option<[CubeFace; 6]>,
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
