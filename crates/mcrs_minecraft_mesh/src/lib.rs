#![cfg_attr(
    test,
    expect(
        clippy::disallowed_methods,
        reason = "a test rewrites the generated shader header when asked through the environment"
    )
)]

pub mod arena;
pub mod block;
mod connectivity;
mod cube;
mod draw_kind;
pub mod face_shade;
mod fluid;
mod group;
mod model;
pub mod pack;
mod pending_group;
mod quad_format;
mod scratch;
mod section_draw;
pub mod smooth_light;
pub mod state_variants;
mod sweep;
pub mod tint;
pub mod uv_orientation;
pub mod variant_pick;

use crate::block::{BlockInfo, Pass, face_axes};
use crate::pack::{
    BOUNDS_HI_X, BOUNDS_HI_Y, BOUNDS_HI_Z, BOUNDS_LO_X, BOUNDS_LO_Y, BOUNDS_LO_Z, FACE_WORDS,
    Field, MODEL_OVERHANG, MODEL_STEPS, MODEL_X, MODEL_Y, MODEL_Z, QUAD_DROP, QUAD_FACE,
    QUAD_FLUID, QUAD_H, QUAD_W, QUAD_WORDS, QUAD_X, QUAD_Y, QUAD_Z,
};
use crate::pending_group::PendingGroup;
use crate::smooth_light::SmoothLight;
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_volume::{BlockVolume, LightVolume};

pub use connectivity::{CONNECT_ALL, Connectivity, OPEN, SEALED, along};
pub use draw_kind::DrawKind;
pub use group::Group;
pub use quad_format::QuadFormat;
pub use scratch::Scratch;
pub use section_draw::SectionDraw;

pub const SECTION_SIZE: usize = mcrs_voxel::SectionPos::SIZE;
pub const SECTION_VOLUME: usize = mcrs_voxel::SectionPos::VOLUME;

pub const GROUP_QUADS: usize = 32;

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct Draw {
    pub kind: DrawKind,
    pub first_group: u32,
    pub group_count: u32,
    pub quad_count: u32,
}

pub struct SectionMesh {
    pub section: [i32; 3],
    pub greedy: Vec<[u32; QUAD_WORDS]>,
    pub faces: Vec<[u32; FACE_WORDS]>,
    pub models: Vec<u32>,
    pub groups: Vec<Group>,
    pub draws: [SectionDraw; DrawKind::COUNT],
    pub connectivity: Connectivity,
}

impl SectionMesh {
    pub fn model_quads(&self) -> usize {
        self.models.len() / model::WORDS_PER_QUAD
    }
}

#[inline]
pub const fn face_normal(face: usize) -> [i32; 3] {
    let [axis, positive, ..] = face_axes(face);
    let sign = if positive == 1 { 1 } else { -1 };
    match axis {
        0 => [sign, 0, 0],
        1 => [0, sign, 0],
        _ => [0, 0, sign],
    }
}

#[inline]
fn axis_mut<T>(point: &mut [T; 3], axis: u8) -> &mut T {
    let [x, y, z] = point;
    match axis {
        0 => x,
        1 => y,
        _ => z,
    }
}

struct Sink {
    greedy: Vec<[u32; QUAD_WORDS]>,
    models: Vec<u32>,
    groups: Vec<PendingGroup>,
    slot: u32,
}

impl Sink {
    fn group(
        &mut self,
        kind: DrawKind,
        face: u64,
        quad_base: usize,
        quad_count: usize,
        bounds: u32,
    ) {
        self.groups.push(PendingGroup {
            kind,
            group: Group {
                quad_base: quad_base as u32,
                quad_count: quad_count as u32,
                section: self.slot,
                face: face as u32,
                bounds,
            },
        });
    }

    fn greedy(&mut self, pass: Pass, face: u64, quads: &[[u32; QUAD_WORDS]]) {
        let kind = DrawKind::new(pass, QuadFormat::Greedy);
        for run in quads.chunks(GROUP_QUADS) {
            let base = self.greedy.len();
            let mut bounds = Bounds::EMPTY;
            for quad in run {
                bounds.cover_greedy(quad[0]);
            }
            self.group(kind, face, base, run.len(), bounds.pack());
            self.greedy.extend_from_slice(run);
        }
    }

    fn model(&mut self, pass: Pass, face: u64, verts: &[u32]) {
        let kind = DrawKind::new(pass, QuadFormat::Model);
        for run in verts.chunks(GROUP_QUADS * model::WORDS_PER_QUAD) {
            let base = self.models.len() / model::WORDS_PER_QUAD;
            let mut bounds = Bounds::EMPTY;
            for vertex in run.chunks(model::WORDS_PER_QUAD / 4) {
                bounds.cover_model(vertex);
            }
            self.group(
                kind,
                face,
                base,
                run.len() / model::WORDS_PER_QUAD,
                bounds.pack(),
            );
            self.models.extend_from_slice(run);
        }
    }
}

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
        for ((lo, hi), at) in self.lo.iter_mut().zip(&mut self.hi).zip(point) {
            *lo = lo.min(at);
            *hi = hi.max(at);
        }
    }

    fn cover_greedy(&mut self, word: u32) {
        let word = word as u64;
        let corner = [QUAD_X, QUAD_Y, QUAD_Z].map(|field| field.get(word) as f32);
        let [normal, _, u_axis, u_positive, v_axis, v_positive] =
            face_axes(QUAD_FACE.get(word) as usize);
        let along = |axis: u8, positive: u8, size: Field| {
            let size = (size.get(word) + 1) as f32;
            let mut step = [0.0; 3];
            *axis_mut(&mut step, axis) = if positive == 1 { size } else { -size };
            step
        };
        let u = along(u_axis, u_positive, QUAD_W);
        let v = along(v_axis, v_positive, QUAD_H);
        let mut far = corner;
        for ((at, u), v) in far.iter_mut().zip(u).zip(v) {
            *at = *at + u + v;
        }
        self.cover(corner);
        self.cover(far);
        if QUAD_DROP.get(word) != 0 || QUAD_FLUID.get(word) != 0 {
            let mut below = corner;
            *axis_mut(&mut below, normal) -= 1.0;
            let mut above = corner;
            *axis_mut(&mut above, normal) += 1.0;
            self.cover(below);
            self.cover(above);
        }
    }

    fn cover_model(&mut self, vertex: &[u32]) {
        self.cover([MODEL_X, MODEL_Y, MODEL_Z].map(|field| {
            let word = vertex.get(field.word as usize).copied().unwrap_or(0);
            field.get(word as u64) as f32 / MODEL_STEPS - MODEL_OVERHANG
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
    world: &(impl BlockVolume + LightVolume),
    catalog: &[BlockInfo],
    section: [i32; 3],
    slot: u32,
    smooth_light: SmoothLight,
    shade: CardinalLight,
    scratch: &mut Scratch,
) -> SectionMesh {
    scratch.smooth_light = smooth_light;
    scratch.shade = shade;
    scratch.load(world, catalog, section.map(|n| n * SECTION_SIZE as i32));

    let mut sink = Sink {
        greedy: Vec::new(),
        models: Vec::new(),
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
    let mut draws = [SectionDraw::default(); DrawKind::COUNT];
    for (&kind, draw) in DrawKind::ALL.iter().zip(&mut draws) {
        let first = groups.len();
        let mut quads = 0u32;
        for &PendingGroup { kind: from, group } in &sink.groups {
            if from == kind {
                quads += group.quad_count;
                groups.push(group);
            }
        }
        *draw = SectionDraw {
            group_count: (groups.len() - first) as u32,
            quad_count: quads,
        };
    }

    SectionMesh {
        section,
        greedy: sink.greedy,
        faces: std::mem::take(&mut scratch.section_faces),
        models: sink.models,
        groups,
        draws,
        connectivity: connectivity::connectivity(&scratch.occludes),
    }
}

#[cfg(test)]
pub fn one_section_world(pick: impl Fn(usize, usize, usize) -> u16) -> OneSection {
    let mut blocks = Box::new([mcrs_voxel::BlockStateId::AIR; SECTION_VOLUME]);
    for y in 0..SECTION_SIZE {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                let state = mcrs_voxel::BlockStateId::checked(pick(x, y, z), 1 << 16);
                blocks[(y * SECTION_SIZE + z) * SECTION_SIZE + x] =
                    state.unwrap_or(mcrs_voxel::BlockStateId::AIR);
            }
        }
    }
    OneSection(blocks)
}

#[cfg(test)]
pub struct OneSection(Box<[mcrs_voxel::BlockStateId; SECTION_VOLUME]>);

#[cfg(test)]
impl mcrs_minecraft_volume::Volume for OneSection {
    fn bounds(&self) -> mcrs_minecraft_core::BoundingBox {
        mcrs_minecraft_core::BoundingBox::of_section(mcrs_voxel::SectionPos::new(0, 0, 0))
    }

    fn access(&self, at: mcrs_voxel::SectionPos) -> mcrs_minecraft_volume::Access {
        if at == mcrs_voxel::SectionPos::new(0, 0, 0) {
            mcrs_minecraft_volume::Access::Read
        } else {
            mcrs_minecraft_volume::Access::Outside
        }
    }
}

#[cfg(test)]
impl BlockVolume for OneSection {
    type Section<'a> = mcrs_minecraft_volume::DenseSection<'a>;

    fn section(
        &self,
        at: mcrs_voxel::SectionPos,
    ) -> Result<Self::Section<'_>, mcrs_minecraft_volume::Unavailable> {
        mcrs_minecraft_volume::Volume::access(self, at).check_read()?;
        Ok(mcrs_minecraft_volume::DenseSection(&self.0))
    }
}

#[cfg(test)]
impl LightVolume for OneSection {
    fn light(&self, at: mcrs_voxel::BlockPos) -> Result<u8, mcrs_minecraft_volume::Unavailable> {
        let at = mcrs_voxel::SectionPos::from(at);
        Ok(if at.x != 0 || at.z != 0 || at.y > 1 {
            0x0f
        } else {
            0
        })
    }
}

#[cfg(test)]
pub struct Batch {
    pub greedy: Vec<[u32; QUAD_WORDS]>,
    pub faces: Vec<[u32; FACE_WORDS]>,
    pub face_base: Vec<u32>,
    pub quad_section: Vec<u32>,
    pub models: Vec<u32>,
}

#[cfg(test)]
impl Batch {
    pub fn model_quads(&self) -> usize {
        self.models.len() / model::WORDS_PER_QUAD
    }
}

#[cfg(test)]
pub fn mesh_world(
    world: &(impl BlockVolume + LightVolume),
    catalog: &[BlockInfo],
    sections: &[[i32; 3]],
    scratch: &mut Scratch,
) -> Batch {
    let mut batch = Batch {
        greedy: Vec::new(),
        faces: Vec::new(),
        face_base: Vec::new(),
        quad_section: Vec::new(),
        models: Vec::new(),
    };
    for &section in sections {
        let slot = batch.face_base.len() as u32;
        let mesh = mesh_section(
            world,
            catalog,
            section,
            slot,
            SmoothLight::On,
            CardinalLight::Default,
            scratch,
        );
        batch.greedy.extend_from_slice(&mesh.greedy);
        batch.quad_section.resize(batch.greedy.len(), slot);
        batch.models.extend_from_slice(&mesh.models);
        batch.face_base.push(batch.faces.len() as u32);
        batch.faces.extend_from_slice(&mesh.faces);
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{CubeFace, ModelQuad};
    use glam::Vec3;

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
                emission: 0,
                uv: crate::uv_orientation::UvOrientation::IDENTITY,
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
            shade: crate::face_shade::FaceShade::Factor(1.0),
            tint: crate::tint::Tint::None,
            emission: 0,
        }];

        let subject = one_section_world(|x, y, z| match (x + y + z) % 4 {
            0 => STONE,
            1 => BUSH,
            _ => 0,
        });
        let other = one_section_world(|x, _, z| if x > z { BUSH } else { STONE });

        let fresh = mesh_section(
            &subject,
            &catalog,
            [0, 0, 0],
            0,
            SmoothLight::On,
            CardinalLight::Default,
            &mut Scratch::new(),
        );

        let mut carried = Scratch::new();
        mesh_section(
            &other,
            &catalog,
            [0, 0, 0],
            0,
            SmoothLight::On,
            CardinalLight::Default,
            &mut carried,
        );
        let again = mesh_section(
            &subject,
            &catalog,
            [0, 0, 0],
            0,
            SmoothLight::On,
            CardinalLight::Default,
            &mut carried,
        );

        assert_eq!(fresh.faces, again.faces, "face attributes");
        assert_eq!(fresh.greedy, again.greedy, "greedy quads");
        assert_eq!(fresh.models, again.models, "model vertices");
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
                emission: 0,
                uv: crate::uv_orientation::UvOrientation::IDENTITY,
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
            shade: crate::face_shade::FaceShade::Factor(1.0),
            tint: crate::tint::Tint::None,
            emission: 0,
        }];

        for (at, state) in [([3, 4, 5], STONE), ([15, 0, 9], STONE), ([7, 12, 0], BUSH)] {
            let world = one_section_world(|x, y, z| if [x, y, z] == at { state } else { 0 });
            let mesh = mesh_section(
                &world,
                &catalog,
                [0, 0, 0],
                0,
                SmoothLight::On,
                CardinalLight::Default,
                &mut Scratch::new(),
            );
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
            shade: crate::face_shade::FaceShade::Factor(1.0),
            tint: crate::tint::Tint::None,
            emission: 0,
        }];
        let world = one_section_world(|_, y, _| if y == 0 { BUSH } else { 0 });
        let mesh = mesh_section(
            &world,
            &catalog,
            [0, 0, 0],
            0,
            SmoothLight::On,
            CardinalLight::Default,
            &mut Scratch::new(),
        );
        assert_eq!(mesh.model_quads(), SECTION_SIZE * SECTION_SIZE);
        assert!(
            mesh.groups
                .iter()
                .all(|group| group.quad_count as usize <= GROUP_QUADS)
        );
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
            emission: 0,
            uv: crate::uv_orientation::UvOrientation::IDENTITY,
        };
        let mut catalog: Vec<BlockInfo> = (0..3).map(|_| BlockInfo::default()).collect();
        catalog[STONE as usize].cube = Some([face(1); 6]);
        catalog[STONE as usize].occludes = true;
        catalog[STONE as usize].faces = Some(Box::new(FaceShapes {
            outer: [[u16::MAX; 16]; 6],
            inner: [[u16::MAX; 16]; 6],
        }));
        let mut sides = [[0u16; 16]; 6];
        sides[0] = [u16::MAX; 16];
        for (side, rows) in sides.iter_mut().enumerate().skip(2) {
            let rows_are_height = side >= 4;
            for (row, covered) in rows.iter_mut().enumerate() {
                *covered = if rows_are_height {
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
        let mesh = mesh_section(
            &world,
            &catalog,
            [0, 0, 0],
            0,
            SmoothLight::On,
            CardinalLight::Default,
            &mut Scratch::new(),
        );
        let tops: Vec<_> = mesh
            .greedy
            .iter()
            .filter(|quad| QUAD_FACE.read(*quad) == 1)
            .collect();
        assert_eq!(
            tops.len(),
            1,
            "the snow tops merge and the stone under them is hidden"
        );
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
                emission: 0,
                uv: crate::uv_orientation::UvOrientation::IDENTITY,
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
            shade: crate::face_shade::FaceShade::Factor(1.0),
            tint: crate::tint::Tint::None,
            emission: 0,
        }];

        let mut scratch = Scratch::new();
        let mesh = mesh_section(
            &world,
            &catalog,
            [0, 0, 0],
            0,
            SmoothLight::On,
            CardinalLight::Default,
            &mut scratch,
        );

        assert!(
            mesh.draws[0].quad_count > 0 && mesh.draws[3].quad_count > 0,
            "the fixture has to reach both a greedy and a model bucket"
        );
        let mut at = 0usize;
        for (draw, &kind) in mesh.draws.iter().zip(&DrawKind::ALL) {
            let run = draw.group_count as usize;
            let held = &mesh.groups[at..at + run];
            at += run;
            let mut quads = 0u32;
            let mut covered = 0u32;
            for group in held {
                assert_eq!(group.quad_base, covered, "{} leaves a hole", kind.label());
                quads += group.quad_count;
                covered += group.quad_count;
            }
            assert_eq!(quads, draw.quad_count);
            let total = match kind.format {
                QuadFormat::Model => mesh.model_quads(),
                QuadFormat::Greedy => mesh.greedy.len(),
            };
            assert!(
                covered as usize <= total,
                "{} names more quads than the section holds",
                kind.label()
            );
        }
        assert_eq!(at, mesh.groups.len(), "a group belongs to no bucket");
    }
}
