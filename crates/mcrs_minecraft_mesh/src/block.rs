use bevy_math::Vec3;
use mcrs_minecraft_core::Direction;

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct SpriteRef {
    pub array: u8,
    pub layer: u16,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fluid {
    pub lava: bool,
    pub amount: u8,
    pub still: SpriteRef,
    pub flow: SpriteRef,
    pub overlay: Option<SpriteRef>,
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TintKind {
    Grass = 0,
    Foliage = 1,
    Water = 2,
}

pub const TINT_KINDS: usize = 3;

#[derive(Copy, Clone, Default)]
pub struct CubeFace {
    pub sprite: SpriteRef,
    pub pass: u8,
    pub tinted: bool,
}

#[derive(Clone)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub cull: Option<Direction>,
    pub face: Option<u8>,
    pub sprite: SpriteRef,
    pub pass: Pass,
    pub shade: [u8; 4],
    pub tinted: bool,
}

#[derive(Clone)]
pub struct BlockInfo {
    pub cube: Option<[CubeFace; 6]>,
    pub quads: Vec<ModelQuad>,
    pub occludes: bool,
    pub self_culls: bool,
    pub sturdy: u8,
    pub tint_kind: TintKind,
    pub emission: u8,
    pub fluid: Option<Fluid>,
}

impl Default for BlockInfo {
    fn default() -> Self {
        Self {
            cube: None,
            quads: Vec::new(),
            occludes: false,
            self_culls: false,
            sturdy: 0,
            tint_kind: TintKind::Grass,
            emission: 0,
            fluid: None,
        }
    }
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
