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

use crate::anvil::{SECTION_SIZE, World};
use crate::blocks::{BlockInfo, CORNER_UV, FACE_AXES, Pass};
use crate::pack::{
    FACE_AO, FACE_ARRAY, FACE_BLOCK_LIGHT, FACE_LAYER, FACE_SKY_LIGHT, FACE_TINT, GROUP_FACE,
    MODEL_ARRAY, MODEL_BLOCK_LIGHT, MODEL_LAYER, MODEL_OVERHANG, MODEL_SECTION, MODEL_SHADE,
    MODEL_SKY_LIGHT, MODEL_STEPS, MODEL_TINT, MODEL_U, MODEL_V, MODEL_X, MODEL_Y, MODEL_Z,
    QUAD_FACE, QUAD_FACE_BASE, QUAD_H, QUAD_SECTION, QUAD_W, QUAD_WORDS, QUAD_X, QUAD_Y, QUAD_Z,
    RENDER_REGION_X, RENDER_REGION_Y, RENDER_REGION_Z, RegionGrid, SECTION_FACE_TABLE,
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
    pub _pad: u32,
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
    for y in -1..=SECTION_SIZE as i32 {
        for z in -1..=SECTION_SIZE as i32 {
            for x in -1..=SECTION_SIZE as i32 {
                let index = border_index(x, y, z);
                let state = world.block(base[0] + x, base[1] + y, base[2] + z);
                let info = &catalog[state as usize];
                scratch.states[index] = state;
                scratch.occludes[index] = info.occludes;
                scratch.light[index] = world.light(base[0] + x, base[1] + y, base[2] + z);
                if !info.occludes && info.cube.is_none() {
                    continue;
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
                }
            }
        }
    }

    let (region_index, local_section) = grid.split(sx, sy, sz);
    let region_index = region_index as u32;
    let faces_at = partial.faces.len() as u32;
    greedy(catalog, scratch, local_section, region_index, partial);
    if !scratch.section_faces.is_empty() {
        partial.faces.extend_from_slice(&scratch.section_faces);
        partial.section_faces.push((local_section, faces_at));
    }
    complex(catalog, scratch, local_section, region_index, partial);
    partial
        .connectivity
        .push((local_section, connectivity(&mut scratch.occludes)));
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
fn face_normal(face: usize) -> [i32; 3] {
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
                        _pad: 0,
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
            let pass = scratch.passes[slot] as usize;
            let mut w = 1;
            while gu + w < SECTION_SIZE {
                let probe = slot + w;
                if scratch.used[probe] || scratch.passes[probe] as usize != pass {
                    break;
                }
                w += 1;
            }
            let mut h = 1;
            'grow: while gv + h < SECTION_SIZE {
                for i in 0..w {
                    let probe = (gv + h) * SECTION_SIZE + gu + i;
                    if scratch.used[probe] || scratch.passes[probe] as usize != pass {
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
            scratch.simple_by_pass[pass]
                .push(pack_quad(face, n, gu, gv, w, h, local_section, base));
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
) -> [u32; QUAD_WORDS] {
    let anchor = quad_anchor(face, n, gu, gv);
    let mut words = [0u32; QUAD_WORDS];
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
                        _pad: 0,
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
        BORDER_VOLUME, BUCKET_SHADES, FACE_ARRAY, FACE_LAYER, QUAD_FACE, QUAD_FACE_BASE, QUAD_H,
        QUAD_SECTION, QUAD_W, QUAD_X, QUAD_Y, QUAD_Z, SECTION_FACE_TABLE, Scratch, border_index,
        connectivity, fixed, mesh_render_region, pack_quad, quad_anchor, shade_bucket,
    };
    use crate::atlas::SpriteRef;
    use crate::anvil::{Palette, SECTION_SIZE, SECTION_VOLUME, World, one_section_region};
    use crate::blocks::{BlockInfo, CubeFace, ModelQuad, Pass};
    use crate::pack::{MODEL_OVERHANG, MODEL_STEPS, RegionGrid, pack_section};
    use crate::blocks::{CORNER_UV, FACE_AXES, cube_corner};
    use bevy::math::Vec3;

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
        let words = pack_quad(3, 9, 3, 7, 12, 16, section, 24_575);
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
