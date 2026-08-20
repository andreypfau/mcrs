//! Turns the region into packed GPU geometry.
//!
//! Two meshers run over every section. Full cubes go through greedy merging into quads that the
//! vertex shader unpacks with no vertex buffer and no index buffer at all; everything else emits
//! its baked model quads as 12-byte vertices. Both write contiguous runs tagged with the section
//! and face they came from, so the culling compute shader can drop whole runs — including the
//! three-or-more face groups pointing away from the camera — before any vertex work happens.
//!
//! A greedy quad carries nothing but where it is and how big it is. What a face looks like — its
//! sprite, its light, its ambient occlusion — is written once per block face into a side buffer the
//! quad points at, and the fragment shader picks its own entry out of that run. Faces therefore
//! merge across block types and across lighting, and a real region file comes out with a little
//! under half the quads it used to.
//!
//! A section's faces are written in one run, and the head of the buffer is a table of where those
//! runs start. That is what keeps a quad down to two words: it names a place inside its own
//! section, which is fifteen bits, instead of a place in the whole render region, which is
//! twenty-one and would not have fitted beside everything else.

use crate::anvil::{SECTION_SIZE, SECTION_VOLUME, World};
use crate::atlas::SpriteRef;
use crate::blocks::{BlockInfo, CORNER_UV, FACE_AXES, Fluid, Pass, TintKind};
use crate::pack::{
    FACE_AO, FACE_ARRAY, FACE_BLOCK_LIGHT, FACE_FLUID, FACE_LAYER, FACE_SKY_LIGHT, FACE_TINT,
    GROUP_FACE, MODEL_ARRAY, MODEL_BLOCK_LIGHT, MODEL_LAYER, MODEL_OVERHANG, MODEL_SECTION,
    MODEL_SHADE, MODEL_SKY_LIGHT, MODEL_STEPS, MODEL_TINT, MODEL_U, MODEL_V, MODEL_X, MODEL_Y,
    FLUID_INSET, MODEL_Z, QUAD_DROP, QUAD_FACE, QUAD_FACE_BASE, QUAD_FLUID, QUAD_H,
    QUAD_SECTION, QUAD_W, QUAD_WORDS,
    QUAD_X, QUAD_Y, QUAD_Z, RENDER_REGION_X, RENDER_REGION_Y, RENDER_REGION_Z, RegionGrid,
    SECTION_FACE_TABLE,
};

/// `pass * 2 + kind`, where kind is 0 for greedy quads and 1 for baked model quads.
pub const STREAMS: usize = Pass::COUNT * 2;

/// Stream order is also draw order: opaque first, then alpha-tested, then blended.
pub const STREAM_NAMES: [&str; STREAMS] = [
    "solid greedy",
    "solid model",
    "cutout greedy",
    "cutout model",
    "translucent greedy",
    "translucent model",
];

/// The last face group means "points nowhere the culling pass knows": drawn whenever the section
/// is visible.
const FACE_NONE: u32 = 10;

/// The ten face groups model geometry can be sorted into — six axes and four horizontal diagonals
/// — plus one for the quads that point squarely along none of them and so are never backfacing as
/// a run.
const FACE_GROUPS: usize = 11;

const BORDER: usize = SECTION_SIZE + 2;
const BORDER_VOLUME: usize = BORDER * BORDER * BORDER;

/// A cell of the fluid grid: vanilla's fluid amount in the low nibble and the kind above it. Zero
/// is "no fluid at all", which is what makes the common case one test.
const FLUID_AMOUNT: u8 = 0x0f;
const FLUID_LAVA: u8 = 0x10;

/// The height of a fluid that has its own kind above it. Vanilla's amount stops at eight ninths,
/// so nine is the one value that means "as tall as the block" and can never be a raw amount.
const FLUID_FULL: u8 = 9;

/// What [`Scratch::flat_drop`] holds where the cell's fluid is not a flat box.
const NOT_FLAT: u8 = 0xff;

/// Above the six face bits of [`Scratch::cover`]: the block is a full cube you can see through —
/// glass, ice, a slime block, leaves. Vanilla shows a fluid its overlay texture against exactly
/// these, so that water behind a window reads as a flat sheet instead of a waterfall.
const COVER_SEE_THROUGH: u8 = 1 << 6;

/// The bits of a border column that fall inside the section, the border cell at either end left
/// out. A cell at `n` occupies bit `n + 1`.
const IN_SECTION: u32 = ((1 << SECTION_SIZE) - 1) << 1;

/// The two fluid kinds, which are numbered by [`FLUID_LAVA`] and never share a mask: water beside
/// lava hides neither face, and one grid for both would hide both.
const FLUID_KINDS: usize = 2;

/// One bit column per cell of a border-sized plane. A column runs along one axis and holds the
/// whole section plus its border on that axis, which is eighteen bits of a `u32`.
const COLUMNS: usize = BORDER * BORDER;

/// Bit `entry * 6 + exit` is set when a sight line can cross the section from face `entry` to face
/// `exit`. Faces follow [`FACE_AXES`] order, so the opposite face is `f ^ 1`.
pub const CONNECT_ALL: u64 = (1 << 36) - 1;

/// One contiguous run of quads from a single section and face group. The unit the compute shader
/// culls: 64 threads per run scatter its surviving quad indices into the visible list.
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Group {
    pub quad_base: u32,
    pub quad_count: u32,
    /// Where inside its render region the section this run came from sits, and the face group its
    /// quads point at. Laid out by `SECTION_INDEX` and `GROUP_FACE`.
    pub section: u32,
    /// Where this run's quads start counting among the quads of its own draw, culled runs included.
    /// A blended stream is laid into the visible list at this offset rather than wherever an atomic
    /// hands out, because the order that list holds is the order the quads are blended in, and an
    /// order the GPU picks afresh every frame is a different picture every frame.
    pub quad_prefix: u32,
}

/// The finished geometry of one render region, meshed on its own.
///
/// The render region is the unit here because it is also the unit a draw covers: one of these
/// becomes one contiguous run of groups per stream, and nothing outside it has to be touched to
/// place it. It is sixteen sections on a side horizontally and a region file is thirty-two, so a
/// render region never straddles two files.
pub struct Batch {
    pub region: usize,
    /// Packed greedy quads, [`QUAD_WORDS`] `u32` each.
    pub simple: Vec<[u32; QUAD_WORDS]>,
    /// [`SECTION_FACE_TABLE`] words saying where each section's faces start, then one packed
    /// attribute per block face. A quad's `QUAD_FACE_BASE` counts from its own section's start, so
    /// the two have to be placed as a pair. Empty when the region has no greedy geometry at all,
    /// which is when the table would describe nothing.
    pub faces: Vec<u32>,
    /// Packed model vertices, three `u32` each, four vertices per quad.
    pub complex: Vec<u32>,
    /// Culling groups in stream order, so each stream owns one contiguous run. `quad_base` is
    /// relative to this batch's own arenas until it is placed.
    pub groups: Vec<Group>,
    /// What each stream holds, which is also where its run of groups starts.
    pub spans: [StreamSpan; STREAMS],
    /// `(section inside this region, mask)`. Numbered inside the region rather than against any
    /// grid, so the walk can be given a different grid later and these laid into it again.
    /// Sections the mesher never touched are left alone and keep [`CONNECT_ALL`]: a section
    /// missing from the file is air, and defaulting it to "closed" would kill a sight-line walk
    /// on its very first step through open sky.
    pub connectivity: Vec<(u32, u64)>,
}

impl Batch {
    pub fn model_quads(&self) -> usize {
        self.complex.len() / 3 / 4
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct StreamSpan {
    pub group_count: u32,
    pub quad_count: u32,
}

pub struct Scratch {
    /// Section volume plus a one-block border, so neighbour lookups never re-enter [`Region`].
    states: Box<[u16; BORDER_VOLUME]>,
    occludes: Box<[bool; BORDER_VOLUME]>,
    light: Box<[u8; BORDER_VOLUME]>,
    /// The same two predicates again as bit columns, one array per axis the columns run along.
    /// Whether a block has a face at all is then a shift and two ands over a whole column at once,
    /// which is what keeps the expensive part — sprite, light, ambient occlusion — off the
    /// nine cells in ten that carry no face.
    cube_columns: Box<[[u32; COLUMNS]; 3]>,
    occlude_columns: Box<[[u32; COLUMNS]; 3]>,
    /// Vanilla's fluid state per border cell, as [`FLUID_AMOUNT`] and [`FLUID_LAVA`].
    fluid: Box<[u8; BORDER_VOLUME]>,
    /// Which of its six sides each border cell closes off, and whether it is see-through, laid out
    /// as [`BlockInfo::sturdy`] plus [`COVER_SEE_THROUGH`]. Held beside the blocks rather than read
    /// off the catalog because a fluid face asks it of two cells at once and the catalog is a wide
    /// struct reached by a random index.
    cover: Box<[u8; BORDER_VOLUME]>,
    /// Which fluid kinds the section itself holds, as a bit each. Most sections of a world hold
    /// none, and this is what lets the whole fluid mesher be skipped for them rather than sweeping
    /// six face groups over an empty grid.
    fluid_kinds: u8,
    /// Every cell holding a fluid of that kind, whatever height it reaches: a fluid face is hidden
    /// by its own kind however deep the neighbour is, which is the one rule that separates a fluid
    /// from a block of glass.
    fluid_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    /// The cells whose fluid is a flat box — one height across the whole surface and no flow — and
    /// so merges the way a cube does. Everything else is a shape of its own and goes to the model
    /// path, which for a real world is the shoreline and the waterfalls and nothing else.
    flat_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    /// How far a flat cell's surface sits below the top of its block, in [`MODEL_STEPS`], or
    /// [`NOT_FLAT`]. Indexed by [`section_cell`] rather than by the border, because only cells
    /// inside the section ever emit geometry.
    flat_drop: Box<[u8; SECTION_VOLUME]>,
    /// The fluid cells that are not flat boxes, with the surface worked out for each.
    sloped: Vec<Sloped>,
    /// For the face group being built, one column of face bits per cell of the slice grid.
    faces: Box<[u32; SECTION_SIZE * SECTION_SIZE]>,
    /// The slice being merged: which pass each cell draws in, which is the whole of what decides
    /// whether two faces may become one quad, and what each cell's face will look like.
    passes: Box<[u8; SECTION_SIZE * SECTION_SIZE]>,
    attrs: Box<[u32; SECTION_SIZE * SECTION_SIZE]>,
    used: Box<[bool; SECTION_SIZE * SECTION_SIZE]>,
    /// Geometry for the face group being built, split by pass so each run stays contiguous.
    simple_by_pass: [Vec<[u32; QUAD_WORDS]>; Pass::COUNT],
    /// The face attributes of the whole section, in the order the quads claimed them. Not split by
    /// pass the way the quads are: a quad needs its own run contiguous and nothing more, so the
    /// base it is packed with is already the one it will be read at.
    section_faces: Vec<u32>,
    complex_by_pass: [[Vec<u32>; FACE_GROUPS]; Pass::COUNT],
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
            faces: Box::new([0; SECTION_SIZE * SECTION_SIZE]),
            passes: Box::new([0; SECTION_SIZE * SECTION_SIZE]),
            attrs: Box::new([0; SECTION_SIZE * SECTION_SIZE]),
            used: Box::new([false; SECTION_SIZE * SECTION_SIZE]),
            simple_by_pass: Default::default(),
            section_faces: Vec::new(),
            complex_by_pass: Default::default(),
        }
    }
}

/// One worker's slice of the finished geometry, before it is sorted into streams.
struct Partial {
    simple: Vec<[u32; QUAD_WORDS]>,
    /// The face table, then the face attributes of every section that has any.
    faces: Vec<u32>,
    /// `(section inside its region, where its faces start)`, which is what fills the table.
    section_faces: Vec<(u32, u32)>,
    complex: Vec<u32>,
    /// `(stream, render region, group)` with `group.quad_base` relative to this worker's own arena.
    groups: Vec<(u32, u32, Group)>,
    /// `(section inside its region, mask)`.
    connectivity: Vec<(u32, u64)>,
}

/// One `draw_indirect`: the groups of a single stream that lie inside a single render region.
/// Draw order is stream order, so the list runs stream by stream.
#[derive(Copy, Clone, Default, Debug)]
pub struct Draw {
    pub stream: u32,
    /// Which render region's geometry this draws, so it can be taken out again when that region
    /// gives its room back.
    pub region: u32,
    /// The region's corner in blocks. Coordinates in a quad are relative to their section, so this
    /// is what puts the geometry back where it belongs.
    pub origin: [i32; 3],
    /// Where this region's sections start in the sight-line bitset.
    pub cave_base: u32,
    /// Where this region's face attributes start in their arena. A quad's own base counts from
    /// here, so this is the whole of what turns one into an index.
    pub face_base: u32,
    pub first_group: u32,
    pub group_count: u32,
    /// Every quad these groups hold, which is the most culling can let through.
    pub quad_count: u32,
}

/// Meshes one render region, reading whatever of the world is loaded around it.
///
/// Everything outside the loaded window reads as air and full sky, which is what a section on the
/// outer wall of the window should see. A section whose neighbouring file has not arrived yet
/// would read the same way and come out with a wall of faces down the seam, so the caller has to
/// hold this back until those files are in.
pub fn mesh_render_region(
    world: &World,
    catalog: &[BlockInfo],
    grid: RegionGrid,
    region: usize,
    scratch: &mut Scratch,
) -> Batch {
    let mut partial = Partial {
        simple: Vec::new(),
        faces: vec![0; SECTION_FACE_TABLE],
        section_faces: Vec::new(),
        complex: Vec::new(),
        groups: Vec::new(),
        connectivity: Vec::new(),
    };
    let [sx0, sy0, sz0] = grid.corner(region);
    let hi = [
        (sx0 + RENDER_REGION_X).min(world.sections[0]),
        (sy0 + RENDER_REGION_Y).min(world.sections[1]),
        (sz0 + RENDER_REGION_Z).min(world.sections[2]),
    ];
    for sz in sz0..hi[2] {
        for sx in sx0..hi[0] {
            for sy in sy0..hi[1] {
                if world.section(sx, sy, sz).is_some() {
                    mesh_section(world, catalog, grid, sx, sy, sz, scratch, &mut partial);
                }
            }
        }
    }

    // A quad's base counts from its own section, so the table that says where each section's run
    // begins is what makes one readable. Nothing to describe means no block at all rather than a
    // table of zeroes.
    if partial.section_faces.is_empty() {
        partial.faces.clear();
    }
    for (section, start) in &partial.section_faces {
        partial.faces[*section as usize] = *start;
    }

    // Stream order is draw order, and one draw covers one stream of one render region, so the
    // groups of a stream have to end up next to each other.
    let mut groups = Vec::with_capacity(partial.groups.len());
    let mut spans = [StreamSpan::default(); STREAMS];
    for stream in 0..STREAMS {
        let first = groups.len();
        let mut quads = 0u32;
        for &(from, _, group) in &partial.groups {
            if from as usize == stream {
                // Counted over the stream's whole run rather than its survivors, so that a run's
                // place in the visible list is the same whatever the culling pass makes of it.
                let mut group = group;
                group.quad_prefix = quads;
                quads += group.quad_count;
                groups.push(group);
            }
        }
        spans[stream] = StreamSpan {
            group_count: (groups.len() - first) as u32,
            quad_count: quads,
        };
    }

    Batch {
        region,
        simple: partial.simple,
        faces: partial.faces,
        complex: partial.complex,
        groups,
        spans,
        connectivity: partial.connectivity,
    }
}

#[allow(clippy::too_many_arguments)]
fn mesh_section(
    world: &World,
    catalog: &[BlockInfo],
    grid: RegionGrid,
    sx: usize,
    sy: usize,
    sz: usize,
    scratch: &mut Scratch,
    partial: &mut Partial,
) {
    let base = [
        (sx * SECTION_SIZE) as i32,
        (sy as i32 + world.min_section[1]) * SECTION_SIZE as i32,
        (sz * SECTION_SIZE) as i32,
    ];

    *scratch.cube_columns = [[0; COLUMNS]; 3];
    *scratch.occlude_columns = [[0; COLUMNS]; 3];
    *scratch.fluid_columns = [[[0; COLUMNS]; 3]; FLUID_KINDS];
    scratch.fluid_kinds = 0;
    for y in -1..=SECTION_SIZE as i32 {
        for z in -1..=SECTION_SIZE as i32 {
            for x in -1..=SECTION_SIZE as i32 {
                let index = border_index(x, y, z);
                let state = world.block(base[0] + x, base[1] + y, base[2] + z);
                let info = &catalog[state as usize];
                scratch.states[index] = state;
                scratch.occludes[index] = info.occludes;
                scratch.light[index] = world.light(base[0] + x, base[1] + y, base[2] + z);
                let fluid = info.fluid.map_or(0, |fluid| {
                    fluid.amount | if fluid.lava { FLUID_LAVA } else { 0 }
                });
                scratch.fluid[index] = fluid;
                // A full cube that hides nothing behind it is what vanilla calls half transparent,
                // and leaves answer the same way: a cube of a texture with holes in it.
                let see_through = info.cube.is_some() && !info.occludes;
                scratch.cover[index] =
                    info.sturdy | if see_through { COVER_SEE_THROUGH } else { 0 };
                if !info.occludes && info.cube.is_none() && fluid == 0 {
                    continue;
                }
                if fluid != 0 && inside(x) && inside(y) && inside(z) {
                    scratch.fluid_kinds |= 1 << fluid_kind(fluid);
                }
                let along = [x, y, z];
                for axis in 0..3 {
                    let column = column_index(axis, x, y, z);
                    let bit = 1u32 << (along[axis] + 1);
                    if info.cube.is_some() {
                        scratch.cube_columns[axis][column] |= bit;
                    }
                    if info.occludes {
                        scratch.occlude_columns[axis][column] |= bit;
                    }
                    if fluid != 0 {
                        scratch.fluid_columns[fluid_kind(fluid)][axis][column] |= bit;
                    }
                }
            }
        }
    }

    let (region_index, local_section) = grid.split(sx, sy, sz);
    let region_index = region_index as u32;
    let faces_at = partial.faces.len() as u32;
    fluid_surfaces(scratch);
    greedy(catalog, scratch, local_section, region_index, partial);
    fluid_greedy(catalog, scratch, local_section, region_index, partial);
    if !scratch.section_faces.is_empty() {
        partial.faces.extend_from_slice(&scratch.section_faces);
        partial.section_faces.push((local_section, faces_at));
    }
    complex(catalog, scratch, local_section, region_index, partial);
    partial
        .connectivity
        .push((local_section, connectivity(&mut scratch.occludes)));
}

/// Whether a border coordinate falls inside the section rather than on its skin.
#[inline]
fn inside(coordinate: i32) -> bool {
    (0..SECTION_SIZE as i32).contains(&coordinate)
}

#[inline]
fn border_index(x: i32, y: i32, z: i32) -> usize {
    ((y + 1) as usize) * BORDER * BORDER + ((z + 1) as usize) * BORDER + (x + 1) as usize
}

/// Which column of `axis` a border coordinate falls in. The coordinate along `axis` is the bit
/// inside the column, so it takes no part in the index.
#[inline]
fn column_index(axis: usize, x: i32, y: i32, z: i32) -> usize {
    let (p, q) = match axis {
        0 => (y, z),
        1 => (x, z),
        _ => (x, y),
    };
    (p + 1) as usize * BORDER + (q + 1) as usize
}

/// Which pairs of section faces a sight line can join, as bit `entry * 6 + exit`.
///
/// Scribbles over `occludes`, marking visited cells in it: this runs last in [`mesh_section`], and
/// filling the border at the start of the next section rewrites the whole array, so nothing
/// downstream ever sees the damage.
fn connectivity(occludes: &mut [bool; BORDER_VOLUME]) -> u64 {
    const N: i32 = SECTION_SIZE as i32;
    let mut mask = 0u64;
    let mut stack: Vec<[i32; 3]> = Vec::new();

    for sy in 0..N {
        for sz in 0..N {
            for sx in 0..N {
                if occludes[border_index(sx, sy, sz)] {
                    continue;
                }
                occludes[border_index(sx, sy, sz)] = true;
                stack.push([sx, sy, sz]);

                let mut touched = 0u8;
                while let Some([x, y, z]) = stack.pop() {
                    touched |= (y == 0) as u8
                        | ((y == N - 1) as u8) << 1
                        | ((z == 0) as u8) << 2
                        | ((z == N - 1) as u8) << 3
                        | ((x == 0) as u8) << 4
                        | ((x == N - 1) as u8) << 5;
                    for face in 0..6usize {
                        let n = face_normal(face);
                        let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                        if nx < 0 || ny < 0 || nz < 0 || nx >= N || ny >= N || nz >= N {
                            continue;
                        }
                        let index = border_index(nx, ny, nz);
                        if occludes[index] {
                            continue;
                        }
                        occludes[index] = true;
                        stack.push([nx, ny, nz]);
                    }
                }

                // Unioned per connected component, never globally: two separate tunnels through
                // one section must not be reported as joined.
                for entry in 0..6 {
                    if touched >> entry & 1 == 1 {
                        mask |= (touched as u64) << (entry * 6);
                    }
                }
            }
        }
    }
    mask
}

#[inline]
pub const fn face_normal(face: usize) -> [i32; 3] {
    let axes = FACE_AXES[face];
    let mut normal = [0i32; 3];
    normal[axes[0] as usize] = if axes[1] == 1 { 1 } else { -1 };
    normal
}

fn greedy(
    catalog: &[BlockInfo],
    scratch: &mut Scratch,
    local_section: u32,
    region_index: u32,
    partial: &mut Partial,
) {
    scratch.section_faces.clear();
    for face in 0..6usize {
        for pass in 0..Pass::COUNT {
            scratch.simple_by_pass[pass].clear();
        }
        let axes = FACE_AXES[face];
        let n_axis = axes[0] as usize;
        let u_axis = axes[2] as usize;
        let u_positive = axes[3] == 1;
        let v_axis = axes[4] as usize;
        let v_positive = axes[5] == 1;

        // Where the faces of this group are, a whole column of the section at a time: a block has
        // one where it carries a cube and the block one step along the axis does not occlude it.
        let n_positive = axes[1] == 1;
        let mut occupied = 0u32;
        for gv in 0..SECTION_SIZE {
            for gu in 0..SECTION_SIZE {
                let mut local = [0i32; 3];
                local[u_axis] = grid_to_local(gu, u_positive);
                local[v_axis] = grid_to_local(gv, v_positive);
                let column = column_index(n_axis, local[0], local[1], local[2]);
                let cubes = scratch.cube_columns[n_axis][column];
                let occludes = scratch.occlude_columns[n_axis][column];
                let front = if n_positive { occludes >> 1 } else { occludes << 1 };
                let visible = cubes & !front;
                scratch.faces[gv * SECTION_SIZE + gu] = visible;
                occupied |= visible;
            }
        }

        for n in 0..SECTION_SIZE {
            // The border occupies bit zero, so a block at `n` is the bit above it.
            let bit = 1u32 << (n + 1);
            if occupied & bit == 0 {
                continue;
            }
            let mut any = false;
            for gv in 0..SECTION_SIZE {
                for gu in 0..SECTION_SIZE {
                    let slot = gv * SECTION_SIZE + gu;
                    if scratch.faces[slot] & bit == 0 {
                        scratch.used[slot] = true;
                        continue;
                    }
                    let mut local = [0i32; 3];
                    local[n_axis] = n as i32;
                    local[u_axis] = grid_to_local(gu, u_positive);
                    local[v_axis] = grid_to_local(gv, v_positive);
                    // Asked one cell at a time even though the columns already found the face: a
                    // block that culls against its own kind is the one rule the two masks cannot
                    // express, because it turns on what the neighbour is rather than on what it
                    // hides.
                    match face_attr(catalog, scratch, local, face) {
                        Some((pass, attr)) => {
                            scratch.used[slot] = false;
                            // No drop: only a fluid surface stops short of the top of its block.
                            scratch.passes[slot] = pass;
                            scratch.attrs[slot] = attr;
                            any = true;
                        }
                        None => scratch.used[slot] = true,
                    }
                }
            }
            if any {
                merge_slice(scratch, face, n, local_section);
            }
        }

        for pass in 0..Pass::COUNT {
            let quads = std::mem::take(&mut scratch.simple_by_pass[pass]);
            if !quads.is_empty() {
                partial.groups.push((
                    (pass * 2) as u32,
                    region_index,
                    Group {
                        quad_base: partial.simple.len() as u32,
                        quad_count: quads.len() as u32,
                        section: local_section | GROUP_FACE.pack(face as u64) as u32,
                        quad_prefix: 0,
                    },
                ));
                partial.simple.extend_from_slice(&quads);
            }
            scratch.simple_by_pass[pass] = quads;
        }
    }
}

#[inline]
fn grid_to_local(grid: usize, positive: bool) -> i32 {
    if positive {
        grid as i32
    } else {
        (SECTION_SIZE - 1 - grid) as i32
    }
}

/// What one face looks like, and which pass draws it. `None` where there is no face.
fn face_attr(
    catalog: &[BlockInfo],
    scratch: &Scratch,
    local: [i32; 3],
    face: usize,
) -> Option<(u8, u32)> {
    let here = scratch.states[border_index(local[0], local[1], local[2])];
    let info = &catalog[here as usize];
    let cube = info.cube.as_ref()?;
    let normal = face_normal(face);
    let front = [
        local[0] + normal[0],
        local[1] + normal[1],
        local[2] + normal[2],
    ];
    let front_index = border_index(front[0], front[1], front[2]);
    if scratch.occludes[front_index] {
        return None;
    }
    if info.self_culls && scratch.states[front_index] == here {
        return None;
    }

    let cube = cube[face];
    let axes = FACE_AXES[face];
    let mut u_step = [0i32; 3];
    u_step[axes[2] as usize] = if axes[3] == 1 { 1 } else { -1 };
    let mut v_step = [0i32; 3];
    v_step[axes[4] as usize] = if axes[5] == 1 { 1 } else { -1 };

    let mut ao = 0u32;
    for corner in 0..4 {
        let du = if CORNER_UV[corner][0] > 0.5 { 1 } else { -1 };
        let dv = if CORNER_UV[corner][1] > 0.5 { 1 } else { -1 };
        let side_u = occludes_at(scratch, front, u_step, du, [0; 3], 0);
        let side_v = occludes_at(scratch, front, v_step, dv, [0; 3], 0);
        let diagonal = occludes_at(scratch, front, u_step, du, v_step, dv);
        let value = if side_u && side_v {
            0
        } else {
            3 - (side_u as u32 + side_v as u32 + diagonal as u32)
        };
        ao |= value << (corner * 2);
    }

    let raw = scratch.light[front_index] as u32;
    let mut words = [0u32; 1];
    FACE_LAYER.set(&mut words, cube.sprite.layer as u64);
    FACE_ARRAY.set(&mut words, cube.sprite.array as u64);
    if cube.tinted {
        FACE_TINT.set(&mut words, info.tint_kind as u64 + 1);
    }
    // Kept apart rather than folded into one value: only the sky half follows the time of day, and
    // the block's own emission is block light like any other.
    FACE_BLOCK_LIGHT.set(&mut words, (raw >> 4).max(info.emission as u32) as u64);
    FACE_SKY_LIGHT.set(&mut words, (raw & 0xf) as u64);
    FACE_AO.set(&mut words, ao as u64);
    Some((cube.pass, words[0]))
}

#[inline]
fn occludes_at(
    scratch: &Scratch,
    base: [i32; 3],
    a: [i32; 3],
    sa: i32,
    b: [i32; 3],
    sb: i32,
) -> bool {
    let x = base[0] + a[0] * sa + b[0] * sb;
    let y = base[1] + a[1] * sa + b[1] * sb;
    let z = base[2] + a[2] * sa + b[2] * sb;
    let limit = -1..=SECTION_SIZE as i32;
    if !limit.contains(&x) || !limit.contains(&y) || !limit.contains(&z) {
        return false;
    }
    scratch.occludes[border_index(x, y, z)]
}

/// What a slice cell carries while it is being merged: the pass in the low bits and, above it, how
/// far below its own plane a fluid face sits. The two together are the whole of what decides
/// whether two faces may become one quad, so they are compared as one number.
const PASS_KEY_BITS: u8 = 2;
const PASS_KEY: u8 = (1 << PASS_KEY_BITS) - 1;
/// The top bit of the key, set on a run of fluid faces. Two faces that agree on everything else
/// still must not merge across it: one of them is held off the block boundary and the other is not.
const FLUID_KEY: u8 = 1 << 7;

fn merge_slice(scratch: &mut Scratch, face: usize, n: usize, local_section: u32) {
    for gv in 0..SECTION_SIZE {
        let mut gu = 0usize;
        while gu < SECTION_SIZE {
            let slot = gv * SECTION_SIZE + gu;
            // Both "already merged into a quad" and "carries no face at all", because the pass of
            // a cell without a face is left over from whichever slice last wrote it.
            if scratch.used[slot] {
                gu += 1;
                continue;
            }
            let key = scratch.passes[slot];
            let mut w = 1;
            while gu + w < SECTION_SIZE {
                let probe = slot + w;
                if scratch.used[probe] || scratch.passes[probe] != key {
                    break;
                }
                w += 1;
            }
            let mut h = 1;
            'grow: while gv + h < SECTION_SIZE {
                for i in 0..w {
                    let probe = (gv + h) * SECTION_SIZE + gu + i;
                    if scratch.used[probe] || scratch.passes[probe] != key {
                        break 'grow;
                    }
                }
                h += 1;
            }
            // Row by row along the quad's own axes, which is the order the fragment shader indexes
            // them in once it has floored its quad coordinate.
            let base = scratch.section_faces.len() as u32;
            for dv in 0..h {
                for du in 0..w {
                    let cell = (gv + dv) * SECTION_SIZE + gu + du;
                    scratch.used[cell] = true;
                    let attr = scratch.attrs[cell];
                    scratch.section_faces.push(attr);
                }
            }
            scratch.simple_by_pass[(key & PASS_KEY) as usize].push(pack_quad(
                face,
                n,
                gu,
                gv,
                w,
                h,
                local_section,
                base,
                (key & !FLUID_KEY) >> PASS_KEY_BITS,
                key & FLUID_KEY != 0,
            ));
            gu += w;
        }
    }
}

/// The world-space anchor of a greedy quad: the corner at grid `(gu, gv)` on the face plane.
#[inline]
fn quad_anchor(face: usize, n: usize, gu: usize, gv: usize) -> [i32; 3] {
    let axes = FACE_AXES[face];
    let mut local = [0i32; 3];
    local[axes[0] as usize] = n as i32 + if axes[1] == 1 { 1 } else { 0 };
    local[axes[2] as usize] = if axes[3] == 1 {
        gu as i32
    } else {
        SECTION_SIZE as i32 - gu as i32
    };
    local[axes[4] as usize] = if axes[5] == 1 {
        gv as i32
    } else {
        SECTION_SIZE as i32 - gv as i32
    };
    local
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn pack_quad(
    face: usize,
    n: usize,
    gu: usize,
    gv: usize,
    w: usize,
    h: usize,
    local_section: u32,
    face_base: u32,
    drop: u8,
    fluid: bool,
) -> [u32; QUAD_WORDS] {
    let anchor = quad_anchor(face, n, gu, gv);
    let mut words = [0u32; QUAD_WORDS];
    QUAD_DROP.set(&mut words, drop as u64);
    QUAD_FLUID.set(&mut words, fluid as u64);
    QUAD_X.set(&mut words, anchor[0] as u64);
    QUAD_Y.set(&mut words, anchor[1] as u64);
    QUAD_Z.set(&mut words, anchor[2] as u64);
    QUAD_FACE.set(&mut words, face as u64);
    QUAD_W.set(&mut words, w as u64 - 1);
    QUAD_H.set(&mut words, h as u64 - 1);
    QUAD_SECTION.set(&mut words, local_section as u64);
    QUAD_FACE_BASE.set(&mut words, face_base as u64);
    words
}


// ---------------------------------------------------------------------------------------------
// Fluids.
//
// A fluid is not a block model. Water and lava ship models holding nothing but a particle texture,
// and the shape the client draws is worked out from the levels of the eight blocks around each
// cell: the surface stops eight ninths of the way up a source block, sags towards whatever is
// shallower beside it, and turns its texture to face the direction the fluid runs.
//
// Almost none of a real world's fluid needs that. Everything under the surface is a full cube, and
// a calm sea is a flat lid one drop below the top of its block; both merge the way any cube face
// merges and cost two words a quad. Only the cells that actually slope or flow — the shoreline,
// the waterfalls — become their own model quads. That split is the whole point: an ocean surface
// that would be sixty-five thousand model quads a render region comes out as a few hundred.

/// Which of the two fluid grids a cell belongs to.
#[inline]
fn fluid_kind(cell: u8) -> usize {
    (cell & FLUID_LAVA != 0) as usize
}

/// Whether a cell holds the given kind of fluid at all.
#[inline]
fn same_fluid(cell: u8, kind: usize) -> bool {
    cell != 0 && fluid_kind(cell) == kind
}

/// Where a cell inside the section sits in the arrays indexed by section rather than by border.
#[inline]
fn section_cell(x: i32, y: i32, z: i32) -> usize {
    (y as usize * SECTION_SIZE + z as usize) * SECTION_SIZE + x as usize
}

/// A fluid cell whose surface is neither flat nor still, and so is a shape of its own.
struct Sloped {
    cell: [i32; 3],
    /// The height of each corner in blocks, in vanilla's order: north-west, south-west, south-east,
    /// north-east.
    corners: [f32; 4],
    /// Which way the surface runs, as the angle its texture is turned by, or `None` where the fluid
    /// is still and takes the surface texture untouched.
    flow: Option<f32>,
}

/// How far below the top of its block a surface of this many ninths sits, in [`MODEL_STEPS`].
///
/// Rounded onto the model path's own grid rather than kept in ninths: the same surface is drawn by
/// the merged path on one side of a shoreline and by the model path on the other, and two grids
/// would leave a step between them along every coast in the world.
#[inline]
fn drop_steps(ninths: u8) -> u8 {
    (f32::from(FLUID_FULL - ninths) * MODEL_STEPS / f32::from(FLUID_FULL)).round() as u8
}

/// Works out the surface of every fluid cell in the section, and sorts each into the merged path or
/// the model path.
fn fluid_surfaces(scratch: &mut Scratch) {
    scratch.sloped.clear();
    // Most sections of a world hold no fluid at all, and everything below this line is wasted on
    // them: the sweep, the two grids it clears, and the six face groups the merger would then walk
    // over an empty mask.
    if scratch.fluid_kinds == 0 {
        return;
    }
    *scratch.flat_columns = [[[0; COLUMNS]; 3]; FLUID_KINDS];
    scratch.flat_drop.fill(NOT_FLAT);

    for z in 0..SECTION_SIZE as i32 {
        for x in 0..SECTION_SIZE as i32 {
            // Which cells of this column hold fluid, a whole column at a time. A section with a
            // spring in the corner of it costs the two hundred and fifty six reads of the columns
            // rather than the four thousand of the cells.
            let column = column_index(1, x, 0, z);
            let mut rows = (scratch.fluid_columns[0][1][column]
                | scratch.fluid_columns[1][1][column])
                & IN_SECTION;
            while rows != 0 {
                let y = rows.trailing_zeros() as i32 - 1;
                rows &= rows - 1;
                let cell = scratch.fluid[border_index(x, y, z)];
                let kind = fluid_kind(cell);
                let above = scratch.fluid[border_index(x, y + 1, z)];
                // A fluid with its own kind above it fills its block outright, which is the whole
                // of a submerged ocean and never needs a corner sampled or a flow worked out.
                let own = if same_fluid(above, kind) {
                    FLUID_FULL
                } else {
                    cell & FLUID_AMOUNT
                };
                if own == FLUID_FULL {
                    mark_flat(scratch, [x, y, z], kind, 0);
                    continue;
                }

                let corners = [
                    corner_height(scratch, [x, y, z], kind, own, -1, -1),
                    corner_height(scratch, [x, y, z], kind, own, -1, 1),
                    corner_height(scratch, [x, y, z], kind, own, 1, 1),
                    corner_height(scratch, [x, y, z], kind, own, 1, -1),
                ];
                let flow = flow_angle(scratch, [x, y, z], kind, own);
                // One height across the whole surface and nothing moving is a box, whatever the
                // level of the block says: a sea of source blocks is every cell of this kind.
                let level = exact_ninths(corners[0]).filter(|_| {
                    flow.is_none() && corners.iter().all(|&c| exact_ninths(c) == exact_ninths(corners[0]))
                });
                match level {
                    Some(ninths) => mark_flat(scratch, [x, y, z], kind, drop_steps(ninths)),
                    None => scratch.sloped.push(Sloped {
                        cell: [x, y, z],
                        corners: corners
                            .map(|(num, den)| num as f32 / (den as f32 * f32::from(FLUID_FULL))),
                        flow,
                    }),
                }
            }
        }
    }
}

/// Records a cell as a flat box the merged path can take.
fn mark_flat(scratch: &mut Scratch, [x, y, z]: [i32; 3], kind: usize, drop: u8) {
    scratch.flat_drop[section_cell(x, y, z)] = drop;
    let along = [x, y, z];
    for axis in 0..3 {
        scratch.flat_columns[kind][axis][column_index(axis, x, y, z)] |= 1 << (along[axis] + 1);
    }
}

/// A corner height as an exact fraction of a ninth, when it is one. A corner averaged out of
/// samples that disagree lands between two ninths and can only be drawn by the model path, so the
/// question the merged path asks is whether the average came out whole.
#[inline]
fn exact_ninths((num, den): (i32, i32)) -> Option<u8> {
    (num % den == 0).then(|| (num / den) as u8)
}

/// Vanilla's fluid height sample, in ninths. `None` is its discarded sample: a solid block beside a
/// fluid must not drag the surface down, only an open one may.
fn height_sample(scratch: &Scratch, x: i32, y: i32, z: i32, kind: usize) -> Option<u8> {
    let index = border_index(x, y, z);
    let cell = scratch.fluid[index];
    if same_fluid(cell, kind) {
        let above = scratch.fluid[border_index(x, y + 1, z)];
        return Some(if same_fluid(above, kind) {
            FLUID_FULL
        } else {
            cell & FLUID_AMOUNT
        });
    }
    (!scratch.occludes[index]).then_some(0)
}

/// One corner of a fluid surface, as `(numerator, denominator)` over ninths.
///
/// Kept as a fraction rather than a float because the merged path turns on whether the answer is a
/// whole ninth, and a float would have to guess at how near is near enough. The two neighbours and
/// the diagonal between them are the same three cells the neighbouring block samples for the corner
/// it shares, so the two agree exactly and no seam opens between them.
fn corner_height(
    scratch: &Scratch,
    [x, y, z]: [i32; 3],
    kind: usize,
    own: u8,
    dx: i32,
    dz: i32,
) -> (i32, i32) {
    let full = (i32::from(FLUID_FULL), 1);
    let a = height_sample(scratch, x + dx, y, z, kind);
    let b = height_sample(scratch, x, y, z + dz, kind);
    if a == Some(FLUID_FULL) || b == Some(FLUID_FULL) {
        return full;
    }
    let mut num = 0;
    let mut den = 0;
    if a.unwrap_or(0) > 0 || b.unwrap_or(0) > 0 {
        let diagonal = height_sample(scratch, x + dx, y, z + dz, kind);
        if diagonal == Some(FLUID_FULL) {
            return full;
        }
        weigh(&mut num, &mut den, diagonal);
    }
    weigh(&mut num, &mut den, Some(own));
    weigh(&mut num, &mut den, a);
    weigh(&mut num, &mut den, b);
    (num, den)
}

/// Vanilla counts a nearly full sample ten times over, which is what holds a deep body of water
/// flat right up to its edge instead of letting every shallow neighbour sag it.
#[inline]
fn weigh(num: &mut i32, den: &mut i32, sample: Option<u8>) {
    let Some(height) = sample else { return };
    let weight = if height >= 8 { 10 } else { 1 };
    *num += i32::from(height) * weight;
    *den += weight;
}

/// The angle vanilla turns a moving surface's texture by, or `None` where the fluid is still.
///
/// This is `FlowingFluid.getFlow` with everything the renderer never reads left out: the vertical
/// term a falling fluid adds does not change the direction the surface runs in, and the length of
/// the vector is thrown away by the angle.
fn flow_angle(scratch: &Scratch, [x, y, z]: [i32; 3], kind: usize, own: u8) -> Option<f32> {
    let ninth = 1.0 / f32::from(FLUID_FULL);
    let own_height = f32::from(own) * ninth;
    let mut flow_x = 0.0f32;
    let mut flow_z = 0.0f32;
    for face in 2..6usize {
        let normal = face_normal(face);
        let side = [x + normal[0], y + normal[1], z + normal[2]];
        let index = border_index(side[0], side[1], side[2]);
        let cell = scratch.fluid[index];
        // Another fluid altogether does not move this one.
        if cell != 0 && fluid_kind(cell) != kind {
            continue;
        }
        let distance = if cell != 0 {
            own_height - f32::from(cell & FLUID_AMOUNT) * ninth
        } else if scratch.occludes[index] {
            0.0
        } else {
            // Nothing beside it, so the fluid runs towards whatever is under the gap: a full drop
            // further than any height difference on the level could ever be.
            let below = scratch.fluid[border_index(side[0], side[1] - 1, side[2])];
            if same_fluid(below, kind) {
                own_height - (f32::from(below & FLUID_AMOUNT) * ninth - 8.0 * ninth)
            } else {
                0.0
            }
        };
        flow_x += normal[0] as f32 * distance;
        flow_z += normal[2] as f32 * distance;
    }
    (flow_x != 0.0 || flow_z != 0.0)
        .then(|| flow_z.atan2(flow_x) - std::f32::consts::FRAC_PI_2)
}

/// Merges the flat fluid cells of the section, one kind and one face group at a time, exactly the
/// way [`greedy`] merges cube faces.
fn fluid_greedy(
    catalog: &[BlockInfo],
    scratch: &mut Scratch,
    local_section: u32,
    region_index: u32,
    partial: &mut Partial,
) {
    for kind in 0..FLUID_KINDS {
        if scratch.fluid_kinds >> kind & 1 == 0 {
            continue;
        }
        for face in 0..6usize {
            for pass in 0..Pass::COUNT {
                scratch.simple_by_pass[pass].clear();
            }
            let axes = FACE_AXES[face];
            let n_axis = axes[0] as usize;
            let u_axis = axes[2] as usize;
            let u_positive = axes[3] == 1;
            let v_axis = axes[4] as usize;
            let v_positive = axes[5] == 1;
            let n_positive = axes[1] == 1;

            // A flat cell has a face wherever the cell one step along the axis is not the same
            // fluid. Whether something opaque hides it is left to the per-cell pass, because a
            // shortened surface sits below the block above it and is hidden by nothing.
            let mut occupied = 0u32;
            for gv in 0..SECTION_SIZE {
                for gu in 0..SECTION_SIZE {
                    let mut local = [0i32; 3];
                    local[u_axis] = grid_to_local(gu, u_positive);
                    local[v_axis] = grid_to_local(gv, v_positive);
                    let column = column_index(n_axis, local[0], local[1], local[2]);
                    let flat = scratch.flat_columns[kind][n_axis][column];
                    let mine = scratch.fluid_columns[kind][n_axis][column];
                    let front = if n_positive { mine >> 1 } else { mine << 1 };
                    let visible = flat & !front;
                    scratch.faces[gv * SECTION_SIZE + gu] = visible;
                    occupied |= visible;
                }
            }

            for n in 0..SECTION_SIZE {
                let bit = 1u32 << (n + 1);
                if occupied & bit == 0 {
                    continue;
                }
                let mut any = false;
                for gv in 0..SECTION_SIZE {
                    for gu in 0..SECTION_SIZE {
                        let slot = gv * SECTION_SIZE + gu;
                        if scratch.faces[slot] & bit == 0 {
                            scratch.used[slot] = true;
                            continue;
                        }
                        let mut local = [0i32; 3];
                        local[n_axis] = n as i32;
                        local[u_axis] = grid_to_local(gu, u_positive);
                        local[v_axis] = grid_to_local(gv, v_positive);
                        match fluid_face_attr(catalog, scratch, local, face) {
                            Some((key, attr)) => {
                                scratch.used[slot] = false;
                                scratch.passes[slot] = key;
                                scratch.attrs[slot] = attr;
                                any = true;
                            }
                            None => scratch.used[slot] = true,
                        }
                    }
                }
                if any {
                    merge_slice(scratch, face, n, local_section);
                }
            }

            for pass in 0..Pass::COUNT {
                let quads = std::mem::take(&mut scratch.simple_by_pass[pass]);
                if !quads.is_empty() {
                    partial.groups.push((
                        (pass * 2) as u32,
                        region_index,
                        Group {
                            quad_base: partial.simple.len() as u32,
                            quad_count: quads.len() as u32,
                            section: local_section | GROUP_FACE.pack(fluid_group(kind, face)) as u32,
                            quad_prefix: 0,
                        },
                    ));
                    partial.simple.extend_from_slice(&quads);
                }
                scratch.simple_by_pass[pass] = quads;
            }
        }
    }
}

/// Which culling group a run of fluid faces belongs to.
///
/// Water is drawn from both sides, because a camera under the surface has to see it, so it is filed
/// under the group that points nowhere and is never dropped for facing away. Lava is opaque and
/// hides its own far side, so it keeps the group its faces really point at and is culled with
/// everything else.
#[inline]
fn fluid_group(kind: usize, face: usize) -> u64 {
    if kind == fluid_kind(FLUID_LAVA) {
        face as u64
    } else {
        FACE_NONE as u64
    }
}

/// What one flat fluid face looks like, and the key it merges on. `None` where there is no face.
fn fluid_face_attr(
    catalog: &[BlockInfo],
    scratch: &Scratch,
    local: [i32; 3],
    face: usize,
) -> Option<(u8, u32)> {
    let here = border_index(local[0], local[1], local[2]);
    let state = scratch.states[here] as usize;
    let fluid = catalog[state].fluid?;
    let drop = scratch.flat_drop[section_cell(local[0], local[1], local[2])];

    // Vanilla asks two blocks whether a fluid face is hidden: the one the fluid is in, which is
    // what stops a waterlogged stair from showing water through its own back, and the one it
    // faces. Both answer per side, so a slab closes one face of six rather than all or none.
    if scratch.cover[here] >> face & 1 == 1 {
        return None;
    }
    let normal = face_normal(face);
    let front = [
        local[0] + normal[0],
        local[1] + normal[1],
        local[2] + normal[2],
    ];
    let front_index = border_index(front[0], front[1], front[2]);
    let front_cover = scratch.cover[front_index];
    // A shortened surface stops below the block above it, so whatever is up there hides nothing.
    if front_cover >> (face ^ 1) & 1 == 1 && !(face == 1 && drop > 0) {
        return None;
    }

    // Vanilla lights a fluid face from the brighter of its own cell and the one above it, so that
    // a sea floor lit from the surface does not read as a cave. The underside takes the cell below
    // in place of its own.
    let vertical = if face == 0 {
        border_index(local[0], local[1] - 1, local[2])
    } else {
        border_index(local[0], local[1] + 1, local[2])
    };
    let mine = scratch.light[here] as u32;
    let other = scratch.light[vertical] as u32;
    let block_light = (mine >> 4).max(other >> 4).max(catalog[state].emission as u32);
    let sky_light = (mine & 0xf).max(other & 0xf);

    // The surface texture lies flat on the top and the bottom; every vertical face takes the
    // flowing one, which is what makes a waterfall read as falling — except where it meets
    // something you can see through, which takes the overlay instead.
    let sprite = if face < 2 {
        fluid.still
    } else {
        side_sprite(fluid, front_cover)
    };

    let mut words = [0u32; 1];
    FACE_LAYER.set(&mut words, sprite.layer as u64);
    FACE_ARRAY.set(&mut words, sprite.array as u64);
    if !fluid.lava {
        FACE_TINT.set(&mut words, TintKind::Water as u64 + 1);
    }
    FACE_BLOCK_LIGHT.set(&mut words, block_light as u64);
    FACE_SKY_LIGHT.set(&mut words, sky_light as u64);
    // Vanilla's fluid renderer takes no ambient occlusion at all, so every corner reads fully open.
    FACE_AO.set(&mut words, FACE_AO.max());
    if face >= 2 {
        FACE_FLUID.set(&mut words, 1);
    }

    let pass = if fluid.lava { Pass::Solid } else { Pass::Translucent };
    Some((pass as u8 | drop << PASS_KEY_BITS | FLUID_KEY, words[0]))
}

/// The texture a vertical fluid face takes, given what it faces. Water behind glass or leaves
/// shows a flat sheet rather than the flowing texture, which is the whole of what the overlay is
/// for; lava ships no overlay and keeps its own.
#[inline]
fn side_sprite(fluid: Fluid, front_cover: u8) -> SpriteRef {
    match fluid.overlay {
        Some(overlay) if front_cover & COVER_SEE_THROUGH != 0 => overlay,
        _ => fluid.flow,
    }
}

/// Emits the fluid cells that are not flat boxes as model quads, following vanilla's own fluid
/// renderer corner for corner.
fn fluid_models(catalog: &[BlockInfo], scratch: &mut Scratch, local_section: u32) {
    let sloped = std::mem::take(&mut scratch.sloped);
    for cell in &sloped {
        let [x, y, z] = cell.cell;
        let here = border_index(x, y, z);
        let state = scratch.states[here] as usize;
        let Some(fluid) = catalog[state].fluid else {
            continue;
        };
        let kind = fluid_kind(scratch.fluid[here]);
        let emission = catalog[state].emission as u32;
        let pass = if fluid.lava { Pass::Solid } else { Pass::Translucent } as usize;
        let tint = if fluid.lava { 0 } else { TintKind::Water as u32 + 1 };
        // Never one of the axis groups: a sloped surface points along none of them squarely, and
        // water has to survive the culling pass from below in any case.
        let out = &mut scratch.complex_by_pass[pass][FACE_GROUPS - 1];

        let mut corners = cell.corners;
        let facing = |face: usize| {
            let normal = face_normal(face);
            border_index(x + normal[0], y + normal[1], z + normal[2])
        };
        let open = |face: usize| {
            let index = facing(face);
            scratch.cover[here] >> face & 1 == 0
                && !same_fluid(scratch.fluid[index], kind)
                && scratch.cover[index] >> (face ^ 1) & 1 == 0
        };
        let lowest = corners.iter().copied().fold(f32::INFINITY, f32::min);
        let above = facing(1);
        // The block above hides the surface only when the surface reaches it.
        let up = scratch.cover[here] >> 1 & 1 == 0
            && !same_fluid(scratch.fluid[above], kind)
            && (scratch.cover[above] & 1 == 0 || lowest < 1.0);
        let down = open(0);
        let bottom = if down { FLUID_INSET } else { 0.0 };

        let light = |a: usize, b: usize| {
            let (a, b) = (scratch.light[a] as u32, scratch.light[b] as u32);
            (
                (a >> 4).max(b >> 4).max(emission),
                (a & 0xf).max(b & 0xf),
            )
        };
        let side_light = light(here, above);

        let (fx, fy, fz) = (x as f32, y as f32, z as f32);
        if up {
            // Pulled a hair down so that a surface level with the block above it does not fight the
            // block's own underside. The sides are drawn off the lowered corners as well, or their
            // top edges would stand above the surface they are meant to close.
            corners = corners.map(|corner| corner - FLUID_INSET);
            let [nw, sw, se, ne] = corners;
            let (sprite, uvs) = match cell.flow {
                None => (fluid.still, [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]),
                Some(angle) => {
                    let (sin, cos) = (angle.sin() * 0.25, angle.cos() * 0.25);
                    (
                        fluid.flow,
                        [
                            [0.5 - cos - sin, 0.5 - cos + sin],
                            [0.5 - cos + sin, 0.5 + cos + sin],
                            [0.5 + cos + sin, 0.5 + cos - sin],
                            [0.5 + cos - sin, 0.5 - cos - sin],
                        ],
                    )
                }
            };
            push_fluid_quad(
                out,
                [
                    [fx, fy + nw, fz],
                    [fx, fy + sw, fz + 1.0],
                    [fx + 1.0, fy + se, fz + 1.0],
                    [fx + 1.0, fy + ne, fz],
                ],
                uvs,
                UP_SHADE,
                light(here, above),
                tint,
                sprite,
                local_section,
            );
        }

        if down {
            push_fluid_quad(
                out,
                [
                    [fx, fy + bottom, fz],
                    [fx + 1.0, fy + bottom, fz],
                    [fx + 1.0, fy + bottom, fz + 1.0],
                    [fx, fy + bottom, fz + 1.0],
                ],
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                DOWN_SHADE,
                light(border_index(x, y - 1, z), here),
                tint,
                fluid.still,
                local_section,
            );
        }

        // The four sides, each a trapezoid running from the two corner heights of its edge down to
        // the floor of the block. The flowing texture is sampled from its top-left quarter, which
        // is what shows it at twice the size of a block face.
        for face in 2..6usize {
            if !open(face) {
                continue;
            }
            let [north_west, south_west, south_east, north_east] = corners;
            let (c0, c1, x0, z0, x1, z1) = match face {
                2 => (north_west, north_east, fx, fz + FLUID_INSET, fx + 1.0, fz + FLUID_INSET),
                3 => (
                    south_east,
                    south_west,
                    fx + 1.0,
                    fz + 1.0 - FLUID_INSET,
                    fx,
                    fz + 1.0 - FLUID_INSET,
                ),
                4 => (south_west, north_west, fx + FLUID_INSET, fz + 1.0, fx + FLUID_INSET, fz),
                _ => (
                    north_east,
                    south_east,
                    fx + 1.0 - FLUID_INSET,
                    fz,
                    fx + 1.0 - FLUID_INSET,
                    fz + 1.0,
                ),
            };
            push_fluid_quad(
                out,
                [
                    [x0, fy + c0, z0],
                    [x1, fy + c1, z1],
                    [x1, fy + bottom, z1],
                    [x0, fy + bottom, z0],
                ],
                [
                    [0.0, (1.0 - c0) * 0.5],
                    [0.5, (1.0 - c1) * 0.5],
                    [0.5, 0.5],
                    [0.0, 0.5],
                ],
                if face < 4 { NORTH_SOUTH_SHADE } else { EAST_WEST_SHADE },
                side_light,
                tint,
                side_sprite(fluid, scratch.cover[facing(face)]),
                local_section,
            );
        }
    }
    scratch.sloped = sloped;
}

/// The directional shade buckets vanilla's fluid renderer uses, in the order [`shade_bucket`]
/// numbers them.
const DOWN_SHADE: u32 = 0;
const EAST_WEST_SHADE: u32 = 1;
const NORTH_SOUTH_SHADE: u32 = 2;
const UP_SHADE: u32 = 3;

#[allow(clippy::too_many_arguments)]
fn push_fluid_quad(
    out: &mut Vec<u32>,
    positions: [[f32; 3]; 4],
    uvs: [[f32; 2]; 4],
    shade: u32,
    (block_light, sky_light): (u32, u32),
    tint: u32,
    sprite: SpriteRef,
    local_section: u32,
) {
    let scale = MODEL_U.max() as f32;
    for corner in 0..4 {
        let mut words = [0u32; 3];
        MODEL_X.set(&mut words, fixed(positions[corner][0]) as u64);
        MODEL_Y.set(&mut words, fixed(positions[corner][1]) as u64);
        MODEL_Z.set(&mut words, fixed(positions[corner][2]) as u64);
        MODEL_U.set(&mut words, (uvs[corner][0].clamp(0.0, 1.0) * scale) as u64);
        MODEL_V.set(&mut words, (uvs[corner][1].clamp(0.0, 1.0) * scale) as u64);
        MODEL_TINT.set(&mut words, tint as u64);
        MODEL_BLOCK_LIGHT.set(&mut words, block_light as u64);
        MODEL_SKY_LIGHT.set(&mut words, sky_light as u64);
        MODEL_SHADE.set(&mut words, shade as u64);
        MODEL_SECTION.set(&mut words, local_section as u64);
        MODEL_ARRAY.set(&mut words, sprite.array as u64);
        MODEL_LAYER.set(&mut words, sprite.layer as u64);
        out.extend_from_slice(&words);
    }
}

/// Baked model geometry, written out verbatim with the block's own light and the vanilla
/// directional shade already folded into each vertex.
///
/// ponytail: smooth ambient occlusion is not recomputed per instance here, so stairs and slabs get
/// flat shading. Complex blocks are a low single-digit share of a region's quads; wire in the same
/// corner sampling the greedy path uses if builds ever dominate the view.
fn complex(
    catalog: &[BlockInfo],
    scratch: &mut Scratch,
    local_section: u32,
    region_index: u32,
    partial: &mut Partial,
) {
    for pass in 0..Pass::COUNT {
        for group in &mut scratch.complex_by_pass[pass] {
            group.clear();
        }
    }

    for y in 0..SECTION_SIZE {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                let here = border_index(x as i32, y as i32, z as i32);
                let info = &catalog[scratch.states[here] as usize];
                if info.quads.is_empty() {
                    continue;
                }

                for quad in &info.quads {
                    // A quad with a cullface sits on the block boundary, so its light comes from
                    // the neighbour it faces. Reading the block's own cell instead would render
                    // every face of a solid block — a grass block's side overlay, a spawner's
                    // inner faces — at the zero light that a solid cell always stores.
                    let mut sample = here;
                    if let Some(cull) = quad.cull {
                        let normal = face_normal(cull as usize);
                        let front = border_index(
                            x as i32 + normal[0],
                            y as i32 + normal[1],
                            z as i32 + normal[2],
                        );
                        if scratch.occludes[front] {
                            continue;
                        }
                        sample = front;
                    }
                    let raw = scratch.light[sample] as u32;
                    let block_light = (raw >> 4).max(info.emission as u32);
                    let sky_light = raw & 0xf;
                    let tint = if quad.tinted {
                        info.tint_kind as u32 + 1
                    } else {
                        0
                    };
                    let group = quad.face.map_or(FACE_GROUPS - 1, |group| group as usize);
                    let out = &mut scratch.complex_by_pass[quad.pass as usize][group];
                    for corner in 0..4 {
                        let p = quad.positions[corner];
                        let scale = MODEL_U.max() as f32;
                        let u = (quad.uvs[corner][0].clamp(0.0, 1.0) * scale) as u32;
                        let v = (quad.uvs[corner][1].clamp(0.0, 1.0) * scale) as u32;
                        let mut words = [0u32; 3];
                        MODEL_X.set(&mut words, fixed(p.x + x as f32) as u64);
                        MODEL_Y.set(&mut words, fixed(p.y + y as f32) as u64);
                        MODEL_Z.set(&mut words, fixed(p.z + z as f32) as u64);
                        MODEL_U.set(&mut words, u as u64);
                        MODEL_V.set(&mut words, v as u64);
                        MODEL_TINT.set(&mut words, tint as u64);
                        MODEL_BLOCK_LIGHT.set(&mut words, block_light as u64);
                        MODEL_SKY_LIGHT.set(&mut words, sky_light as u64);
                        MODEL_SHADE.set(&mut words, shade_bucket(quad.shade[corner]) as u64);
                        MODEL_SECTION.set(&mut words, local_section as u64);
                        MODEL_ARRAY.set(&mut words, quad.sprite.array as u64);
                        MODEL_LAYER.set(&mut words, quad.sprite.layer as u64);
                        out.extend_from_slice(&words);
                    }
                }
            }
        }
    }

    fluid_models(catalog, scratch, local_section);

    for pass in 0..Pass::COUNT {
        for group in 0..FACE_GROUPS {
            let verts = std::mem::take(&mut scratch.complex_by_pass[pass][group]);
            if !verts.is_empty() {
                let face = if group == FACE_GROUPS - 1 {
                    FACE_NONE
                } else {
                    group as u32
                };
                partial.groups.push((
                    (pass * 2 + 1) as u32,
                    region_index,
                    Group {
                        quad_base: (partial.complex.len() / 3 / 4) as u32,
                        quad_count: (verts.len() / 3 / 4) as u32,
                        section: local_section | GROUP_FACE.pack(face as u64) as u32,
                        quad_prefix: 0,
                    },
                ));
                partial.complex.extend_from_slice(&verts);
            }
            scratch.complex_by_pass[pass][group] = verts;
        }
    }
}

/// Vanilla only ever emits four distinct directional shade values, one per face orientation, so two
/// bits carry the whole range and the shader expands them from a constant table.
#[inline]
fn shade_bucket(shade: u8) -> u32 {
    match shade {
        0..=140 => 0,   // down, 0.5
        141..=175 => 1, // west / east, 0.6
        176..=225 => 2, // north / south, 0.8
        _ => 3,         // up, 1.0
    }
}

/// The shade each bucket stands for, in bucket order. The shader holds its own copy of this table
/// because it is what expands the two packed bits, and a test reads that copy back.
#[cfg(test)]
const BUCKET_SHADES: [f32; 4] = [0.5, 0.6, 0.8, 1.0];

/// Model positions live in fixed point, biased by the overhang, so a face that pokes outside its
/// own block still packs into a non-negative number.
#[inline]
fn fixed(value: f32) -> u32 {
    ((value + MODEL_OVERHANG) * MODEL_STEPS)
        .round()
        .clamp(0.0, MODEL_X.max() as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::{
        BORDER_VOLUME, BUCKET_SHADES, FACE_ARRAY, FACE_LAYER, FLUID_FULL, QUAD_DROP,
        QUAD_FACE, QUAD_FACE_BASE, QUAD_H, QUAD_SECTION, QUAD_W, QUAD_X, QUAD_Y, QUAD_Z,
        SECTION_FACE_TABLE, Scratch, border_index, connectivity, drop_steps, fixed,
        mesh_render_region, pack_quad, quad_anchor, shade_bucket,
    };
    use crate::atlas::SpriteRef;
    use crate::anvil::{Palette, SECTION_SIZE, SECTION_VOLUME, World, one_section_region};
    use crate::bake::Dir;
    use crate::blocks::{BlockInfo, CubeFace, Fluid, ModelQuad, Pass};
    use crate::pack::{MODEL_OVERHANG, MODEL_STEPS, RegionGrid, pack_section};
    use crate::blocks::{CORNER_UV, FACE_AXES, cube_corner};
    use bevy::math::Vec3;


    /// The merged path drops a fluid quad below its own plane and the model path places the same
    /// surface outright. The two have to land on the same number: a bay is drawn by one path and
    /// its shoreline by the other, and a grid apart would show a step along every coast there is.
    #[test]
    fn the_two_fluid_paths_put_a_surface_in_the_same_place() {
        for ninths in 0..=FLUID_FULL {
            let merged = MODEL_STEPS - f32::from(drop_steps(ninths));
            let model =
                fixed(f32::from(ninths) / f32::from(FLUID_FULL)) as f32 - fixed(0.0) as f32;
            assert_eq!(merged, model, "{ninths} ninths of a block lands in two places");
        }
    }

    /// Water that is level with itself is a flat lid, and the whole reason fluids are split across
    /// the two paths is that the lid merges. A sea a section wide comes out as one quad, sunk the
    /// ninth of a block vanilla stops a source short of the top by, and only the rim — where the
    /// surface really does slope away into open air — reaches the model path.
    #[test]
    fn a_flat_sea_merges_into_one_quad_a_ninth_below_the_block_top() {
        let mut palette = Palette::new();
        let mut world = World::new([0, 0], [1, 1]);
        world.insert(&mut palette, [0, 0], one_section_region("minecraft:water"));
        let id = palette
            .states
            .iter()
            .position(|state| state.name == "minecraft:water")
            .unwrap();
        let mut blocks: Vec<BlockInfo> =
            (0..palette.states.len()).map(|_| BlockInfo::default()).collect();
        blocks[id].fluid = Some(Fluid {
            lava: false,
            amount: 8,
            still: SpriteRef::default(),
            flow: SpriteRef::default(),
            overlay: None,
        });

        let grid = RegionGrid::covering(world.sections);
        let mut scratch = Scratch::new();
        let mut surfaces = Vec::new();
        let mut models = 0;
        for region in 0..grid.len() {
            let batch = mesh_render_region(&world, &blocks, grid, region, &mut scratch);
            models += batch.model_quads();
            for quad in &batch.simple {
                if QUAD_FACE.read(quad) == 1 {
                    surfaces.push((
                        QUAD_W.read(quad) + 1,
                        QUAD_H.read(quad) + 1,
                        QUAD_DROP.read(quad),
                    ));
                }
            }
        }

        // Every cell but the outermost ring is level with all four of its neighbours, and the one
        // layer with open air above it is the only one whose upward face is not hidden by more of
        // itself.
        let interior = SECTION_SIZE as u64 - 2;
        assert_eq!(
            surfaces,
            [(interior, interior, drop_steps(8) as u64)],
            "a flat sea did not come out as a single sunk quad"
        );
        assert!(models > 0, "the rim of the sea slopes away and has to be modelled");
    }


    /// The block a fluid is waterlogged into hides the fluid faces its own geometry covers. Without
    /// that, a waterlogged stair or slab is drawn with a sheet of water laid over its solid sides,
    /// and every one of those sheets is a blended quad nothing can ever see.
    #[test]
    fn a_waterlogged_block_hides_the_fluid_faces_it_covers() {
        let upward = |sturdy: u8| {
            let mut palette = Palette::new();
            let mut world = World::new([0, 0], [1, 1]);
            world.insert(&mut palette, [0, 0], one_section_region("minecraft:oak_slab"));
            let id = palette
                .states
                .iter()
                .position(|state| state.name == "minecraft:oak_slab")
                .unwrap();
            let mut blocks: Vec<BlockInfo> =
                (0..palette.states.len()).map(|_| BlockInfo::default()).collect();
            blocks[id].sturdy = sturdy;
            blocks[id].fluid = Some(Fluid {
                lava: false,
                amount: 8,
                still: SpriteRef::default(),
                flow: SpriteRef::default(),
                overlay: None,
            });

            let grid = RegionGrid::covering(world.sections);
            let mut scratch = Scratch::new();
            (0..grid.len())
                .map(|region| mesh_render_region(&world, &blocks, grid, region, &mut scratch))
                .flat_map(|batch| batch.simple.into_iter())
                .filter(|quad| QUAD_FACE.read(quad) == Dir::Up as u64)
                .count()
        };

        assert_eq!(upward(0), 1, "open water shows its surface");
        assert_eq!(
            upward(1 << Dir::Up as u8),
            0,
            "a block that closes its own top must hide the water under it"
        );
    }


    /// Water shows vanilla's overlay texture where it meets something you can see through, so that
    /// a window into a flooded room reads as a sheet of water rather than a waterfall running down
    /// the glass. Every other vertical face keeps the flowing texture.
    #[test]
    fn water_against_glass_takes_the_overlay_texture() {
        const WATER: usize = 0;
        const GLASS: usize = 1;
        let mut palette = Palette::new();
        let mut world = World::new([0, 0], [1, 1]);
        world.insert(
            &mut palette,
            [0, 0],
            crate::anvil::one_section_region_of(
                &["minecraft:water", "minecraft:glass"],
                |x, _, _| if x < 8 { WATER } else { GLASS },
            ),
        );
        let id = |name: &str| {
            palette.states.iter().position(|state| state.name == name).unwrap()
        };

        let sprite = |layer: u16| SpriteRef { array: 0, layer };
        let mut blocks: Vec<BlockInfo> =
            (0..palette.states.len()).map(|_| BlockInfo::default()).collect();
        blocks[id("minecraft:water")].fluid = Some(Fluid {
            lava: false,
            amount: 8,
            still: sprite(2),
            flow: sprite(3),
            overlay: Some(sprite(4)),
        });
        // A cube that hides nothing behind it, which is the whole of what marks a block as one the
        // overlay is shown against.
        blocks[id("minecraft:glass")].cube = Some([CubeFace {
            sprite: sprite(1),
            pass: Pass::Translucent as u8,
            tinted: false,
        }; 6]);

        let grid = RegionGrid::covering(world.sections);
        let mut scratch = Scratch::new();
        let mut sprites = [0usize; 5];
        for region in 0..grid.len() {
            let batch = mesh_render_region(&world, &blocks, grid, region, &mut scratch);
            for attr in batch.faces.iter().skip(SECTION_FACE_TABLE) {
                sprites[FACE_LAYER.get(*attr as u64) as usize] += 1;
            }
        }

        // One face for every cell of the wall the water stands against, bar the topmost layer:
        // there the surface slopes away towards the open air over the glass and is drawn as its own
        // shape instead of merged.
        assert_eq!(
            sprites[4],
            (SECTION_SIZE - 1) * SECTION_SIZE,
            "the water did not meet the glass with its overlay"
        );
        assert!(sprites[3] > 0, "water away from the glass keeps the flowing texture");
    }

    fn pair(entry: usize, exit: usize) -> u64 {
        1 << (entry * 6 + exit)
    }

    /// Every mesher stage has to name a block in the world's own numbering. Each region file
    /// interns its ids from zero, so a stage reaching past the world's remap into a file's palette
    /// names a different block for every file but the first: not a crash, not a hole, just the
    /// wrong model on every block of three regions out of four.
    #[test]
    fn the_model_mesher_names_blocks_in_the_worlds_numbering() {
        let mut palette = Palette::new();
        let mut world = World::new([0, 0], [1, 1]);
        world.insert(&mut palette, [0, 0], one_section_region("minecraft:test_block"));
        let id = palette
            .states
            .iter()
            .position(|state| state.name == "minecraft:test_block")
            .unwrap();
        assert_ne!(id, 0, "the fixture only bites while the two numberings disagree");

        // Only the world's id for the block carries geometry. Whatever the file called it does not.
        let mut blocks: Vec<BlockInfo> =
            (0..palette.states.len()).map(|_| BlockInfo::default()).collect();
        blocks[id].quads = vec![ModelQuad {
            positions: [Vec3::ZERO; 4],
            uvs: [[0.0; 2]; 4],
            cull: None,
            face: None,
            sprite: SpriteRef::default(),
            pass: Pass::Solid,
            shade: [255; 4],
            tinted: false,
        }];

        let grid = RegionGrid::covering(world.sections);
        let mut scratch = Scratch::new();
        let quads: usize = (0..grid.len())
            .map(|region| mesh_render_region(&world, &blocks, grid, region, &mut scratch).model_quads())
            .sum();
        assert_eq!(
            quads,
            SECTION_VOLUME,
            "one model quad per block of the one section the fixture fills"
        );
    }

    fn solid_section() -> Box<[bool; BORDER_VOLUME]> {
        Box::new([true; BORDER_VOLUME])
    }

    /// Pins the flood fill and the Down=0 / Up=1 numbering in one go.
    #[test]
    fn a_vertical_shaft_connects_only_down_and_up() {
        let mut occludes = solid_section();
        for y in 0..16 {
            occludes[border_index(8, y, 8)] = false;
        }
        assert_eq!(
            connectivity(&mut occludes),
            pair(0, 0) | pair(0, 1) | pair(1, 0) | pair(1, 1)
        );
    }

    /// A naive implementation — "which faces does any air at all touch" — would report every pair
    /// between Down/Up and West/East here. It is the one real way to get this algorithm wrong.
    #[test]
    fn two_disjoint_shafts_do_not_join() {
        let mut occludes = solid_section();
        for y in 0..16 {
            occludes[border_index(2, y, 2)] = false;
        }
        for x in 0..16 {
            occludes[border_index(x, 12, 12)] = false;
        }
        let mask = connectivity(&mut occludes);
        assert_eq!(mask & (pair(0, 4) | pair(4, 0)), 0, "shafts must not merge");
        assert_eq!(
            mask,
            pair(0, 0)
                | pair(0, 1)
                | pair(1, 0)
                | pair(1, 1)
                | pair(4, 4)
                | pair(4, 5)
                | pair(5, 4)
                | pair(5, 5)
        );
    }

    /// Every quad names as many face attributes as it covers blocks, and the runs of a batch tile
    /// its face buffer with no gap and no overlap once each is resolved through the section table
    /// the way the shader resolves it. A base off by one run is not a hole in the frame: it draws
    /// the whole world out of its neighbours' sprites and light.
    #[test]
    fn the_face_runs_of_a_batch_tile_it_exactly() {
        let mut palette = Palette::new();
        let mut world = World::new([0, 0], [1, 1]);
        world.insert(&mut palette, [0, 0], one_section_region("minecraft:test_block"));
        let id = palette
            .states
            .iter()
            .position(|state| state.name == "minecraft:test_block")
            .unwrap();
        let mut blocks: Vec<BlockInfo> =
            (0..palette.states.len()).map(|_| BlockInfo::default()).collect();
        blocks[id].cube = Some(
            [CubeFace {
                sprite: SpriteRef { array: 1, layer: 7 },
                pass: Pass::Solid as u8,
                tinted: false,
            }; 6],
        );
        blocks[id].occludes = true;

        let grid = RegionGrid::covering(world.sections);
        let mut scratch = Scratch::new();
        let mut quads = 0usize;
        for region in 0..grid.len() {
            let batch = mesh_render_region(&world, &blocks, grid, region, &mut scratch);
            quads += batch.simple.len();
            if batch.simple.is_empty() {
                assert!(batch.faces.is_empty(), "a table describing nothing was written");
                continue;
            }
            let mut runs: Vec<(u64, u64)> = batch
                .simple
                .iter()
                .map(|quad| {
                    let section = QUAD_SECTION.read(quad) as usize;
                    (
                        batch.faces[section] as u64 + QUAD_FACE_BASE.read(quad),
                        (QUAD_W.read(quad) + 1) * (QUAD_H.read(quad) + 1),
                    )
                })
                .collect();
            runs.sort();
            let mut at = SECTION_FACE_TABLE as u64;
            for (base, len) in runs {
                assert_eq!(base, at, "a face run does not start where the last one ended");
                at += len;
            }
            assert_eq!(at as usize, batch.faces.len(), "the runs leave the buffer uncovered");
            for attr in &batch.faces[SECTION_FACE_TABLE..] {
                assert_eq!(FACE_LAYER.get(*attr as u64), 7, "sprite layer");
                assert_eq!(FACE_ARRAY.get(*attr as u64), 1, "sprite array");
            }
        }
        assert_eq!(
            quads, 6,
            "a lone solid section is six merged faces, one per side"
        );
    }

    #[test]
    fn fixed_point_covers_the_overhang_a_model_can_have() {
        assert_eq!(fixed(-2.0), 0);
        assert_eq!(fixed(0.0), 64);
        assert_eq!(fixed(1.0), 96);
        assert_eq!(fixed(0.5), 80);
        // The far corner a model may reach: the whole section plus the overhang on both sides.
        let far = SECTION_SIZE as f32 + MODEL_OVERHANG;
        assert_eq!(
            fixed(far),
            ((far + MODEL_OVERHANG) * MODEL_STEPS) as u32,
            "the overhang past the far face has to survive the encoding"
        );
    }

    /// A one-block quad must land on exactly the same corners the model baker produces for that
    /// face, otherwise greedy geometry and baked geometry would not line up at a seam.
    #[test]
    fn a_single_block_greedy_quad_matches_the_baked_cube() {
        for face in 0..6usize {
            let axes = FACE_AXES[face];
            // Place the block at local (0,0,0) of a section rooted at the origin.
            let gu = if axes[3] == 1 { 0 } else { 15 };
            let gv = if axes[5] == 1 { 0 } else { 15 };
            let anchor = quad_anchor(face, 0, gu, gv);

            for corner in 0..4 {
                let cu = CORNER_UV[corner][0];
                let cv = CORNER_UV[corner][1];
                let mut world = [0f32; 3];
                world[axes[0] as usize] = anchor[axes[0] as usize] as f32;
                world[axes[2] as usize] = anchor[axes[2] as usize] as f32
                    + if axes[3] == 1 { cu } else { -cu };
                world[axes[4] as usize] = anchor[axes[4] as usize] as f32
                    + if axes[5] == 1 { cv } else { -cv };
                let expected = cube_corner(
                    crate::bake::Dir::ALL[face],
                    corner,
                );
                assert!(
                    Vec3::from(world).distance(expected) < 1e-5,
                    "face {face} corner {corner}: greedy {world:?} vs baked {expected:?}"
                );
            }
        }
    }

    /// The mesher buckets a baked shade byte and the shader turns the bucket back into a number,
    /// and nothing links the two ends. Getting the order wrong there does not fail to compile and
    /// does not move a single vertex: it just lights a wall as though it faced another way.
    #[test]
    fn a_bucketed_shade_decodes_to_the_shade_it_stood_for() {
        // What the model baker emits for a face in open air, one byte per vanilla shade.
        for (byte, shade) in [(127u8, 0.5), (153, 0.6), (204, 0.8), (255, 1.0)] {
            let bucket = shade_bucket(byte) as usize;
            assert_eq!(
                BUCKET_SHADES[bucket], shade,
                "a face baked at {shade} buckets to {bucket}"
            );
        }

        let source = include_str!("terrain.wgsl");
        let arms: Vec<f32> = source
            .lines()
            .filter_map(|line| {
                let (_, rest) = line.trim().split_once("shade = ")?;
                rest.split_once(';')?.0.parse().ok()
            })
            .collect();
        assert_eq!(
            arms.len(),
            BUCKET_SHADES.len() + 1,
            "the shader's shade table is not shaped the way this test reads it"
        );
        // The first is the variable's initialiser, then one arm per bucket, the last being the
        // default the shader uses for the highest bucket.
        assert_eq!(arms[1..], BUCKET_SHADES, "the shader expands the buckets differently");
    }

    #[test]
    fn a_packed_quad_round_trips_every_field() {
        let section = pack_section(9, 5, 12);
        let words = pack_quad(3, 9, 3, 7, 12, 16, section, 24_575, 0, false);
        let anchor = quad_anchor(3, 9, 3, 7);
        assert_eq!(QUAD_X.read(&words), anchor[0] as u64, "x");
        assert_eq!(QUAD_Y.read(&words), anchor[1] as u64, "y");
        assert_eq!(QUAD_Z.read(&words), anchor[2] as u64, "z");
        assert_eq!(QUAD_SECTION.read(&words), section as u64, "section");
        assert_eq!(QUAD_FACE.read(&words), 3, "face");
        assert_eq!(QUAD_W.read(&words) + 1, 12, "w");
        assert_eq!(QUAD_H.read(&words) + 1, 16, "h");
        assert_eq!(QUAD_FACE_BASE.read(&words), 24_575, "face base");
    }
}
