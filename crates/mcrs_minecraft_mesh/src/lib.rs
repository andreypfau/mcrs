pub mod ambient;
pub mod arena;
pub mod block;
mod connectivity;
mod cube;
mod fluid;
mod model;
pub mod pack;
mod scratch;
mod sweep;
pub mod tint;

use crate::block::{BlockInfo, FACE_AXES, Pass};
use crate::pack::{
    BOUNDS_HI_X, BOUNDS_HI_Y, BOUNDS_HI_Z, BOUNDS_LO_X, BOUNDS_LO_Y, BOUNDS_LO_Z, FACE_WORDS,
    Field, MODEL_OVERHANG, MODEL_STEPS, MODEL_X, MODEL_Y, MODEL_Z, QUAD_DROP, QUAD_FACE,
    QUAD_FLUID, QUAD_H, QUAD_W, QUAD_WORDS, QUAD_X, QUAD_Y, QUAD_Z,
};

pub use connectivity::{CONNECT_ALL, Connectivity, OPEN, SEALED, along};
pub use scratch::Scratch;

pub const SECTION_SIZE: usize = mcrs_minecraft_core::SectionPos::SIZE;
pub const SECTION_VOLUME: usize = mcrs_minecraft_core::SectionPos::VOLUME;

/// The two things a section's mesh reads of the world around it, by block.
pub trait BlockView {
    fn block(&self, x: i32, y: i32, z: i32) -> u16;

    fn light(&self, x: i32, y: i32, z: i32) -> u8;
}

pub const STREAMS: usize = Pass::COUNT * 2;

/// Most quads a group holds. The cull tests a group as one box, so a small group hugs its quads
/// and hides behind terrain that a whole section's worth of them would reach past.
pub const GROUP_QUADS: usize = 32;

pub const STREAM_NAMES: [&str; STREAMS] = [
    "solid greedy",
    "solid model",
    "cutout greedy",
    "cutout model",
    "translucent greedy",
    "translucent model",
];

pub const fn stream_pass(stream: u32) -> Pass {
    Pass::from_index(stream as usize / 2)
}

pub const fn stream_is_model(stream: u32) -> bool {
    stream % 2 == 1
}

#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Group {
    pub quad_base: u32,
    pub quad_count: u32,
    pub section: u32,
    pub face: u32,
    /// The box its quads lie in, packed by the `BOUNDS_*` fields.
    pub bounds: u32,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct StreamSpan {
    pub group_count: u32,
    pub quad_count: u32,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct Draw {
    pub stream: u32,
    pub first_group: u32,
    pub group_count: u32,
    pub quad_count: u32,
}

pub struct SectionMesh {
    pub section: [i32; 3],
    pub simple: Vec<[u32; QUAD_WORDS]>,
    pub faces: Vec<[u32; FACE_WORDS]>,
    pub complex: Vec<u32>,
    pub groups: Vec<Group>,
    pub spans: [StreamSpan; STREAMS],
    pub connectivity: Connectivity,
}

impl SectionMesh {
    pub fn model_quads(&self) -> usize {
        self.complex.len() / model::WORDS_PER_QUAD
    }
}

#[inline]
pub const fn face_normal(face: usize) -> [i32; 3] {
    let axes = FACE_AXES[face];
    let mut normal = [0i32; 3];
    normal[axes[0] as usize] = if axes[1] == 1 { 1 } else { -1 };
    normal
}

struct Sink {
    simple: Vec<[u32; QUAD_WORDS]>,
    complex: Vec<u32>,
    groups: Vec<(u32, Group)>,
    slot: u32,
}

impl Sink {
    fn group(&mut self, stream: usize, face: u64, quad_base: usize, quad_count: usize, bounds: u32) {
        self.groups.push((
            stream as u32,
            Group {
                quad_base: quad_base as u32,
                quad_count: quad_count as u32,
                section: self.slot,
                face: face as u32,
                bounds,
            },
        ));
    }

    fn simple(&mut self, pass: usize, face: u64, quads: &[[u32; QUAD_WORDS]]) {
        for run in quads.chunks(GROUP_QUADS) {
            let base = self.simple.len();
            let mut bounds = Bounds::EMPTY;
            for quad in run {
                bounds.cover_greedy(quad[0]);
            }
            self.group(pass * 2, face, base, run.len(), bounds.pack());
            self.simple.extend_from_slice(run);
        }
    }

    fn complex(&mut self, pass: usize, face: u64, verts: &[u32]) {
        for run in verts.chunks(GROUP_QUADS * model::WORDS_PER_QUAD) {
            let base = self.complex.len() / model::WORDS_PER_QUAD;
            let mut bounds = Bounds::EMPTY;
            for vertex in run.chunks(model::WORDS_PER_QUAD / 4) {
                bounds.cover_model(vertex);
            }
            self.group(
                pass * 2 + 1,
                face,
                base,
                run.len() / model::WORDS_PER_QUAD,
                bounds.pack(),
            );
            self.complex.extend_from_slice(run);
        }
    }
}

/// A box in blocks from a section's corner, grown to take in quads as `quad.wgsl` places them.
struct Bounds {
    lo: [f32; 3],
    hi: [f32; 3],
}

impl Bounds {
    const EMPTY: Self = Self {
        lo: [f32::MAX; 3],
        hi: [f32::MIN; 3],
    };

    fn cover(&mut self, point: [f32; 3]) {
        for axis in 0..3 {
            self.lo[axis] = self.lo[axis].min(point[axis]);
            self.hi[axis] = self.hi[axis].max(point[axis]);
        }
    }

    /// The rectangle spans the quad's size along its face's u and v axes from its corner. A
    /// fluid surface drops below the plane it names and is pulled in off the face behind it,
    /// both along the normal and by less than a block.
    fn cover_greedy(&mut self, word: u32) {
        let word = word as u64;
        let corner = [QUAD_X, QUAD_Y, QUAD_Z].map(|field| field.get(word) as f32);
        let axes = FACE_AXES[QUAD_FACE.get(word) as usize];
        let along = |axis: u8, positive: u8, size: Field| {
            let size = (size.get(word) + 1) as f32;
            let mut step = [0.0; 3];
            step[axis as usize] = if positive == 1 { size } else { -size };
            step
        };
        let u = along(axes[2], axes[3], QUAD_W);
        let v = along(axes[4], axes[5], QUAD_H);
        let far = std::array::from_fn(|axis| corner[axis] + u[axis] + v[axis]);
        self.cover(corner);
        self.cover(far);
        if QUAD_DROP.get(word) != 0 || QUAD_FLUID.get(word) != 0 {
            let normal = axes[0] as usize;
            let mut below = corner;
            below[normal] -= 1.0;
            let mut above = corner;
            above[normal] += 1.0;
            self.cover(below);
            self.cover(above);
        }
    }

    fn cover_model(&mut self, vertex: &[u32]) {
        self.cover([MODEL_X, MODEL_Y, MODEL_Z].map(|field| {
            field.get(vertex[field.word as usize] as u64) as f32 / MODEL_STEPS - MODEL_OVERHANG
        }));
    }

    fn pack(&self) -> u32 {
        let field = |field: Field, value: f32| {
            field.pack((value + MODEL_OVERHANG).clamp(0.0, field.max() as f32) as u64) as u32
        };
        field(BOUNDS_LO_X, self.lo[0].floor())
            | field(BOUNDS_LO_Y, self.lo[1].floor())
            | field(BOUNDS_LO_Z, self.lo[2].floor())
            | field(BOUNDS_HI_X, self.hi[0].ceil())
            | field(BOUNDS_HI_Y, self.hi[1].ceil())
            | field(BOUNDS_HI_Z, self.hi[2].ceil())
    }
}

pub fn mesh_section(
    world: &impl BlockView,
    catalog: &[BlockInfo],
    section: [i32; 3],
    slot: u32,
    scratch: &mut Scratch,
) -> SectionMesh {
    scratch.load(world, catalog, section.map(|n| n * SECTION_SIZE as i32));

    let mut sink = Sink {
        simple: Vec::new(),
        complex: Vec::new(),
        groups: Vec::new(),
        slot,
    };

    scratch.section_faces.clear();
    fluid::surfaces(scratch);
    cube::greedy(catalog, scratch, &mut sink);
    fluid::greedy(catalog, scratch, &mut sink);

    model::blocks(catalog, scratch);
    fluid::models(catalog, scratch);
    model::emit(scratch, &mut sink);

    let mut groups = Vec::with_capacity(sink.groups.len());
    let mut spans = [StreamSpan::default(); STREAMS];
    for stream in 0..STREAMS {
        let first = groups.len();
        let mut quads = 0u32;
        for &(from, group) in &sink.groups {
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

    SectionMesh {
        section,
        simple: sink.simple,
        faces: std::mem::take(&mut scratch.section_faces),
        complex: sink.complex,
        groups,
        spans,
        connectivity: connectivity::connectivity(&scratch.occludes),
    }
}

/// One unlit section at the world origin, with every block chosen by `pick`.
#[cfg(test)]
pub fn one_section_world(pick: impl Fn(usize, usize, usize) -> u16) -> OneSection {
    let mut blocks = Box::new([0u16; SECTION_VOLUME]);
    for y in 0..SECTION_SIZE {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                blocks[(y * SECTION_SIZE + z) * SECTION_SIZE + x] = pick(x, y, z);
            }
        }
    }
    OneSection(blocks)
}

/// The section at the origin, alone in its column: the rows beside it in the
/// column are dark, and everything past the column is open sky.
#[cfg(test)]
pub struct OneSection(Box<[u16; SECTION_VOLUME]>);

#[cfg(test)]
impl BlockView for OneSection {
    fn block(&self, x: i32, y: i32, z: i32) -> u16 {
        let pos = mcrs_minecraft_core::BlockPos::new(x, y, z);
        let at = mcrs_minecraft_core::SectionPos::from(pos);
        if at.x == 0 && at.y == 0 && at.z == 0 {
            self.0[mcrs_minecraft_core::LocalPos::from(pos).index()]
        } else {
            0
        }
    }

    fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        let at = mcrs_minecraft_core::SectionPos::from(mcrs_minecraft_core::BlockPos::new(x, y, z));
        if at.x != 0 || at.z != 0 || at.y > 1 {
            0x0f
        } else {
            0
        }
    }
}

#[cfg(test)]
pub struct Batch {
    pub simple: Vec<[u32; QUAD_WORDS]>,
    pub faces: Vec<[u32; FACE_WORDS]>,
    pub face_base: Vec<u32>,
    pub quad_section: Vec<u32>,
    pub complex: Vec<u32>,
}

#[cfg(test)]
impl Batch {
    pub fn model_quads(&self) -> usize {
        self.complex.len() / model::WORDS_PER_QUAD
    }
}

/// The named sections, meshed into one batch, with a table slot handed out in walk order.
#[cfg(test)]
pub fn mesh_world(
    world: &impl BlockView,
    catalog: &[BlockInfo],
    sections: &[[i32; 3]],
    scratch: &mut Scratch,
) -> Batch {
    let mut batch = Batch {
        simple: Vec::new(),
        faces: Vec::new(),
        face_base: Vec::new(),
        quad_section: Vec::new(),
        complex: Vec::new(),
    };
    for &section in sections {
        let slot = batch.face_base.len() as u32;
        let mesh = mesh_section(world, catalog, section, slot, scratch);
        batch.simple.extend_from_slice(&mesh.simple);
        batch.quad_section.resize(batch.simple.len(), slot);
        batch.complex.extend_from_slice(&mesh.complex);
        batch.face_base.push(batch.faces.len() as u32);
        batch.faces.extend_from_slice(&mesh.faces);
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{CubeFace, ModelQuad};
    use bevy_math::Vec3;

    #[test]
    fn a_reused_scratch_meshes_a_section_exactly_as_a_fresh_one_does() {
        const STONE: u16 = 1;
        const BUSH: u16 = 2;
        let mut catalog: Vec<BlockInfo> = (0..3).map(|_| BlockInfo::default()).collect();
        catalog[STONE as usize].cube = Some(
            [CubeFace {
                sprite: 1,
                pass: Pass::Solid as u8,
                tint: crate::tint::Tint::None,
            }; 6],
        );
        catalog[STONE as usize].occludes = true;
        catalog[BUSH as usize].quads = vec![ModelQuad {
            positions: [Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Y],
            uvs: [[0.0; 2]; 4],
            cull: None,
            facing: mcrs_minecraft_core::Direction::Up,
            face: None,
            sprite: 0,
            pass: Pass::Cutout,
            shade: 1.0,
            tint: crate::tint::Tint::None,
        }];

        let subject = one_section_world(|x, y, z| match (x + y + z) % 4 {
            0 => STONE,
            1 => BUSH,
            _ => 0,
        });
        let other = one_section_world(|x, _, z| if x > z { BUSH } else { STONE });

        let fresh = mesh_section(&subject, &catalog, [0, 0, 0], 0, &mut Scratch::new());

        let mut carried = Scratch::new();
        mesh_section(&other, &catalog, [0, 0, 0], 0, &mut carried);
        let again = mesh_section(&subject, &catalog, [0, 0, 0], 0, &mut carried);

        assert_eq!(fresh.faces, again.faces, "face attributes");
        assert_eq!(fresh.simple, again.simple, "greedy quads");
        assert_eq!(fresh.complex, again.complex, "model vertices");
        assert_eq!(fresh.groups.len(), again.groups.len(), "groups");
        assert_eq!(fresh.connectivity, again.connectivity, "connectivity");
    }

    fn unpack_bounds(bounds: u32) -> ([f32; 3], [f32; 3]) {
        let get = |field: Field| field.get(bounds as u64) as f32 - MODEL_OVERHANG;
        (
            [get(BOUNDS_LO_X), get(BOUNDS_LO_Y), get(BOUNDS_LO_Z)],
            [get(BOUNDS_HI_X), get(BOUNDS_HI_Y), get(BOUNDS_HI_Z)],
        )
    }

    #[test]
    fn a_lone_block_is_boxed_to_its_own_cell() {
        const STONE: u16 = 1;
        const BUSH: u16 = 2;
        let mut catalog: Vec<BlockInfo> = (0..3).map(|_| BlockInfo::default()).collect();
        catalog[STONE as usize].cube = Some(
            [CubeFace {
                sprite: 1,
                pass: Pass::Solid as u8,
                tint: crate::tint::Tint::None,
            }; 6],
        );
        catalog[STONE as usize].occludes = true;
        catalog[BUSH as usize].quads = vec![ModelQuad {
            positions: [Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Y],
            uvs: [[0.0; 2]; 4],
            cull: None,
            facing: mcrs_minecraft_core::Direction::Up,
            face: None,
            sprite: 0,
            pass: Pass::Cutout,
            shade: 1.0,
            tint: crate::tint::Tint::None,
        }];

        for (at, state) in [([3, 4, 5], STONE), ([15, 0, 9], STONE), ([7, 12, 0], BUSH)] {
            let world = one_section_world(|x, y, z| if [x, y, z] == at { state } else { 0 });
            let mesh = mesh_section(&world, &catalog, [0, 0, 0], 0, &mut Scratch::new());
            assert!(!mesh.groups.is_empty());
            let cell = at.map(|n| n as f32);
            for group in &mesh.groups {
                let (lo, hi) = unpack_bounds(group.bounds);
                for axis in 0..3 {
                    assert!(
                        lo[axis] >= cell[axis] && hi[axis] <= cell[axis] + 1.0,
                        "a group of the block at {at:?} is boxed {lo:?}..{hi:?}"
                    );
                    assert!(lo[axis] <= hi[axis]);
                }
            }
        }
    }

    #[test]
    fn a_long_run_of_quads_is_split_into_groups_no_larger_than_the_cap() {
        const BUSH: u16 = 2;
        let mut catalog: Vec<BlockInfo> = (0..3).map(|_| BlockInfo::default()).collect();
        catalog[BUSH as usize].quads = vec![ModelQuad {
            positions: [Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Y],
            uvs: [[0.0; 2]; 4],
            cull: None,
            facing: mcrs_minecraft_core::Direction::Up,
            face: None,
            sprite: 0,
            pass: Pass::Cutout,
            shade: 1.0,
            tint: crate::tint::Tint::None,
        }];
        let world = one_section_world(|_, y, _| if y == 0 { BUSH } else { 0 });
        let mesh = mesh_section(&world, &catalog, [0, 0, 0], 0, &mut Scratch::new());
        assert_eq!(mesh.model_quads(), SECTION_SIZE * SECTION_SIZE);
        assert!(mesh.groups.iter().all(|group| group.quad_count as usize <= GROUP_QUADS));
    }

    #[test]
    fn a_field_of_snow_is_one_lowered_top_over_hidden_ground() {
        use crate::block::FaceShapes;
        use crate::pack::{QUAD_DROP, QUAD_FACE};
        const STONE: u16 = 1;
        const SNOW: u16 = 2;
        let face = |sprite| CubeFace {
            sprite,
            pass: Pass::Solid as u8,
            tint: crate::tint::Tint::None,
        };
        let mut catalog: Vec<BlockInfo> = (0..3).map(|_| BlockInfo::default()).collect();
        catalog[STONE as usize].cube = Some([face(1); 6]);
        catalog[STONE as usize].occludes = true;
        catalog[STONE as usize].faces = Some(Box::new(FaceShapes {
            outer: [[u16::MAX; 16]; 6],
            inner: [[u16::MAX; 16]; 6],
        }));
        // One layer: the floor is covered whole, each side along its lowest two rows.
        let mut sides = [[0u16; 16]; 6];
        sides[0] = [u16::MAX; 16];
        for side in 2..6 {
            let rows_are_height = side >= 4;
            for row in 0..16 {
                sides[side][row] = if rows_are_height {
                    if row < 2 { u16::MAX } else { 0 }
                } else {
                    0b11
                };
            }
        }
        catalog[SNOW as usize].cube = Some([face(2); 6]);
        catalog[SNOW as usize].drop = 28;
        catalog[SNOW as usize].faces = Some(Box::new(FaceShapes {
            outer: sides,
            inner: sides,
        }));
        catalog[SNOW as usize].ambient_occlusion = true;

        let world = one_section_world(|_, y, _| match y {
            0 => STONE,
            1 => SNOW,
            _ => 0,
        });
        let mesh = mesh_section(&world, &catalog, [0, 0, 0], 0, &mut Scratch::new());
        let tops: Vec<_> = mesh
            .simple
            .iter()
            .filter(|quad| QUAD_FACE.read(*quad) == 1)
            .collect();
        assert_eq!(tops.len(), 1, "the snow tops merge and the stone under them is hidden");
        assert_eq!(QUAD_DROP.read(tops[0]), 28);
    }

    #[test]
    fn the_groups_of_a_section_tile_the_quads_of_that_section() {
        const STONE: u16 = 1;
        const BUSH: u16 = 2;
        let world = one_section_world(|x, _, _| if x < 8 { STONE } else { BUSH });

        let mut catalog: Vec<BlockInfo> = (0..3).map(|_| BlockInfo::default()).collect();
        catalog[STONE as usize].cube = Some(
            [CubeFace {
                sprite: 1,
                pass: Pass::Solid as u8,
                tint: crate::tint::Tint::None,
            }; 6],
        );
        catalog[STONE as usize].occludes = true;
        catalog[BUSH as usize].quads = vec![ModelQuad {
            positions: [Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Y],
            uvs: [[0.0; 2]; 4],
            cull: None,
            facing: mcrs_minecraft_core::Direction::Up,
            face: None,
            sprite: 0,
            pass: Pass::Cutout,
            shade: 1.0,
            tint: crate::tint::Tint::None,
        }];

        let mut scratch = Scratch::new();
        let mesh = mesh_section(&world, &catalog, [0, 0, 0], 0, &mut scratch);

        assert!(
            mesh.spans[0].quad_count > 0 && mesh.spans[3].quad_count > 0,
            "the fixture has to reach both a greedy and a model bucket"
        );
        let mut at = 0usize;
        for stream in 0..STREAMS {
            let run = mesh.spans[stream].group_count as usize;
            let held = &mesh.groups[at..at + run];
            at += run;
            let mut quads = 0u32;
            let mut covered = 0u32;
            for group in held {
                assert_eq!(group.quad_base, covered, "stream {stream} leaves a hole");
                quads += group.quad_count;
                covered += group.quad_count;
            }
            assert_eq!(quads, mesh.spans[stream].quad_count);
            let total = match stream_is_model(stream as u32) {
                true => mesh.model_quads(),
                false => mesh.simple.len(),
            };
            assert!(
                covered as usize <= total,
                "stream {stream} names more quads than the section holds"
            );
        }
        assert_eq!(at, mesh.groups.len(), "a group belongs to no bucket");
    }
}
