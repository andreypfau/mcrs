use crate::anvil::{SECTION_SIZE, SECTION_VOLUME, World};
use crate::blocks::{BlockInfo, FACE_AXES, Pass};
use crate::pack::{FACE_NONE, QUAD_WORDS};

use super::fluid::{COVER_SEE_THROUGH, FLUID_LAVA, Sloped, fluid_kind};

pub(super) const BORDER: usize = SECTION_SIZE + 2;
pub(super) const BORDER_VOLUME: usize = BORDER * BORDER * BORDER;
pub(super) const COLUMNS: usize = BORDER * BORDER;

pub(super) const FACE_GROUPS: usize = FACE_NONE as usize + 1;
pub(super) const FLUID_KINDS: usize = 2;

pub(super) const NOT_FLAT: u8 = 0xff;

const GRID: usize = SECTION_SIZE * SECTION_SIZE;

pub struct Scratch {
    pub(super) states: Box<[u16; BORDER_VOLUME]>,
    pub(super) occludes: Box<[bool; BORDER_VOLUME]>,
    pub(super) light: Box<[u8; BORDER_VOLUME]>,
    pub(super) cube_columns: Box<[[u32; COLUMNS]; 3]>,
    pub(super) occlude_columns: Box<[[u32; COLUMNS]; 3]>,
    pub(super) fluid: Box<[u8; BORDER_VOLUME]>,
    pub(super) cover: Box<[u8; BORDER_VOLUME]>,
    pub(super) fluid_kinds: u8,
    pub(super) fluid_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    pub(super) flat_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    pub(super) flat_drop: Box<[u8; SECTION_VOLUME]>,
    pub(super) sloped: Vec<Sloped>,
    pub(super) faces: Box<[u32; GRID]>,
    pub(super) passes: Box<[u8; GRID]>,
    pub(super) attrs: Box<[u32; GRID]>,
    pub(super) used: Box<[bool; GRID]>,
    pub(super) simple_by_pass: [Vec<[u32; QUAD_WORDS]>; Pass::COUNT],
    pub(super) section_faces: Vec<u32>,
    pub(super) complex_by_pass: [[Vec<u32>; FACE_GROUPS]; Pass::COUNT],
}

#[derive(Copy, Clone)]
pub(super) enum Columns {
    Cubes,
    Fluid(usize),
}

impl Scratch {
    pub fn new() -> Self {
        Self {
            states: Box::new([0; BORDER_VOLUME]),
            occludes: Box::new([false; BORDER_VOLUME]),
            light: Box::new([0; BORDER_VOLUME]),
            cube_columns: Box::new([[0; COLUMNS]; 3]),
            occlude_columns: Box::new([[0; COLUMNS]; 3]),
            fluid: Box::new([0; BORDER_VOLUME]),
            cover: Box::new([0; BORDER_VOLUME]),
            fluid_kinds: 0,
            fluid_columns: Box::new([[[0; COLUMNS]; 3]; FLUID_KINDS]),
            flat_columns: Box::new([[[0; COLUMNS]; 3]; FLUID_KINDS]),
            flat_drop: Box::new([NOT_FLAT; SECTION_VOLUME]),
            sloped: Vec::new(),
            faces: Box::new([0; GRID]),
            passes: Box::new([0; GRID]),
            attrs: Box::new([0; GRID]),
            used: Box::new([false; GRID]),
            simple_by_pass: Default::default(),
            section_faces: Vec::new(),
            complex_by_pass: Default::default(),
        }
    }

    pub(super) fn load(&mut self, world: &World, catalog: &[BlockInfo], base: [i32; 3]) {
        *self.cube_columns = [[0; COLUMNS]; 3];
        *self.occlude_columns = [[0; COLUMNS]; 3];
        *self.fluid_columns = [[[0; COLUMNS]; 3]; FLUID_KINDS];
        self.fluid_kinds = 0;
        for y in -1..=SECTION_SIZE as i32 {
            for z in -1..=SECTION_SIZE as i32 {
                for x in -1..=SECTION_SIZE as i32 {
                    let index = border_index(x, y, z);
                    let state = world.block(base[0] + x, base[1] + y, base[2] + z);
                    let info = &catalog[state as usize];
                    self.states[index] = state;
                    self.occludes[index] = info.occludes;
                    self.light[index] = world.light(base[0] + x, base[1] + y, base[2] + z);
                    let fluid = info.fluid.map_or(0, |fluid| {
                        fluid.amount | if fluid.lava { FLUID_LAVA } else { 0 }
                    });
                    self.fluid[index] = fluid;
                    let see_through = info.cube.is_some() && !info.occludes;
                    self.cover[index] =
                        info.sturdy | if see_through { COVER_SEE_THROUGH } else { 0 };
                    if !info.occludes && info.cube.is_none() && fluid == 0 {
                        continue;
                    }
                    if fluid != 0 && inside(x) && inside(y) && inside(z) {
                        self.fluid_kinds |= 1 << fluid_kind(fluid);
                    }
                    let along = [x, y, z];
                    for axis in 0..3 {
                        let column = column_index(axis, x, y, z);
                        let bit = 1u32 << (along[axis] + 1);
                        if info.cube.is_some() {
                            self.cube_columns[axis][column] |= bit;
                        }
                        if info.occludes {
                            self.occlude_columns[axis][column] |= bit;
                        }
                        if fluid != 0 {
                            self.fluid_columns[fluid_kind(fluid)][axis][column] |= bit;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn visible(
        &self,
        columns: Columns,
        axis: usize,
        column: usize,
        n_positive: bool,
    ) -> u32 {
        let (own, blocker) = match columns {
            Columns::Cubes => (
                self.cube_columns[axis][column],
                self.occlude_columns[axis][column],
            ),
            Columns::Fluid(kind) => (
                self.flat_columns[kind][axis][column],
                self.fluid_columns[kind][axis][column],
            ),
        };
        let front = if n_positive { blocker >> 1 } else { blocker << 1 };
        own & !front
    }
}

#[inline]
pub(super) fn inside(coordinate: i32) -> bool {
    (0..SECTION_SIZE as i32).contains(&coordinate)
}

#[inline]
pub(super) fn border_index(x: i32, y: i32, z: i32) -> usize {
    ((y + 1) as usize) * BORDER * BORDER + ((z + 1) as usize) * BORDER + (x + 1) as usize
}

#[inline]
pub(super) fn column_index(axis: usize, x: i32, y: i32, z: i32) -> usize {
    let (p, q) = match axis {
        0 => (y, z),
        1 => (x, z),
        _ => (x, y),
    };
    (p + 1) as usize * BORDER + (q + 1) as usize
}

#[inline]
pub(super) fn section_cell(x: i32, y: i32, z: i32) -> usize {
    (y as usize * SECTION_SIZE + z as usize) * SECTION_SIZE + x as usize
}

#[inline]
pub(super) fn grid_to_local(grid: usize, positive: bool) -> i32 {
    if positive {
        grid as i32
    } else {
        (SECTION_SIZE - 1 - grid) as i32
    }
}

#[inline]
pub(super) fn face_axis(face: usize, local: &mut [i32; 3], gu: usize, gv: usize) {
    let axes = FACE_AXES[face];
    local[axes[2] as usize] = grid_to_local(gu, axes[3] == 1);
    local[axes[4] as usize] = grid_to_local(gv, axes[5] == 1);
}
