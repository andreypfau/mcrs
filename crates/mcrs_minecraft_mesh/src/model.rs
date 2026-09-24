use crate::SECTION_SIZE;
use crate::ambient;
use crate::block::{BlockInfo, Pass};
use crate::pack::{
    FACE_NONE, MODEL_BLOCK_LIGHT, MODEL_OVERHANG, MODEL_SHADE, MODEL_SKY_LIGHT, MODEL_SPRITE,
    MODEL_STEPS, MODEL_TINT, MODEL_TINT_HIGH, MODEL_U, MODEL_V, MODEL_X, MODEL_Y, MODEL_Z,
};

use super::Sink;
use super::face_normal;
use super::scratch::{FACE_GROUPS, Scratch, border_index};

pub const WORDS_PER_QUAD: usize = 3 * 4;

pub(super) const UNGROUPED: usize = FACE_GROUPS - 1;

pub(super) const SHADE_DOWN: u8 = 127;
pub(super) const SHADE_EAST_WEST: u8 = 153;
pub(super) const SHADE_NORTH_SOUTH: u8 = 204;
pub(super) const SHADE_UP: u8 = 255;

pub(super) struct Quad {
    pub positions: [[f32; 3]; 4],
    pub uvs: [[f32; 2]; 4],
    pub shade: [u8; 4],
    pub light: [u32; 4],
    pub tint: u32,
    pub sprite: u16,
}

pub(super) fn push(out: &mut Vec<u32>, quad: &Quad) {
    let scale = MODEL_U.max() as f32;
    for corner in 0..4 {
        let mut words = [0u32; 3];
        MODEL_X.set(&mut words, fixed(quad.positions[corner][0]) as u64);
        MODEL_Y.set(&mut words, fixed(quad.positions[corner][1]) as u64);
        MODEL_Z.set(&mut words, fixed(quad.positions[corner][2]) as u64);
        MODEL_U.set(
            &mut words,
            (quad.uvs[corner][0].clamp(0.0, 1.0) * scale) as u64,
        );
        MODEL_V.set(
            &mut words,
            (quad.uvs[corner][1].clamp(0.0, 1.0) * scale) as u64,
        );
        MODEL_TINT.set(&mut words, quad.tint as u64 & MODEL_TINT.max());
        MODEL_TINT_HIGH.set(&mut words, quad.tint as u64 >> MODEL_TINT.bits);
        MODEL_BLOCK_LIGHT.set(&mut words, ambient::block_light(quad.light[corner]) as u64);
        MODEL_SKY_LIGHT.set(&mut words, ambient::sky_light(quad.light[corner]) as u64);
        MODEL_SHADE.set(&mut words, quad.shade[corner] as u64);
        MODEL_SPRITE.set(&mut words, quad.sprite as u64);
        out.extend_from_slice(&words);
    }
}

pub(super) fn blocks(catalog: &[BlockInfo], scratch: &mut Scratch) {
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
                    let (shade, light) = if info.ambient_occlusion {
                        let lit = ambient::smooth(
                            &quad.positions,
                            quad.facing,
                            info.neighbour.full_block,
                            scratch.coords[here],
                            |o| scratch.sample(x as i32 + o.x, y as i32 + o.y, z as i32 + o.z),
                        );
                        (
                            lit.shade.map(|ao| ambient::shade_byte(ao, quad.shade)),
                            lit.light,
                        )
                    } else {
                        if quad.cull.is_none()
                            && ambient::face_cubic(
                                &quad.positions,
                                quad.facing,
                                info.neighbour.full_block,
                            )
                        {
                            let normal = face_normal(quad.facing as usize);
                            sample = border_index(
                                x as i32 + normal[0],
                                y as i32 + normal[1],
                                z as i32 + normal[2],
                            );
                        }
                        let light = ambient::light_coords(
                            info.emissive,
                            info.emission,
                            scratch.light[sample],
                        );
                        ([ambient::shade_byte(1.0, quad.shade); 4], [light; 4])
                    };
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
                            shade,
                            light,
                            tint: quad.tint.index(),
                            sprite: quad.sprite,
                        },
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
pub(super) fn fixed(value: f32) -> u32 {
    ((value + MODEL_OVERHANG) * MODEL_STEPS)
        .round()
        .clamp(0.0, MODEL_X.max() as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::fixed;
    use crate::ambient::{Neighbour, corner};
    use crate::block::{BlockInfo, ModelQuad, Pass};
    use crate::pack::{MODEL_OVERHANG, MODEL_SHADE, MODEL_STEPS};
    use crate::{SECTION_SIZE, SECTION_VOLUME};
    use crate::{Scratch, mesh_world, one_section_world};
    use bevy_math::Vec3;
    use mcrs_minecraft_core::Direction;

    #[test]
    fn the_model_mesher_names_blocks_in_the_worlds_numbering() {
        const TEST_BLOCK: u16 = 37;
        let world = one_section_world(|_, _, _| TEST_BLOCK);

        let mut blocks: Vec<BlockInfo> = (0..=TEST_BLOCK).map(|_| BlockInfo::default()).collect();
        blocks[TEST_BLOCK as usize].quads = vec![ModelQuad {
            positions: [Vec3::ZERO; 4],
            uvs: [[0.0; 2]; 4],
            cull: None,
            facing: Direction::Up,
            face: None,
            sprite: 0,
            pass: Pass::Solid,
            shade: 1.0,
            tint: crate::tint::Tint::None,
        }];

        let mut scratch = Scratch::new();
        let quads = mesh_world(&world, &blocks, &[[0, 0, 0]], &mut scratch).model_quads();
        assert_eq!(
            quads, SECTION_VOLUME,
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

    fn west_face_on_a_floor(ambient_occlusion: bool) -> [u64; 4] {
        const FLOOR: u16 = 1;
        const BLOCK: u16 = 2;
        let world = one_section_world(|x, y, z| match (x, y, z) {
            (8, 1, 8) => BLOCK,
            (6..=10, 0, 6..=10) => FLOOR,
            _ => 0,
        });
        let mut catalog: Vec<BlockInfo> = (0..3).map(|_| BlockInfo::default()).collect();
        let solid = Neighbour {
            full_block: true,
            solid_render: true,
            light_opaque: true,
        };
        catalog[FLOOR as usize].neighbour = solid;
        catalog[BLOCK as usize].neighbour = solid;
        catalog[BLOCK as usize].ambient_occlusion = ambient_occlusion;
        catalog[BLOCK as usize].quads = vec![ModelQuad {
            positions: std::array::from_fn(|i| corner(Direction::West, i, Vec3::ZERO, Vec3::ONE)),
            uvs: [[0.0; 2]; 4],
            cull: None,
            facing: Direction::West,
            face: None,
            sprite: 0,
            pass: Pass::Solid,
            shade: 0.6,
            tint: crate::tint::Tint::None,
        }];

        let batch = mesh_world(&world, &catalog, &[[0, 0, 0]], &mut Scratch::new());
        assert_eq!(batch.model_quads(), 1);
        std::array::from_fn(|corner| MODEL_SHADE.read(&batch.complex[corner * 3..corner * 3 + 3]))
    }

    #[test]
    fn a_model_face_is_occluded_by_the_blocks_around_it_in_the_section() {
        assert_eq!(west_face_on_a_floor(true), [153, 91, 91, 153]);
    }

    #[test]
    fn a_model_without_ambient_occlusion_keeps_only_its_face_shade() {
        assert_eq!(west_face_on_a_floor(false), [153; 4]);
    }
}
