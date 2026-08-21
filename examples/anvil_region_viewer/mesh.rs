use crate::anvil::{SECTION_SIZE, SECTION_VOLUME, World};
use crate::atlas::SpriteRef;
use crate::blocks::{BlockInfo, CORNER_UV, FACE_AXES, Fluid, Pass, TintKind};
use crate::pack::{
    FACE_AO, FACE_ARRAY, FACE_BLOCK_LIGHT, FACE_FLUID, FACE_LAYER, FACE_SKY_LIGHT, FACE_TINT,
    FACE_NONE, GROUP_FACE, MODEL_ARRAY, MODEL_BLOCK_LIGHT, MODEL_LAYER, MODEL_OVERHANG, MODEL_SECTION,
    MODEL_SHADE, MODEL_SKY_LIGHT, MODEL_STEPS, MODEL_TINT, MODEL_U, MODEL_V, MODEL_X, MODEL_Y,
    FLUID_INSET, MODEL_Z, QUAD_DROP, QUAD_FACE, QUAD_FACE_BASE, QUAD_FLUID, QUAD_H,
    QUAD_SECTION, QUAD_W, QUAD_WORDS,
    QUAD_X, QUAD_Y, QUAD_Z, RENDER_REGION_X, RENDER_REGION_Y, RENDER_REGION_Z, RegionGrid,
    SECTION_FACE_TABLE,
};

pub const STREAMS: usize = Pass::COUNT * 2;

pub const STREAM_NAMES: [&str; STREAMS] = [
    "solid greedy",
    "solid model",
    "cutout greedy",
    "cutout model",
    "translucent greedy",
    "translucent model",
];

const FACE_GROUPS: usize = FACE_NONE as usize + 1;

const BORDER: usize = SECTION_SIZE + 2;
const BORDER_VOLUME: usize = BORDER * BORDER * BORDER;

const FLUID_AMOUNT: u8 = 0x0f;
const FLUID_LAVA: u8 = 0x10;

const FLUID_FULL: u8 = 9;

const NOT_FLAT: u8 = 0xff;

const COVER_SEE_THROUGH: u8 = 1 << 6;

const IN_SECTION: u32 = ((1 << SECTION_SIZE) - 1) << 1;

const FLUID_KINDS: usize = 2;

const COLUMNS: usize = BORDER * BORDER;

pub const CONNECT_ALL: u64 = (1 << 36) - 1;

#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Group {
    pub quad_base: u32,
    pub quad_count: u32,
    pub section: u32,
    pub quad_prefix: u32,
}

pub struct Batch {
    pub region: usize,
    pub simple: Vec<[u32; QUAD_WORDS]>,
    pub faces: Vec<u32>,
    pub complex: Vec<u32>,
    pub groups: Vec<Group>,
    pub spans: [StreamSpan; STREAMS],
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
    states: Box<[u16; BORDER_VOLUME]>,
    occludes: Box<[bool; BORDER_VOLUME]>,
    light: Box<[u8; BORDER_VOLUME]>,
    cube_columns: Box<[[u32; COLUMNS]; 3]>,
    occlude_columns: Box<[[u32; COLUMNS]; 3]>,
    fluid: Box<[u8; BORDER_VOLUME]>,
    cover: Box<[u8; BORDER_VOLUME]>,
    fluid_kinds: u8,
    fluid_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    flat_columns: Box<[[[u32; COLUMNS]; 3]; FLUID_KINDS]>,
    flat_drop: Box<[u8; SECTION_VOLUME]>,
    sloped: Vec<Sloped>,
    faces: Box<[u32; SECTION_SIZE * SECTION_SIZE]>,
    passes: Box<[u8; SECTION_SIZE * SECTION_SIZE]>,
    attrs: Box<[u32; SECTION_SIZE * SECTION_SIZE]>,
    used: Box<[bool; SECTION_SIZE * SECTION_SIZE]>,
    simple_by_pass: [Vec<[u32; QUAD_WORDS]>; Pass::COUNT],
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

struct Partial {
    simple: Vec<[u32; QUAD_WORDS]>,
    faces: Vec<u32>,
    section_faces: Vec<(u32, u32)>,
    complex: Vec<u32>,
    groups: Vec<(u32, u32, Group)>,
    connectivity: Vec<(u32, u64)>,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct Draw {
    pub stream: u32,
    pub region: u32,
    pub origin: [i32; 3],
    pub cave_base: u32,
    pub face_base: u32,
    pub first_group: u32,
    pub group_count: u32,
    pub quad_count: u32,
}

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

    if partial.section_faces.is_empty() {
        partial.faces.clear();
    }
    for (section, start) in &partial.section_faces {
        partial.faces[*section as usize] = *start;
    }

    let mut groups = Vec::with_capacity(partial.groups.len());
    let mut spans = [StreamSpan::default(); STREAMS];
    for stream in 0..STREAMS {
        let first = groups.len();
        let mut quads = 0u32;
        for &(from, _, group) in &partial.groups {
            if from as usize == stream {
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

#[inline]
fn inside(coordinate: i32) -> bool {
    (0..SECTION_SIZE as i32).contains(&coordinate)
}

#[inline]
fn border_index(x: i32, y: i32, z: i32) -> usize {
    ((y + 1) as usize) * BORDER * BORDER + ((z + 1) as usize) * BORDER + (x + 1) as usize
}

#[inline]
fn column_index(axis: usize, x: i32, y: i32, z: i32) -> usize {
    let (p, q) = match axis {
        0 => (y, z),
        1 => (x, z),
        _ => (x, y),
    };
    (p + 1) as usize * BORDER + (q + 1) as usize
}

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

const PASS_KEY_BITS: u8 = 2;
const PASS_KEY: u8 = (1 << PASS_KEY_BITS) - 1;
const FLUID_KEY: u8 = 1 << 7;

fn merge_slice(scratch: &mut Scratch, face: usize, n: usize, local_section: u32) {
    for gv in 0..SECTION_SIZE {
        let mut gu = 0usize;
        while gu < SECTION_SIZE {
            let slot = gv * SECTION_SIZE + gu;
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



#[inline]
fn fluid_kind(cell: u8) -> usize {
    (cell & FLUID_LAVA != 0) as usize
}

#[inline]
fn same_fluid(cell: u8, kind: usize) -> bool {
    cell != 0 && fluid_kind(cell) == kind
}

#[inline]
fn section_cell(x: i32, y: i32, z: i32) -> usize {
    (y as usize * SECTION_SIZE + z as usize) * SECTION_SIZE + x as usize
}

struct Sloped {
    cell: [i32; 3],
    corners: [f32; 4],
    flow: Option<f32>,
}

#[inline]
fn drop_steps(ninths: u8) -> u8 {
    (f32::from(FLUID_FULL - ninths) * MODEL_STEPS / f32::from(FLUID_FULL)).round() as u8
}

fn fluid_surfaces(scratch: &mut Scratch) {
    scratch.sloped.clear();
    if scratch.fluid_kinds == 0 {
        return;
    }
    *scratch.flat_columns = [[[0; COLUMNS]; 3]; FLUID_KINDS];
    scratch.flat_drop.fill(NOT_FLAT);

    for z in 0..SECTION_SIZE as i32 {
        for x in 0..SECTION_SIZE as i32 {
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

fn mark_flat(scratch: &mut Scratch, [x, y, z]: [i32; 3], kind: usize, drop: u8) {
    scratch.flat_drop[section_cell(x, y, z)] = drop;
    let along = [x, y, z];
    for axis in 0..3 {
        scratch.flat_columns[kind][axis][column_index(axis, x, y, z)] |= 1 << (along[axis] + 1);
    }
}

#[inline]
fn exact_ninths((num, den): (i32, i32)) -> Option<u8> {
    (num % den == 0).then(|| (num / den) as u8)
}

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

#[inline]
fn weigh(num: &mut i32, den: &mut i32, sample: Option<u8>) {
    let Some(height) = sample else { return };
    let weight = if height >= 8 { 10 } else { 1 };
    *num += i32::from(height) * weight;
    *den += weight;
}

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
        if cell != 0 && fluid_kind(cell) != kind {
            continue;
        }
        let distance = if cell != 0 {
            own_height - f32::from(cell & FLUID_AMOUNT) * ninth
        } else if scratch.occludes[index] {
            0.0
        } else {
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

#[inline]
fn fluid_group(kind: usize, face: usize) -> u64 {
    if kind == fluid_kind(FLUID_LAVA) {
        face as u64
    } else {
        FACE_NONE as u64
    }
}

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
    if front_cover >> (face ^ 1) & 1 == 1 && !(face == 1 && drop > 0) {
        return None;
    }

    let vertical = if face == 0 {
        border_index(local[0], local[1] - 1, local[2])
    } else {
        border_index(local[0], local[1] + 1, local[2])
    };
    let mine = scratch.light[here] as u32;
    let other = scratch.light[vertical] as u32;
    let block_light = (mine >> 4).max(other >> 4).max(catalog[state].emission as u32);
    let sky_light = (mine & 0xf).max(other & 0xf);

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
    FACE_AO.set(&mut words, FACE_AO.max());
    if face >= 2 {
        FACE_FLUID.set(&mut words, 1);
    }

    let pass = if fluid.lava { Pass::Solid } else { Pass::Translucent };
    Some((pass as u8 | drop << PASS_KEY_BITS | FLUID_KEY, words[0]))
}

#[inline]
fn side_sprite(fluid: Fluid, front_cover: u8) -> SpriteRef {
    match fluid.overlay {
        Some(overlay) if front_cover & COVER_SEE_THROUGH != 0 => overlay,
        _ => fluid.flow,
    }
}

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

#[inline]
fn shade_bucket(shade: u8) -> u32 {
    match shade {
        0..=140 => 0,
        141..=175 => 1,
        176..=225 => 2,
        _ => 3,
    }
}

#[cfg(test)]
const BUCKET_SHADES: [f32; 4] = [0.5, 0.6, 0.8, 1.0];

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


    #[test]
    fn the_two_fluid_paths_put_a_surface_in_the_same_place() {
        for ninths in 0..=FLUID_FULL {
            let merged = MODEL_STEPS - f32::from(drop_steps(ninths));
            let model =
                fixed(f32::from(ninths) / f32::from(FLUID_FULL)) as f32 - fixed(0.0) as f32;
            assert_eq!(merged, model, "{ninths} ninths of a block lands in two places");
        }
    }

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

        let interior = SECTION_SIZE as u64 - 2;
        assert_eq!(
            surfaces,
            [(interior, interior, drop_steps(8) as u64)],
            "a flat sea did not come out as a single sunk quad"
        );
        assert!(models > 0, "the rim of the sea slopes away and has to be modelled");
    }


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
        let far = SECTION_SIZE as f32 + MODEL_OVERHANG;
        assert_eq!(
            fixed(far),
            ((far + MODEL_OVERHANG) * MODEL_STEPS) as u32,
            "the overhang past the far face has to survive the encoding"
        );
    }

    #[test]
    fn a_single_block_greedy_quad_matches_the_baked_cube() {
        for face in 0..6usize {
            let axes = FACE_AXES[face];
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

    #[test]
    fn a_bucketed_shade_decodes_to_the_shade_it_stood_for() {
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
