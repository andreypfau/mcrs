mod connectivity;
mod cube;
mod fluid;
mod model;
mod scratch;
mod sweep;

use crate::anvil::{SECTION_SIZE, World};
use crate::blocks::{BlockInfo, FACE_AXES, Pass};
use crate::pack::QUAD_WORDS;

pub use connectivity::CONNECT_ALL;
pub use scratch::Scratch;

pub const STREAMS: usize = Pass::COUNT * 2;

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
    pub quad_prefix: u32,
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
    pub section: [usize; 3],
    pub simple: Vec<[u32; QUAD_WORDS]>,
    pub faces: Vec<u32>,
    pub complex: Vec<u32>,
    pub groups: Vec<Group>,
    pub spans: [StreamSpan; STREAMS],
    pub connectivity: u64,
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

struct Partial {
    simple: Vec<[u32; QUAD_WORDS]>,
    complex: Vec<u32>,
    groups: Vec<(u32, Group)>,
}

struct Sink<'a> {
    partial: &'a mut Partial,
    slot: u32,
}

impl Sink<'_> {
    fn group(&mut self, stream: usize, face: u64, quad_base: usize, quad_count: usize) {
        self.partial.groups.push((
            stream as u32,
            Group {
                quad_base: quad_base as u32,
                quad_count: quad_count as u32,
                section: self.slot,
                face: face as u32,
                quad_prefix: 0,
            },
        ));
    }

    fn simple(&mut self, pass: usize, face: u64, quads: &[[u32; QUAD_WORDS]]) {
        if quads.is_empty() {
            return;
        }
        let base = self.partial.simple.len();
        self.group(pass * 2, face, base, quads.len());
        self.partial.simple.extend_from_slice(quads);
    }

    fn complex(&mut self, pass: usize, face: u64, verts: &[u32]) {
        if verts.is_empty() {
            return;
        }
        let base = self.partial.complex.len() / model::WORDS_PER_QUAD;
        self.group(
            pass * 2 + 1,
            face,
            base,
            verts.len() / model::WORDS_PER_QUAD,
        );
        self.partial.complex.extend_from_slice(verts);
    }
}

pub fn mesh_section(
    world: &World,
    catalog: &[BlockInfo],
    [sx, sy, sz]: [usize; 3],
    slot: u32,
    scratch: &mut Scratch,
) -> SectionMesh {
    scratch.load(
        world,
        catalog,
        [
            (sx * SECTION_SIZE) as i32,
            (sy as i32 + world.min_section[1]) * SECTION_SIZE as i32,
            (sz * SECTION_SIZE) as i32,
        ],
    );

    let mut partial = Partial {
        simple: Vec::new(),
        complex: Vec::new(),
        groups: Vec::new(),
    };
    let mut sink = Sink {
        partial: &mut partial,
        slot,
    };

    scratch.section_faces.clear();
    fluid::surfaces(scratch);
    cube::greedy(catalog, scratch, &mut sink);
    fluid::greedy(catalog, scratch, &mut sink);

    model::blocks(catalog, scratch);
    fluid::models(catalog, scratch);
    model::emit(scratch, &mut sink);

    let mut groups = Vec::with_capacity(partial.groups.len());
    let mut spans = [StreamSpan::default(); STREAMS];
    for stream in 0..STREAMS {
        let first = groups.len();
        let mut quads = 0u32;
        for &(from, group) in &partial.groups {
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

    SectionMesh {
        section: [sx, sy, sz],
        simple: partial.simple,
        faces: std::mem::take(&mut scratch.section_faces),
        complex: partial.complex,
        groups,
        spans,
        connectivity: connectivity::connectivity(&mut scratch.occludes),
    }
}

#[cfg(test)]
pub struct Batch {
    pub simple: Vec<[u32; QUAD_WORDS]>,
    pub faces: Vec<u32>,
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

/// Every section of a world, meshed into one batch, with a table slot handed out in walk order.
#[cfg(test)]
pub fn mesh_world(world: &World, catalog: &[BlockInfo], scratch: &mut Scratch) -> Batch {
    let mut batch = Batch {
        simple: Vec::new(),
        faces: Vec::new(),
        face_base: Vec::new(),
        quad_section: Vec::new(),
        complex: Vec::new(),
    };
    for sz in 0..world.sections[2] {
        for sx in 0..world.sections[0] {
            for sy in 0..world.sections[1] {
                if world.section(sx, sy, sz).is_none() {
                    continue;
                }
                let slot = batch.face_base.len() as u32;
                let mesh = mesh_section(world, catalog, [sx, sy, sz], slot, scratch);
                batch.simple.extend_from_slice(&mesh.simple);
                batch.quad_section.resize(batch.simple.len(), slot);
                batch.complex.extend_from_slice(&mesh.complex);
                batch.face_base.push(batch.faces.len() as u32);
                batch.faces.extend_from_slice(&mesh.faces);
            }
        }
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anvil::{Palette, one_section_region_of};
    use crate::atlas::SpriteRef;
    use crate::blocks::{CubeFace, ModelQuad};
    use bevy::math::Vec3;

    #[test]
    fn the_groups_of_a_section_tile_the_quads_of_that_section() {
        const STONE: usize = 0;
        const BUSH: usize = 1;
        let mut palette = Palette::new();
        let mut world = World::new([0, 0], [1, 1]);
        world.insert(
            &mut palette,
            [0, 0],
            one_section_region_of(&["minecraft:stone", "minecraft:bush"], |x, _, _| {
                if x < 8 { STONE } else { BUSH }
            }),
        );
        let id = |name: &str| {
            palette
                .states
                .iter()
                .position(|state| state.name == name)
                .expect("the fixture interned it")
        };
        let mut catalog: Vec<BlockInfo> = (0..palette.states.len())
            .map(|_| BlockInfo::default())
            .collect();
        let stone = id("minecraft:stone");
        catalog[stone].cube = Some(
            [CubeFace {
                sprite: SpriteRef { array: 0, layer: 1 },
                pass: Pass::Solid as u8,
                tinted: false,
            }; 6],
        );
        catalog[stone].occludes = true;
        catalog[id("minecraft:bush")].quads = vec![ModelQuad {
            positions: [Vec3::ZERO, Vec3::X, Vec3::ONE, Vec3::Y],
            uvs: [[0.0; 2]; 4],
            cull: None,
            face: None,
            sprite: SpriteRef::default(),
            pass: Pass::Cutout,
            shade: [255; 4],
            tinted: false,
        }];

        let mut scratch = Scratch::new();
        let filled = (0..world.sections[1])
            .find(|sy| world.section(0, *sy, 0).is_some())
            .expect("the fixture holds one section");
        let mesh = mesh_section(&world, &catalog, [0, filled, 0], 0, &mut scratch);

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
                assert_eq!(group.quad_prefix, quads, "stream {stream} skips a slot");
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
