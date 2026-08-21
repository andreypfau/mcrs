use crate::anvil::SECTION_SIZE;
use crate::atlas::SpriteRef;
use crate::blocks::{BlockInfo, Pass};
use crate::pack::{
    FACE_NONE, MODEL_ARRAY, MODEL_BLOCK_LIGHT, MODEL_LAYER, MODEL_OVERHANG, MODEL_SECTION,
    MODEL_SHADE, MODEL_SKY_LIGHT, MODEL_STEPS, MODEL_TINT, MODEL_U, MODEL_V, MODEL_X, MODEL_Y,
    MODEL_Z,
};

use super::Sink;
use super::scratch::{FACE_GROUPS, Scratch, border_index};
use super::face_normal;

pub const WORDS_PER_QUAD: usize = 3 * 4;

pub(super) const UNGROUPED: usize = FACE_GROUPS - 1;

pub(super) const SHADE_DOWN: u32 = 0;
pub(super) const SHADE_EAST_WEST: u32 = 1;
pub(super) const SHADE_NORTH_SOUTH: u32 = 2;
pub(super) const SHADE_UP: u32 = 3;

pub(super) struct Quad {
    pub positions: [[f32; 3]; 4],
    pub uvs: [[f32; 2]; 4],
    pub shade: [u32; 4],
    pub light: (u32, u32),
    pub tint: u32,
    pub sprite: SpriteRef,
}

pub(super) fn push(out: &mut Vec<u32>, quad: &Quad, local_section: u32) {
    let scale = MODEL_U.max() as f32;
    for corner in 0..4 {
        let mut words = [0u32; 3];
        MODEL_X.set(&mut words, fixed(quad.positions[corner][0]) as u64);
        MODEL_Y.set(&mut words, fixed(quad.positions[corner][1]) as u64);
        MODEL_Z.set(&mut words, fixed(quad.positions[corner][2]) as u64);
        MODEL_U.set(&mut words, (quad.uvs[corner][0].clamp(0.0, 1.0) * scale) as u64);
        MODEL_V.set(&mut words, (quad.uvs[corner][1].clamp(0.0, 1.0) * scale) as u64);
        MODEL_TINT.set(&mut words, quad.tint as u64);
        MODEL_BLOCK_LIGHT.set(&mut words, quad.light.0 as u64);
        MODEL_SKY_LIGHT.set(&mut words, quad.light.1 as u64);
        MODEL_SHADE.set(&mut words, quad.shade[corner] as u64);
        MODEL_SECTION.set(&mut words, local_section as u64);
        MODEL_ARRAY.set(&mut words, quad.sprite.array as u64);
        MODEL_LAYER.set(&mut words, quad.sprite.layer as u64);
        out.extend_from_slice(&words);
    }
}

pub(super) fn blocks(catalog: &[BlockInfo], scratch: &mut Scratch, local_section: u32) {
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
                    let group = quad.face.map_or(UNGROUPED, |group| group as usize);
                    let offset = [x as f32, y as f32, z as f32];
                    push(
                        &mut scratch.complex_by_pass[quad.pass as usize][group],
                        &Quad {
                            positions: std::array::from_fn(|corner| {
                                let p = quad.positions[corner];
                                [p.x + offset[0], p.y + offset[1], p.z + offset[2]]
                            }),
                            uvs: quad.uvs,
                            shade: quad.shade.map(shade_bucket),
                            light: ((raw >> 4).max(info.emission as u32), raw & 0xf),
                            tint: if quad.tinted {
                                info.tint_kind as u32 + 1
                            } else {
                                0
                            },
                            sprite: quad.sprite,
                        },
                        local_section,
                    );
                }
            }
        }
    }
}

pub(super) fn emit(scratch: &mut Scratch, sink: &mut Sink) {
    for pass in 0..Pass::COUNT {
        for group in 0..FACE_GROUPS {
            let verts = std::mem::take(&mut scratch.complex_by_pass[pass][group]);
            let face = if group == UNGROUPED {
                FACE_NONE as u64
            } else {
                group as u64
            };
            sink.complex(pass, face, &verts);
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
pub(super) fn fixed(value: f32) -> u32 {
    ((value + MODEL_OVERHANG) * MODEL_STEPS)
        .round()
        .clamp(0.0, MODEL_X.max() as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::{BUCKET_SHADES, fixed, shade_bucket};
    use crate::anvil::{Palette, SECTION_SIZE, SECTION_VOLUME, World, one_section_region};
    use crate::atlas::SpriteRef;
    use crate::blocks::{BlockInfo, ModelQuad, Pass};
    use crate::mesh::{Scratch, mesh_render_region};
    use crate::pack::{MODEL_OVERHANG, MODEL_STEPS, RegionGrid};
    use bevy::math::Vec3;

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
            .map(|region| {
                mesh_render_region(&world, &blocks, grid, region, &mut scratch).model_quads()
            })
            .sum();
        assert_eq!(
            quads,
            SECTION_VOLUME,
            "one model quad per block of the one section the fixture fills"
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
    fn a_bucketed_shade_decodes_to_the_shade_it_stood_for() {
        for (byte, shade) in [(127u8, 0.5), (153, 0.6), (204, 0.8), (255, 1.0)] {
            let bucket = shade_bucket(byte) as usize;
            assert_eq!(
                BUCKET_SHADES[bucket], shade,
                "a face baked at {shade} buckets to {bucket}"
            );
        }

        let source = include_str!("../render/shaders/terrain.wgsl");
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
}
