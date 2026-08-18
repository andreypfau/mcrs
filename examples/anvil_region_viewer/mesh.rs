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

/// Quads live in region-absolute block coordinates, and `y` is biased so the lowest section starts
/// at zero. 10 / 9 / 10 bits then cover a whole region without a per-draw section origin.
pub const Y_BIAS: i32 = 64;

const BORDER: usize = SECTION_SIZE + 2;
const BORDER_VOLUME: usize = BORDER * BORDER * BORDER;

/// One contiguous run of quads from a single section and face group. The unit the compute shader
/// culls: 64 threads per run scatter its surviving quad indices into the visible list.
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Group {
    pub quad_base: u32,
    pub quad_count: u32,
    /// `sx | sy << 5 | sz << 10 | face << 15`, section coordinates in units of 16 blocks.
    pub section: u32,
    pub _pad: u32,
}

pub struct RegionMesh {
    /// Packed greedy quads, two `u32` each.
    pub simple: Vec<u64>,
    /// Packed model vertices, three `u32` each, four vertices per quad.
    pub complex: Vec<u32>,
    /// Culling groups, ordered so each stream owns one contiguous span.
    pub groups: Vec<Group>,
    /// `(first_group, group_count, first_quad, quad_count)` per stream.
    pub streams: [StreamSpan; STREAMS],
}

#[derive(Copy, Clone, Default, Debug)]
pub struct StreamSpan {
    pub first_group: u32,
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
    /// Zero means there is no face here.
    layer_plus_one: u32,
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
    complex_by_pass: [Vec<u32>; Pass::COUNT],
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
    /// `(stream, group)` with `group.quad_base` relative to this worker's own arena.
    groups: Vec<(u32, Group)>,
}

pub fn build(region: &Region, catalog: &Catalog) -> RegionMesh {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let next = AtomicUsize::new(0);
    let partials: Mutex<Vec<Partial>> = Mutex::new(Vec::new());
    let columns = REGION_CHUNKS * REGION_CHUNKS;

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut scratch = Scratch::new();
                let mut partial = Partial {
                    simple: Vec::new(),
                    complex: Vec::new(),
                    groups: Vec::new(),
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
                            mesh_section(region, catalog, cx, sy, cz, &mut scratch, &mut partial);
                        }
                    }
                }
                partials.lock().unwrap().push(partial);
            });
        }
    });

    assemble(partials.into_inner().unwrap())
}

fn assemble(partials: Vec<Partial>) -> RegionMesh {
    let mut simple = Vec::new();
    let mut complex = Vec::new();
    let mut by_stream: Vec<Vec<Group>> = (0..STREAMS).map(|_| Vec::new()).collect();
    let mut simple_base = 0u32;
    let mut complex_base = 0u32;

    for partial in &partials {
        for &(stream, group) in &partial.groups {
            let base = if stream % 2 == 0 {
                simple_base
            } else {
                complex_base
            };
            by_stream[stream as usize].push(Group {
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
    for stream in 0..STREAMS {
        let mut quads = 0u32;
        for group in &by_stream[stream] {
            quads += group.quad_count;
        }
        streams[stream] = StreamSpan {
            first_group: groups.len() as u32,
            group_count: by_stream[stream].len() as u32,
            quad_count: quads,
        };
        groups.append(&mut by_stream[stream]);
    }

    RegionMesh {
        simple,
        complex,
        groups,
        streams,
    }
}

fn mesh_section(
    region: &Region,
    catalog: &Catalog,
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
    let packed_section = (cx as u32) | ((sy as u32) << 5) | ((cz as u32) << 10);
    greedy(catalog, section, scratch, base, packed_section, partial);
    complex(catalog, section, scratch, base, packed_section, partial);
}

#[inline]
fn border_index(x: i32, y: i32, z: i32) -> usize {
    ((y + 1) as usize) * BORDER * BORDER + ((z + 1) as usize) * BORDER + (x + 1) as usize
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
    base: [i32; 3],
    packed_section: u32,
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
                    any |= key.layer_plus_one != 0;
                    scratch.keys[slot] = key;
                }
            }
            if any {
                merge_slice(scratch, face, n, base);
            }
        }

        for pass in 0..Pass::COUNT {
            let quads = std::mem::take(&mut scratch.simple_by_pass[pass]);
            if !quads.is_empty() {
                partial.groups.push((
                    (pass * 2) as u32,
                    Group {
                        quad_base: partial.simple.len() as u32,
                        quad_count: quads.len() as u32,
                        section: packed_section | ((face as u32) << 15),
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
        layer_plus_one: cube.layer as u32 + 1,
        tint: if cube.tinted {
            info.tint_kind as u32 + 1
        } else {
            0
        },
        // Full daylight, so vanilla's `max(block, sky * daylight)` collapses to a plain max.
        light: (raw >> 4).max(raw & 0xf).max(info.emission as u32),
        ao,
        // Splitting a quad along the wrong diagonal makes the ambient occlusion gradient kink
        // visibly; the shader picks the other diagonal when this is set.
        flip: corners[0] + corners[2] != corners[1] + corners[3],
        pass: cube.pass as u32,
    }
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

fn merge_slice(scratch: &mut Scratch, face: usize, n: usize, base: [i32; 3]) {
    for gv in 0..SECTION_SIZE {
        let mut gu = 0usize;
        while gu < SECTION_SIZE {
            let slot = gv * SECTION_SIZE + gu;
            let key = scratch.keys[slot];
            if key.layer_plus_one == 0 || scratch.used[slot] {
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
                .push(pack_quad(&key, face, n, gu, gv, w, h, base));
            gu += w;
        }
    }
}

/// The world-space anchor of a greedy quad: the corner at grid `(gu, gv)` on the face plane.
#[inline]
fn quad_anchor(face: usize, n: usize, gu: usize, gv: usize, base: [i32; 3]) -> [i32; 3] {
    let axes = FACE_AXES[face];
    let mut world = [0i32; 3];
    world[axes[0] as usize] = base[axes[0] as usize] + n as i32 + if axes[1] == 1 { 1 } else { 0 };
    world[axes[2] as usize] = base[axes[2] as usize]
        + if axes[3] == 1 {
            gu as i32
        } else {
            SECTION_SIZE as i32 - gu as i32
        };
    world[axes[4] as usize] = base[axes[4] as usize]
        + if axes[5] == 1 {
            gv as i32
        } else {
            SECTION_SIZE as i32 - gv as i32
        };
    world
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
    base: [i32; 3],
) -> u64 {
    let anchor = quad_anchor(face, n, gu, gv, base);
    let lo = (anchor[0] as u64 & 0x3ff)
        | (((anchor[1] + Y_BIAS) as u64 & 0x1ff) << 10)
        | ((anchor[2] as u64 & 0x3ff) << 19)
        | ((face as u64 & 0x7) << 29);
    let hi = ((w as u64 - 1) & 0xf)
        | (((h as u64 - 1) & 0xf) << 4)
        | (((key.layer_plus_one as u64 - 1) & 0x1ff) << 8)
        | ((key.ao as u64 & 0xff) << 17)
        | ((key.light as u64 & 0xf) << 25)
        | ((key.flip as u64) << 29)
        | ((key.tint as u64 & 0x3) << 30);
    lo | (hi << 32)
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
    base: [i32; 3],
    packed_section: u32,
    partial: &mut Partial,
) {
    for pass in 0..Pass::COUNT {
        scratch.complex_by_pass[pass].clear();
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
                    let out = &mut scratch.complex_by_pass[quad.pass as usize];
                    for corner in 0..4 {
                        let p = quad.positions[corner];
                        let u = (quad.uvs[corner][0].clamp(0.0, 1.0) * 1023.0) as u32;
                        let v = (quad.uvs[corner][1].clamp(0.0, 1.0) * 1023.0) as u32;
                        out.push(
                            fixed(p.x + (base[0] + x as i32) as f32)
                                | (fixed(p.y + (base[1] + Y_BIAS + y as i32) as f32) << 16),
                        );
                        out.push(
                            fixed(p.z + (base[2] + z as i32) as f32) | (u << 16) | (tint << 26),
                        );
                        out.push(
                            quad.layer as u32
                                | (v << 16)
                                | (light << 26)
                                | (shade_bucket(quad.shade[corner]) << 30),
                        );
                    }
                }
            }
        }
    }

    for pass in 0..Pass::COUNT {
        let verts = std::mem::take(&mut scratch.complex_by_pass[pass]);
        if !verts.is_empty() {
            partial.groups.push((
                (pass * 2 + 1) as u32,
                Group {
                    quad_base: (partial.complex.len() / 3 / 4) as u32,
                    quad_count: (verts.len() / 3 / 4) as u32,
                    section: packed_section | (FACE_NONE << 15),
                    _pad: 0,
                },
            ));
            partial.complex.extend_from_slice(&verts);
        }
        scratch.complex_by_pass[pass] = verts;
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

/// Model positions live in 1/32 of a block, biased two blocks, so a face that pokes outside its own
/// block — a fence arm, a rail on a slope — still packs into 16 bits alongside the region offset.
#[inline]
fn fixed(value: f32) -> u32 {
    ((value + 2.0) * 32.0).round().clamp(0.0, 65535.0) as u32
}

#[cfg(test)]
mod tests {
    use super::{Key, Y_BIAS, fixed, pack_quad, quad_anchor};
    use crate::blocks::{CORNER_UV, FACE_AXES, cube_corner};
    use bevy::math::Vec3;

    #[test]
    fn fixed_point_covers_the_overhang_a_model_can_have() {
        assert_eq!(fixed(-2.0), 0);
        assert_eq!(fixed(0.0), 64);
        assert_eq!(fixed(1.0), 96);
        assert_eq!(fixed(0.5), 80);
        assert_eq!(fixed(512.0), 16448);
    }

    /// A one-block quad must land on exactly the same corners the model baker produces for that
    /// face, otherwise greedy geometry and baked geometry would not line up at a seam.
    #[test]
    fn a_single_block_greedy_quad_matches_the_baked_cube() {
        for face in 0..6usize {
            let axes = FACE_AXES[face];
            // Place the block at local (0,0,0) of a section rooted at the origin.
            let mut grid = [0usize; 3];
            grid[axes[2] as usize] = 0;
            let gu = if axes[3] == 1 { 0 } else { 15 };
            let gv = if axes[5] == 1 { 0 } else { 15 };
            let _ = grid;
            let anchor = quad_anchor(face, 0, gu, gv, [0, 0, 0]);

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
            layer_plus_one: 300 + 1,
            tint: 2,
            light: 11,
            ao: 0b11_10_01_00,
            flip: true,
            pass: 1,
        };
        let packed = pack_quad(&key, 3, 9, 3, 7, 12, 16, [32, -64, 480]);
        let lo = packed as u32;
        let hi = (packed >> 32) as u32;
        let anchor = quad_anchor(3, 9, 3, 7, [32, -64, 480]);
        assert_eq!(lo & 0x3ff, anchor[0] as u32, "x");
        assert_eq!((lo >> 10) & 0x1ff, (anchor[1] + Y_BIAS) as u32, "y");
        assert_eq!((lo >> 19) & 0x3ff, anchor[2] as u32, "z");
        assert_eq!((lo >> 29) & 0x7, 3, "face");
        assert_eq!((hi & 0xf) + 1, 12, "w");
        assert_eq!(((hi >> 4) & 0xf) + 1, 16, "h");
        assert_eq!((hi >> 8) & 0x1ff, 300, "layer");
        assert_eq!((hi >> 17) & 0xff, 0b11_10_01_00, "ao");
        assert_eq!((hi >> 25) & 0xf, 11, "light");
        assert_eq!((hi >> 29) & 1, 1, "flip");
        assert_eq!((hi >> 30) & 0x3, 2, "tint");
    }
}
