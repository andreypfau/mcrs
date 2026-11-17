use glam::Vec3;
use mcrs_minecraft_core::Direction;

use crate::face_shade::FaceShade;
use crate::smooth_light::LightState;
use crate::state_variants::StateVariants;
use crate::uv_orientation::UvOrientation;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fluid {
    pub lava: bool,
    pub amount: u8,
    pub still: u16,
    pub flow: u16,
    pub overlay: Option<u16>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub enum Pass {
    #[default]
    Solid = 0,
    Cutout = 1,
    Translucent = 2,
}

impl Pass {
    pub const COUNT: usize = 3;

    pub const ALL: [Pass; Pass::COUNT] = [Pass::Solid, Pass::Cutout, Pass::Translucent];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Pass::Solid => "solid",
            Pass::Cutout => "cutout",
            Pass::Translucent => "translucent",
        }
    }

    #[must_use]
    pub const fn translucent(self) -> bool {
        matches!(self, Pass::Translucent)
    }

    #[must_use]
    pub const fn writes_depth(self) -> bool {
        !self.translucent()
    }

    pub(crate) fn of_mut<T>(self, table: &mut [T; Pass::COUNT]) -> &mut T {
        let [solid, cutout, translucent] = table;
        match self {
            Pass::Solid => solid,
            Pass::Cutout => cutout,
            Pass::Translucent => translucent,
        }
    }

    #[must_use]
    pub const fn from_index(index: usize) -> Option<Pass> {
        match index {
            0 => Some(Pass::Solid),
            1 => Some(Pass::Cutout),
            2 => Some(Pass::Translucent),
            _ => None,
        }
    }
}

use crate::tint::Tint;

#[derive(Copy, Clone, Default, Debug)]
pub struct CubeFace {
    pub sprite: u16,
    pub pass: u8,
    pub tint: Tint,
    pub uv: UvOrientation,
    pub emission: u8,
}

#[derive(Clone, Debug)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub cull: Option<Direction>,
    pub facing: Direction,
    pub face: Option<u8>,
    pub sprite: u16,
    pub pass: Pass,
    pub shade: FaceShade,
    pub tint: Tint,
    pub emission: u8,
}

pub type SideCells = [u16; 16];

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FaceShapes {
    pub outer: [SideCells; 6],
    pub inner: [SideCells; 6],
}

pub fn face_hidden(own: &BlockInfo, neighbour: &BlockInfo, side: usize) -> bool {
    if neighbour.occludes {
        return true;
    }
    let (Some(own), Some(across)) = (own.faces.as_deref(), neighbour.faces.as_deref()) else {
        return false;
    };
    let (Some(face), Some(cover)) = (own.outer.get(side), across.inner.get(side ^ 1)) else {
        return false;
    };
    face.iter().any(|row| *row != 0) && face.iter().zip(cover).all(|(f, c)| f & !c == 0)
}

#[derive(Clone, Default)]
pub struct BlockInfo {
    pub cube: Option<[CubeFace; 6]>,
    pub faces: Option<Box<FaceShapes>>,
    pub drop: u8,
    pub quads: Vec<ModelQuad>,
    pub occludes: bool,
    pub self_culls: bool,
    pub sturdy: u8,
    pub fluid: Option<Fluid>,
    pub light: LightState,
    pub ambient_occlusion: bool,
    pub variants: Option<Box<StateVariants>>,
}

pub(crate) static AIR: BlockInfo = BlockInfo {
    cube: None,
    faces: None,
    drop: 0,
    quads: Vec::new(),
    occludes: false,
    self_culls: false,
    sturdy: 0,
    fluid: None,
    light: LightState::AIR,
    ambient_occlusion: false,
    variants: None,
};

pub const FACE_AXES: [[u8; 6]; 6] = [
    [1, 0, 0, 1, 2, 0],
    [1, 1, 0, 1, 2, 1],
    [2, 0, 0, 0, 1, 0],
    [2, 1, 0, 1, 1, 0],
    [0, 0, 2, 1, 1, 0],
    [0, 1, 2, 0, 1, 0],
];

pub const fn face_axes(face: usize) -> [u8; 6] {
    let [down, up, north, south, west, east] = FACE_AXES;
    match face {
        0 => down,
        1 => up,
        2 => north,
        3 => south,
        4 => west,
        _ => east,
    }
}

pub(crate) const fn face_direction(face: usize) -> Direction {
    match face {
        0 => Direction::Down,
        1 => Direction::Up,
        2 => Direction::North,
        3 => Direction::South,
        4 => Direction::West,
        _ => Direction::East,
    }
}

pub const CORNER_UV: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
