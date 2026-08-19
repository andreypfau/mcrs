//! Turns the region into packed GPU geometry.
//!
//! Two meshers run over every section. Full cubes go through greedy merging into 8-byte quads that
//! the vertex shader unpacks with no vertex buffer and no index buffer at all; everything else
//! emits its baked model quads as 12-byte vertices. Both write contiguous runs tagged with the
//! section and face they came from, so the culling compute shader can drop whole runs — including
//! the three-or-more face groups pointing away from the camera — before any vertex work happens.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::anvil::{REGION_CHUNKS, Region, SECTION_SIZE, Section};
use crate::blocks::{CORNER_UV, Catalog, FACE_AXES, Pass};
use crate::atlas::SpriteRef;
use crate::pack::{
    GROUP_FACE, MODEL_ARRAY, MODEL_LAYER, MODEL_LIGHT, MODEL_OVERHANG, MODEL_SECTION, MODEL_SHADE,
    MODEL_STEPS, MODEL_TINT, MODEL_U, MODEL_V, MODEL_X, MODEL_Y, MODEL_Z, QUAD_AO, QUAD_ARRAY,
    QUAD_FACE, QUAD_FLIP, QUAD_H, QUAD_LAYER, QUAD_LIGHT, QUAD_SECTION, QUAD_TINT, QUAD_W, QUAD_X,
    QUAD_Y, QUAD_Z, RegionGrid, SECTIONS_PER_RENDER_REGION,
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

/// Face group 7 means "no cullface": drawn whenever the section is visible.
const FACE_NONE: u32 = 7;

/// The six face groups model geometry can be sorted into, plus one for the quads that face no axis
/// squarely and so are never backfacing as a run.
const FACE_GROUPS: usize = 7;

const BORDER: usize = SECTION_SIZE + 2;
const BORDER_VOLUME: usize = BORDER * BORDER * BORDER;

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

pub struct RegionMesh {
    /// Packed greedy quads, two `u32` each.
    pub simple: Vec<u64>,
    /// Packed model vertices, three `u32` each, four vertices per quad.
    pub complex: Vec<u32>,
    /// Culling groups, ordered so each draw owns one contiguous span.
    pub groups: Vec<Group>,
    /// What each stream holds in total, for the load report. The draws below are what the frame
    /// actually issues.
    pub streams: [StreamSpan; STREAMS],
    /// One entry per stream and render region that holds anything, in draw order.
    pub draws: Vec<Draw>,
    /// The render regions the world was cut into.
    pub grid: RegionGrid,
    /// Per-section face connectivity, indexed the way the sight-line walk numbers sections. Slots
    /// the mesher never touched stay [`CONNECT_ALL`]: a section missing from the file is air, and
    /// defaulting it to "closed" would kill a sight-line walk on its very first step through open
    /// sky.
    pub connectivity: Vec<u64>,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct StreamSpan {
    pub group_count: u32,
    pub quad_count: u32,
}

impl RegionMesh {
    pub fn greedy_quads(&self) -> usize {
        self.simple.len()
    }

    pub fn model_quads(&self) -> usize {
        self.complex.len() / 3 / 4
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Default)]
struct Key {
    /// `None` when there is no face here. Two faces only merge when they name the same sprite, so
    /// the array is part of the key as much as the layer is.
    sprite: Option<SpriteRef>,
    tint: u32,
    light: u32,
    ao: u32,
    flip: bool,
    pass: u32,
}

struct Scratch {
    /// Section volume plus a one-block border, so neighbour lookups never re-enter [`Region`].
    states: Box<[u16; BORDER_VOLUME]>,
    occludes: Box<[bool; BORDER_VOLUME]>,
    light: Box<[u8; BORDER_VOLUME]>,
    keys: Box<[Key; SECTION_SIZE * SECTION_SIZE]>,
    used: Box<[bool; SECTION_SIZE * SECTION_SIZE]>,
    /// Geometry for the face group being built, split by pass so each run stays contiguous.
    simple_by_pass: [Vec<u64>; Pass::COUNT],
    complex_by_pass: [[Vec<u32>; FACE_GROUPS]; Pass::COUNT],
}

impl Scratch {
    fn new() -> Self {
        Self {
            states: Box::new([0; BORDER_VOLUME]),
            occludes: Box::new([false; BORDER_VOLUME]),
            light: Box::new([0; BORDER_VOLUME]),
            keys: Box::new([Key::default(); SECTION_SIZE * SECTION_SIZE]),
            used: Box::new([false; SECTION_SIZE * SECTION_SIZE]),
            simple_by_pass: Default::default(),
            complex_by_pass: Default::default(),
        }
    }
}

/// One worker's slice of the finished geometry. Concatenated into a single arena afterwards.
struct Partial {
    simple: Vec<u64>,
    complex: Vec<u32>,
    /// `(stream, render region, group)` with `group.quad_base` relative to this worker's own arena.
    groups: Vec<(u32, u32, Group)>,
    /// `(section slot, mask)`, keyed the way the sight-line walk indexes sections.
    connectivity: Vec<(u32, u64)>,
}

/// One `draw_indirect`: the groups of a single stream that lie inside a single render region.
/// Draw order is stream order, so the list runs stream by stream.
#[derive(Copy, Clone, Default, Debug)]
pub struct Draw {
    pub stream: u32,
    /// The region's corner in blocks. Coordinates in a quad are relative to their section, so this
    /// is what puts the geometry back where it belongs.
    pub origin: [i32; 3],
    /// Where this region's sections start in the sight-line bitset.
    pub cave_base: u32,
    pub first_group: u32,
    pub group_count: u32,
    /// Every quad these groups hold, which is the most culling can let through.
    pub quad_count: u32,
}

pub fn build(region: &Region, catalog: &Catalog) -> RegionMesh {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let next = AtomicUsize::new(0);
    let partials: Mutex<Vec<Partial>> = Mutex::new(Vec::new());
    let columns = REGION_CHUNKS * REGION_CHUNKS;
    let grid = RegionGrid::covering([REGION_CHUNKS, region.sections_y, REGION_CHUNKS]);

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut scratch = Scratch::new();
                let mut partial = Partial {
                    simple: Vec::new(),
                    complex: Vec::new(),
                    groups: Vec::new(),
                    connectivity: Vec::new(),
                };
                loop {
                    let column = next.fetch_add(1, Ordering::Relaxed);
                    if column >= columns {
                        break;
                    }
                    let cx = column % REGION_CHUNKS;
                    let cz = column / REGION_CHUNKS;
                    for sy in 0..region.sections_y {
                        if region.section(cx, sy, cz).is_some() {
                            mesh_section(
                                region, catalog, grid, cx, sy, cz, &mut scratch, &mut partial,
                            );
                        }
                    }
                }
                partials.lock().unwrap().push(partial);
            });
        }
    });

    assemble(partials.into_inner().unwrap(), grid, region.min_section_y)
}

fn assemble(partials: Vec<Partial>, grid: RegionGrid, min_section_y: i32) -> RegionMesh {
    let mut simple = Vec::new();
    let mut complex = Vec::new();
    // One bucket per stream and render region. A bucket is one `draw_indirect`, so the groups in
    // it have to end up contiguous and every quad in it has to share the same region corner.
    let mut buckets: Vec<Vec<Group>> = (0..STREAMS * grid.len()).map(|_| Vec::new()).collect();
    let mut simple_base = 0u32;
    let mut complex_base = 0u32;

    for partial in &partials {
        for &(stream, region, group) in &partial.groups {
            let base = if stream % 2 == 0 {
                simple_base
            } else {
                complex_base
            };
            buckets[stream as usize * grid.len() + region as usize].push(Group {
                quad_base: group.quad_base + base,
                ..group
            });
        }
        simple_base += partial.simple.len() as u32;
        complex_base += (partial.complex.len() / 3 / 4) as u32;
        simple.extend_from_slice(&partial.simple);
        complex.extend_from_slice(&partial.complex);
    }

    let mut groups = Vec::new();
    let mut streams = [StreamSpan::default(); STREAMS];
    let mut draws = Vec::new();
    let size = SECTION_SIZE as i32;
    for stream in 0..STREAMS {
        let mut stream_quads = 0u32;
        let mut stream_groups = 0u32;
        for region in 0..grid.len() {
            let bucket = &mut buckets[stream * grid.len() + region];
            if bucket.is_empty() {
                continue;
            }
            let quad_count = bucket.iter().map(|group| group.quad_count).sum();
            let [sx, sy, sz] = grid.corner(region);
            draws.push(Draw {
                stream: stream as u32,
                origin: [
                    sx as i32 * size,
                    (sy as i32 + min_section_y) * size,
                    sz as i32 * size,
                ],
                cave_base: (region * SECTIONS_PER_RENDER_REGION) as u32,
                first_group: groups.len() as u32,
                group_count: bucket.len() as u32,
                quad_count,
            });
            stream_quads += quad_count;
            stream_groups += bucket.len() as u32;
            groups.append(bucket);
        }
        streams[stream] = StreamSpan {
            group_count: stream_groups,
            quad_count: stream_quads,
        };
    }

    let mut connectivity = vec![CONNECT_ALL; grid.slots()];
    for partial in &partials {
        for &(slot, mask) in &partial.connectivity {
            connectivity[slot as usize] = mask;
        }
    }

    RegionMesh {
        simple,
        complex,
        groups,
        streams,
        draws,
        grid,
        connectivity,
    }
}

#[allow(clippy::too_many_arguments)]
fn mesh_section(
    region: &Region,
    catalog: &Catalog,
    grid: RegionGrid,
    cx: usize,
    sy: usize,
    cz: usize,
    scratch: &mut Scratch,
    partial: &mut Partial,
) {
    let base = [
        (cx * SECTION_SIZE) as i32,
        (sy as i32 + region.min_section_y) * SECTION_SIZE as i32,
        (cz * SECTION_SIZE) as i32,
    ];

    for y in -1..=SECTION_SIZE as i32 {
        for z in -1..=SECTION_SIZE as i32 {
            for x in -1..=SECTION_SIZE as i32 {
                let index = border_index(x, y, z);
                let state = region.block(base[0] + x, base[1] + y, base[2] + z);
                scratch.states[index] = state;
                scratch.occludes[index] = catalog.blocks[state as usize].occludes;
                scratch.light[index] = region.light(base[0] + x, base[1] + y, base[2] + z);
            }
        }
    }

    let section = region
        .section(cx, sy, cz)
        .expect("caller checked the section exists");
    let (region_index, local_section) = grid.split(cx, sy, cz);
    let region_index = region_index as u32;
    greedy(catalog, section, scratch, local_section, region_index, partial);
    complex(catalog, section, scratch, local_section, region_index, partial);
    partial
        .connectivity
        .push((grid.slot(cx, sy, cz) as u32, connectivity(&mut scratch.occludes)));
}

#[inline]
fn border_index(x: i32, y: i32, z: i32) -> usize {
    ((y + 1) as usize) * BORDER * BORDER + ((z + 1) as usize) * BORDER + (x + 1) as usize
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
    catalog: &Catalog,
    section: &Section,
    scratch: &mut Scratch,
    local_section: u32,
    region_index: u32,
    partial: &mut Partial,
) {
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

        for n in 0..SECTION_SIZE {
            let mut any = false;
            for gv in 0..SECTION_SIZE {
                for gu in 0..SECTION_SIZE {
                    let mut local = [0i32; 3];
                    local[n_axis] = n as i32;
                    local[u_axis] = grid_to_local(gu, u_positive);
                    local[v_axis] = grid_to_local(gv, v_positive);
                    let slot = gv * SECTION_SIZE + gu;
                    scratch.used[slot] = false;
                    let key = face_key(catalog, section, scratch, local, face);
                    any |= key.sprite.is_some();
                    scratch.keys[slot] = key;
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

fn face_key(
    catalog: &Catalog,
    section: &Section,
    scratch: &Scratch,
    local: [i32; 3],
    face: usize,
) -> Key {
    let here = scratch.states[border_index(local[0], local[1], local[2])];
    let info = &catalog.blocks[here as usize];
    let Some(cube) = info.cube.as_ref() else {
        return Key::default();
    };
    let normal = face_normal(face);
    let front = [
        local[0] + normal[0],
        local[1] + normal[1],
        local[2] + normal[2],
    ];
    let front_index = border_index(front[0], front[1], front[2]);
    if scratch.occludes[front_index] {
        return Key::default();
    }
    if info.self_culls && scratch.states[front_index] == here {
        return Key::default();
    }
    let _ = section;

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
    let corners = [ao & 3, (ao >> 2) & 3, (ao >> 4) & 3, (ao >> 6) & 3];
    Key {
        sprite: Some(cube.sprite),
        tint: if cube.tinted {
            info.tint_kind as u32 + 1
        } else {
            0
        },
        // Full daylight, so vanilla's `max(block, sky * daylight)` collapses to a plain max.
        light: (raw >> 4).max(raw & 0xf).max(info.emission as u32),
        ao,
        flip: split_flip(corners),
        pass: cube.pass as u32,
    }
}

/// Which diagonal the shader splits the quad along: corners 0-2 normally, 1-3 when this is set.
///
/// Splitting a quad along the wrong diagonal makes the ambient occlusion gradient kink visibly. A
/// corner on the split lies in both triangles, so its shade bleeds across the whole quad; picking
/// the diagonal with the brighter pair keeps the darkest corner inside one triangle.
#[inline]
fn split_flip(corners: [u32; 4]) -> bool {
    corners[0] + corners[2] < corners[1] + corners[3]
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
            let key = scratch.keys[slot];
            if key.sprite.is_none() || scratch.used[slot] {
                gu += 1;
                continue;
            }
            let mut w = 1;
            while gu + w < SECTION_SIZE {
                let probe = slot + w;
                if scratch.used[probe] || scratch.keys[probe] != key {
                    break;
                }
                w += 1;
            }
            let mut h = 1;
            'grow: while gv + h < SECTION_SIZE {
                for i in 0..w {
                    let probe = (gv + h) * SECTION_SIZE + gu + i;
                    if scratch.used[probe] || scratch.keys[probe] != key {
                        break 'grow;
                    }
                }
                h += 1;
            }
            for dv in 0..h {
                for du in 0..w {
                    scratch.used[(gv + dv) * SECTION_SIZE + gu + du] = true;
                }
            }
            scratch.simple_by_pass[key.pass as usize]
                .push(pack_quad(&key, face, n, gu, gv, w, h, local_section));
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
    key: &Key,
    face: usize,
    n: usize,
    gu: usize,
    gv: usize,
    w: usize,
    h: usize,
    local_section: u32,
) -> u64 {
    let anchor = quad_anchor(face, n, gu, gv);
    let mut words = [0u32; 2];
    QUAD_X.set(&mut words, anchor[0] as u64);
    QUAD_Y.set(&mut words, anchor[1] as u64);
    QUAD_Z.set(&mut words, anchor[2] as u64);
    QUAD_FACE.set(&mut words, face as u64);
    QUAD_W.set(&mut words, w as u64 - 1);
    QUAD_H.set(&mut words, h as u64 - 1);
    QUAD_LIGHT.set(&mut words, key.light as u64);
    QUAD_TINT.set(&mut words, key.tint as u64);
    QUAD_AO.set(&mut words, key.ao as u64);
    QUAD_FLIP.set(&mut words, key.flip as u64);
    QUAD_SECTION.set(&mut words, local_section as u64);
    let sprite = key.sprite.expect("a quad is only packed where there is a face");
    QUAD_ARRAY.set(&mut words, sprite.array as u64);
    QUAD_LAYER.set(&mut words, sprite.layer as u64);
    words[0] as u64 | (words[1] as u64) << 32
}

/// Baked model geometry, written out verbatim with the block's own light and the vanilla
/// directional shade already folded into each vertex.
///
/// ponytail: smooth ambient occlusion is not recomputed per instance here, so stairs and slabs get
/// flat shading. Complex blocks are a low single-digit share of a region's quads; wire in the same
/// corner sampling the greedy path uses if builds ever dominate the view.
fn complex(
    catalog: &Catalog,
    section: &Section,
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
                let state = section.blocks[y * 256 + z * SECTION_SIZE + x];
                let info = &catalog.blocks[state as usize];
                if info.quads.is_empty() {
                    continue;
                }
                let here = border_index(x as i32, y as i32, z as i32);

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
                    let light = (raw >> 4).max(raw & 0xf).max(info.emission as u32);
                    let tint = if quad.tinted {
                        info.tint_kind as u32 + 1
                    } else {
                        0
                    };
                    let group = quad.face.map_or(FACE_GROUPS - 1, |dir| dir as usize);
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
                        MODEL_LIGHT.set(&mut words, light as u64);
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
        141..=175 => 1, // north / south, 0.6
        176..=225 => 2, // east / west, 0.8
        _ => 3,         // up, 1.0
    }
}

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
        BORDER_VOLUME, Key, QUAD_AO, QUAD_FACE, QUAD_FLIP, QUAD_H, QUAD_LAYER, QUAD_LIGHT,
        QUAD_ARRAY, QUAD_SECTION, QUAD_TINT, QUAD_W, QUAD_X, QUAD_Y, QUAD_Z, border_index,
        connectivity, fixed, pack_quad, quad_anchor, split_flip,
    };
    use crate::atlas::SpriteRef;
    use crate::anvil::SECTION_SIZE;
    use crate::pack::{MODEL_OVERHANG, MODEL_STEPS, pack_section};
    use crate::blocks::{CORNER_UV, FACE_AXES, cube_corner};
    use bevy::math::Vec3;

    fn pair(entry: usize, exit: usize) -> u64 {
        1 << (entry * 6 + exit)
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

    /// The split has to miss the darkest corner. On the other diagonal that corner belongs to both
    /// triangles and its shadow interpolates across the entire quad instead of staying in place.
    #[test]
    fn the_split_diagonal_misses_the_darkest_corner() {
        for dark in 0..4usize {
            let mut corners = [3u32; 4];
            corners[dark] = 0;
            let diagonal = if split_flip(corners) { [1, 3] } else { [0, 2] };
            assert!(
                !diagonal.contains(&dark),
                "corner {dark} dark: split runs {diagonal:?}"
            );
        }
    }

    /// Four equal corners leave nothing to fix, and the cheaper triangulation is the unflipped one.
    #[test]
    fn a_flat_quad_keeps_the_default_diagonal() {
        assert!(!split_flip([2, 2, 2, 2]));
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

    #[test]
    fn a_packed_quad_round_trips_every_field() {
        let key = Key {
            sprite: Some(SpriteRef {
                array: 2,
                layer: 1000,
            }),
            tint: 2,
            light: 11,
            ao: 0b11_10_01_00,
            flip: true,
            pass: 1,
        };
        let section = pack_section(9, 5, 12);
        let packed = pack_quad(&key, 3, 9, 3, 7, 12, 16, section);
        let words = [packed as u32, (packed >> 32) as u32];
        let anchor = quad_anchor(3, 9, 3, 7);
        assert_eq!(QUAD_X.read(&words), anchor[0] as u64, "x");
        assert_eq!(QUAD_Y.read(&words), anchor[1] as u64, "y");
        assert_eq!(QUAD_Z.read(&words), anchor[2] as u64, "z");
        assert_eq!(QUAD_SECTION.read(&words), section as u64, "section");
        assert_eq!(QUAD_FACE.read(&words), 3, "face");
        assert_eq!(QUAD_W.read(&words) + 1, 12, "w");
        assert_eq!(QUAD_H.read(&words) + 1, 16, "h");
        assert_eq!(QUAD_LIGHT.read(&words), 11, "light");
        assert_eq!(QUAD_TINT.read(&words), 2, "tint");
        assert_eq!(QUAD_AO.read(&words), 0b11_10_01_00, "ao");
        assert_eq!(QUAD_FLIP.read(&words), 1, "flip");
        assert_eq!(QUAD_ARRAY.read(&words), 2, "array");
        assert_eq!(QUAD_LAYER.read(&words), 1000, "layer");
    }
}
