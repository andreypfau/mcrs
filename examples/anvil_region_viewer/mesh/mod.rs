mod connectivity;
mod cube;
mod fluid;
mod model;
mod scratch;
mod sweep;

use crate::anvil::{SECTION_SIZE, World};
use crate::blocks::{BlockInfo, FACE_AXES, Pass};
use crate::pack::{
    GROUP_FACE, QUAD_WORDS, RENDER_REGION_X, RENDER_REGION_Y, RENDER_REGION_Z, RegionGrid,
    SECTION_FACE_TABLE,
};

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

#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Group {
    pub quad_base: u32,
    pub quad_count: u32,
    pub section: u32,
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
    pub region: u32,
    pub origin: [i32; 3],
    pub cave_base: u32,
    pub face_base: u32,
    pub first_group: u32,
    pub group_count: u32,
    pub quad_count: u32,
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
    faces: Vec<u32>,
    section_faces: Vec<(u32, u32)>,
    complex: Vec<u32>,
    groups: Vec<(u32, u32, Group)>,
    connectivity: Vec<(u32, u64)>,
}

struct Sink<'a> {
    partial: &'a mut Partial,
    local_section: u32,
    region_index: u32,
}

impl Sink<'_> {
    fn section(&self) -> u32 {
        self.local_section
    }

    fn group(&mut self, stream: usize, face: u64, quad_base: usize, quad_count: usize) {
        self.partial.groups.push((
            stream as u32,
            self.region_index,
            Group {
                quad_base: quad_base as u32,
                quad_count: quad_count as u32,
                section: self.local_section | GROUP_FACE.pack(face) as u32,
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
        self.group(pass * 2 + 1, face, base, verts.len() / model::WORDS_PER_QUAD);
        self.partial.complex.extend_from_slice(verts);
    }
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
                    mesh_section(world, catalog, grid, [sx, sy, sz], scratch, &mut partial);
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

fn mesh_section(
    world: &World,
    catalog: &[BlockInfo],
    grid: RegionGrid,
    [sx, sy, sz]: [usize; 3],
    scratch: &mut Scratch,
    partial: &mut Partial,
) {
    scratch.load(
        world,
        catalog,
        [
            (sx * SECTION_SIZE) as i32,
            (sy as i32 + world.min_section[1]) * SECTION_SIZE as i32,
            (sz * SECTION_SIZE) as i32,
        ],
    );

    let (region_index, local_section) = grid.split(sx, sy, sz);
    let faces_at = partial.faces.len() as u32;
    let mut sink = Sink {
        partial,
        local_section,
        region_index: region_index as u32,
    };

    scratch.section_faces.clear();
    fluid::surfaces(scratch);
    cube::greedy(catalog, scratch, &mut sink);
    fluid::greedy(catalog, scratch, &mut sink);

    model::blocks(catalog, scratch, local_section);
    fluid::models(catalog, scratch, local_section);
    model::emit(scratch, &mut sink);

    if !scratch.section_faces.is_empty() {
        partial.faces.extend_from_slice(&scratch.section_faces);
        partial.section_faces.push((local_section, faces_at));
    }
    partial
        .connectivity
        .push((local_section, connectivity::connectivity(&mut scratch.occludes)));
}
