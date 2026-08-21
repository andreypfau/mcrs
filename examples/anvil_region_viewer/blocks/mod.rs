mod build;
mod tint;

use bevy::math::Vec3;

use crate::anvil::BlockStateKey;
use crate::atlas::{Opacity, SpriteRef, SpriteRegistry};
use crate::bake::{Dir, TinyWorld};
use crate::pack::{MAX_SPRITES, MAX_SPRITE_ARRAYS};

pub use build::Fluid;
pub use tint::tint_square;

use build::build_one;
use tint::extend_tints;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Pass {
    Solid = 0,
    Cutout = 1,
    Translucent = 2,
}

impl Pass {
    pub const COUNT: usize = 3;

    pub const fn from_index(index: usize) -> Pass {
        match index {
            0 => Pass::Solid,
            1 => Pass::Cutout,
            _ => Pass::Translucent,
        }
    }

    fn of(opacity: Opacity) -> Pass {
        match opacity {
            Opacity::Solid => Pass::Solid,
            Opacity::Cutout => Pass::Cutout,
            Opacity::Translucent => Pass::Translucent,
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
    pub cull: Option<Dir>,
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

pub struct Catalog {
    pub blocks: Vec<BlockInfo>,
    pub sprites: SpriteRegistry,
    pub tints: Vec<[f32; 4]>,
    pub failures: Vec<String>,
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

pub fn cube_corner(dir: Dir, corner: usize) -> Vec3 {
    let a = FACE_AXES[dir as usize];
    let (cu, cv) = (CORNER_UV[corner][0], CORNER_UV[corner][1]);
    let mut p = [0.0f32; 3];
    p[a[0] as usize] = if a[1] == 1 { 1.0 } else { 0.0 };
    p[a[2] as usize] = if a[3] == 1 { cu } else { 1.0 - cu };
    p[a[4] as usize] = if a[5] == 1 { cv } else { 1.0 - cv };
    Vec3::from(p)
}
pub fn empty() -> Catalog {
    Catalog {
        blocks: Vec::new(),
        sprites: SpriteRegistry::new(),
        tints: vec![[1.0, 1.0, 1.0, 1.0]],
        failures: Vec::new(),
    }
}

pub fn extend(catalog: &mut Catalog, states: &[BlockStateKey], biomes: &[String]) {
    let neighbours = TinyWorld::default();
    for state in &states[catalog.blocks.len()..] {
        match build_one(state, &neighbours, &mut catalog.sprites) {
            Ok(info) => catalog.blocks.push(info),
            Err(reason) => {
                catalog.failures.push(format!("{}: {reason}", state.label()));
                catalog.blocks.push(BlockInfo::default());
            }
        }
    }
    assert!(
        catalog.sprites.arrays().len() <= MAX_SPRITE_ARRAYS,
        "the pack uses {} sprite resolutions, but a packed quad can address only \
         {MAX_SPRITE_ARRAYS} arrays",
        catalog.sprites.arrays().len(),
    );
    for array in catalog.sprites.arrays() {
        assert!(
            array.stills() + catalog.sprites.animations().len() <= MAX_SPRITES,
            "{} still sprites are {}x{} and {} animations sit above them, but a quad can name \
             only {MAX_SPRITES} layers",
            array.stills(),
            array.size,
            array.size,
            catalog.sprites.animations().len(),
        );
    }

    extend_tints(catalog, biomes);
}
