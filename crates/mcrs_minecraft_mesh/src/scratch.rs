use crate::ambient::{self, Neighbour, Sample};
use crate::axis_mut;
use crate::block::{AIR, BlockInfo, Pass, face_axes};
use crate::pack::{FACE_NONE, FACE_WORDS, QUAD_WORDS};
use crate::{SECTION_SIZE, SECTION_VOLUME};
use mcrs_minecraft_core::{BlockPos, LocalPos, SectionPos};
use mcrs_minecraft_volume::{BlockVolume, LightVolume, SectionBlocks};

use super::fluid::{COVER_SEE_THROUGH, FLUID_LAVA, Sloped, fluid_kind};

pub(super) const BORDER: usize = SECTION_SIZE + 2;
pub(super) const BORDER_VOLUME: usize = BORDER * BORDER * BORDER;
pub(super) const COLUMNS: usize = BORDER * BORDER;

/// Occlusion probes the block past a face's neighbour, so they reach one block further out than
/// anything else the mesher reads.
const APRON: usize = SECTION_SIZE + 4;
const APRON_VOLUME: usize = APRON * APRON * APRON;

pub(super) const FACE_GROUPS: usize = FACE_NONE as usize + 1;
pub(super) const FLUID_KINDS: usize = 2;

pub(super) const NOT_FLAT: u8 = 0xff;

const GRID: usize = SECTION_SIZE * SECTION_SIZE;

pub struct Scratch {
    pub(super) states: Box<[u16; BORDER_VOLUME]>,
    pub(super) occludes: Box<[bool; BORDER_VOLUME]>,
    neighbours: Box<[Neighbour; APRON_VOLUME]>,
    pub(super) coords: Box<[u32; BORDER_VOLUME]>,
    pub(super) light: Box<[u8; BORDER_VOLUME]>,
    pub(super) cube_columns: Box<[[u32; COLUMNS]; 3]>,
    pub(super) occlude_columns: Box<[[u32; COLUMNS]; 3]>,
    /// Lowered boxes along each vertical column, whose top shows whatever lies above them.
    lowered_columns: Box<[u32; COLUMNS]>,
    pub(super) fluid: Box<[u8; BORDER_VOLUME]>,
    pub(super) cover: Box<[u8; BORDER_VOLUME]>,
    pub(super) fluid_kinds: u8,
    pub(super) fluid_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    pub(super) flat_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    pub(super) flat_drop: Box<[u8; SECTION_VOLUME]>,
    pub(super) sloped: Vec<Sloped>,
    pub(super) faces: Box<[u32; GRID]>,
    pub(super) passes: Box<[u8; GRID]>,
    pub(super) attrs: Box<[[u32; FACE_WORDS]; GRID]>,
    pub(super) used: Box<[bool; GRID]>,
    pub(super) simple_by_pass: [Vec<[u32; QUAD_WORDS]>; Pass::COUNT],
    pub(super) section_faces: Vec<[u32; FACE_WORDS]>,
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
            neighbours: Box::new([Neighbour::default(); APRON_VOLUME]),
            coords: Box::new([0; BORDER_VOLUME]),
            light: Box::new([0; BORDER_VOLUME]),
            cube_columns: Box::new([[0; COLUMNS]; 3]),
            occlude_columns: Box::new([[0; COLUMNS]; 3]),
            lowered_columns: Box::new([0; COLUMNS]),
            fluid: Box::new([0; BORDER_VOLUME]),
            cover: Box::new([0; BORDER_VOLUME]),
            fluid_kinds: 0,
            fluid_columns: Box::new([[[0; COLUMNS]; 3]; FLUID_KINDS]),
            flat_columns: Box::new([[[0; COLUMNS]; 3]; FLUID_KINDS]),
            flat_drop: Box::new([NOT_FLAT; SECTION_VOLUME]),
            sloped: Vec::new(),
            faces: Box::new([0; GRID]),
            passes: Box::new([0; GRID]),
            attrs: Box::new([[0; FACE_WORDS]; GRID]),
            used: Box::new([false; GRID]),
            simple_by_pass: Default::default(),
            section_faces: Vec::new(),
            complex_by_pass: Default::default(),
        }
    }

    #[expect(
        clippy::excessive_nesting,
        reason = "loads the section and its apron cell by cell, filling the light, fluid and cube column bits in the same pass so each cell is read once"
    )]
    pub(super) fn load<W>(&mut self, world: &W, catalog: &[BlockInfo], base: [i32; 3])
    where
        W: BlockVolume + LightVolume,
    {
        // The section and its apron fall in the 3³ sections around it, each looked up once.
        let origin = SectionPos::from(BlockPos::new(base[0], base[1], base[2]));
        let around: [_; 27] = std::array::from_fn(|i| {
            let (dx, dy, dz) = (i as i32 % 3 - 1, i as i32 / 9 - 1, i as i32 / 3 % 3 - 1);
            world
                .section(SectionPos::new(origin.x + dx, origin.y + dy, origin.z + dz))
                .ok()
        });
        let block = |pos: BlockPos| {
            let at = SectionPos::from(pos);
            let (dx, dy, dz) = (
                at.x - origin.x + 1,
                at.y - origin.y + 1,
                at.z - origin.z + 1,
            );
            around
                .get((dy * 9 + dz * 3 + dx) as usize)
                .and_then(Option::as_ref)
                .map_or(0, |section| section.get(LocalPos::from(pos)).number())
        };
        *self.cube_columns = [[0; COLUMNS]; 3];
        *self.occlude_columns = [[0; COLUMNS]; 3];
        *self.lowered_columns = [0; COLUMNS];
        *self.fluid_columns = [[[0; COLUMNS]; 3]; FLUID_KINDS];
        self.fluid_kinds = 0;
        for y in -1..=SECTION_SIZE as i32 {
            for z in -1..=SECTION_SIZE as i32 {
                for x in -1..=SECTION_SIZE as i32 {
                    let index = border_index(x, y, z);
                    let pos = BlockPos::new(base[0] + x, base[1] + y, base[2] + z);
                    let state = block(pos);
                    // A server whose block corpus differs from ours can name a state
                    // this catalog has no row for; it is drawn as air rather than
                    // taking the frame down.
                    let (state, info) = match catalog.get(state as usize) {
                        Some(info) => (state, info),
                        None => (0, block_info(catalog, 0)),
                    };
                    put(&mut *self.states, index, state);
                    put(&mut *self.occludes, index, info.occludes);
                    put(&mut *self.neighbours, apron_index(x, y, z), info.neighbour);
                    // Light a store cannot give is open sky, as outside every loaded column.
                    let light = world.light(pos).unwrap_or(0x0f);
                    put(&mut *self.light, index, light);
                    let coords = ambient::light_coords(info.emissive, info.emission, light);
                    put(&mut *self.coords, index, coords);
                    let fluid = info.fluid.map_or(0, |fluid| {
                        fluid.amount | if fluid.lava { FLUID_LAVA } else { 0 }
                    });
                    put(&mut *self.fluid, index, fluid);
                    let see_through = info.cube.is_some() && info.drop == 0 && !info.occludes;
                    let cover = info.sturdy | if see_through { COVER_SEE_THROUGH } else { 0 };
                    put(&mut *self.cover, index, cover);
                    if !info.occludes && info.cube.is_none() && fluid == 0 {
                        continue;
                    }
                    if fluid != 0 && inside(x) && inside(y) && inside(z) {
                        self.fluid_kinds |= 1 << fluid_kind(fluid);
                    }
                    let mut fluids = (fluid != 0)
                        .then(|| self.fluid_columns.get_mut(fluid_kind(fluid)))
                        .flatten();
                    for (axis, ((along, cubes), occluders)) in [x, y, z]
                        .into_iter()
                        .zip(self.cube_columns.iter_mut())
                        .zip(self.occlude_columns.iter_mut())
                        .enumerate()
                    {
                        let column = column_index(axis, x, y, z);
                        let bit = 1u32 << (along + 1);
                        if info.cube.is_some() {
                            set_bit(cubes, column, bit);
                            if axis == 1 && info.drop != 0 {
                                set_bit(&mut *self.lowered_columns, column, bit);
                            }
                        }
                        if info.occludes {
                            set_bit(occluders, column, bit);
                        }
                        if let Some(fluids) = fluids.as_mut().and_then(|f| f.get_mut(axis)) {
                            set_bit(fluids, column, bit);
                        }
                    }
                }
            }
        }

        let apron = -2..=SECTION_SIZE as i32 + 1;
        for y in apron.clone() {
            for z in apron.clone() {
                for x in apron.clone() {
                    if border(x) && border(y) && border(z) {
                        continue;
                    }
                    let state =
                        block(BlockPos::new(base[0] + x, base[1] + y, base[2] + z)) as usize;
                    let neighbour = catalog
                        .get(state)
                        .map_or(Neighbour::default(), |info| info.neighbour);
                    put(&mut *self.neighbours, apron_index(x, y, z), neighbour);
                }
            }
        }
    }

    /// What smooth lighting reads of the block at a border position.
    pub(super) fn sample(&self, x: i32, y: i32, z: i32) -> Sample {
        let neighbour = self
            .neighbours
            .get(apron_index(x, y, z))
            .copied()
            .unwrap_or_default();
        let light = if border(x) && border(y) && border(z) {
            cell(&*self.coords, border_index(x, y, z))
        } else {
            0
        };
        Sample { neighbour, light }
    }

    pub(super) fn visible(
        &self,
        columns: Columns,
        axis: usize,
        column: usize,
        n_positive: bool,
    ) -> u32 {
        let along = |columns: &[[u32; COLUMNS]; 3]| {
            columns.get(axis).map_or(0, |columns| cell(columns, column))
        };
        let (own, blocker) = match columns {
            Columns::Cubes => (along(&self.cube_columns), along(&self.occlude_columns)),
            Columns::Fluid(kind) => (
                self.flat_columns.get(kind).map_or(0, along),
                self.fluid_columns.get(kind).map_or(0, along),
            ),
        };
        let front = if n_positive {
            blocker >> 1
        } else {
            blocker << 1
        };
        let shown = match columns {
            Columns::Cubes if axis == 1 && n_positive => {
                !front | cell(&*self.lowered_columns, column)
            }
            _ => !front,
        };
        own & shown
    }

    /// The pass key of a grid slot that holds a face no quad has taken yet.
    #[inline]
    pub(super) fn open(&self, slot: usize) -> Option<u8> {
        match (self.used.get(slot), self.passes.get(slot)) {
            (Some(false), Some(&key)) => Some(key),
            _ => None,
        }
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

/// The catalog row of a state, where a state the catalog has no row for is air.
#[inline]
pub(super) fn block_info(catalog: &[BlockInfo], state: u16) -> &BlockInfo {
    catalog.get(state as usize).unwrap_or(&AIR)
}

/// A cell past the end of its array reads as the zero value, as an empty cell does.
#[inline]
pub(super) fn cell<T: Copy + Default>(cells: &[T], index: usize) -> T {
    cells.get(index).copied().unwrap_or_default()
}

#[inline]
pub(super) fn put<T>(cells: &mut [T], index: usize, value: T) {
    if let Some(cell) = cells.get_mut(index) {
        *cell = value;
    }
}

#[inline]
pub(super) fn set_bit(columns: &mut [u32], column: usize, bit: u32) {
    if let Some(column) = columns.get_mut(column) {
        *column |= bit;
    }
}

#[inline]
pub(super) fn inside(coordinate: i32) -> bool {
    (0..SECTION_SIZE as i32).contains(&coordinate)
}

#[inline]
fn border(coordinate: i32) -> bool {
    (-1..=SECTION_SIZE as i32).contains(&coordinate)
}

#[inline]
fn apron_index(x: i32, y: i32, z: i32) -> usize {
    ((y + 2) as usize * APRON + (z + 2) as usize) * APRON + (x + 2) as usize
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
    let [_, _, u_axis, u_positive, v_axis, v_positive] = face_axes(face);
    *axis_mut(local, u_axis) = grid_to_local(gu, u_positive == 1);
    *axis_mut(local, v_axis) = grid_to_local(gv, v_positive == 1);
}
