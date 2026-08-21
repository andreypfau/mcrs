use crate::anvil::SECTION_SIZE;
use crate::blocks::{BlockInfo, FACE_AXES, Pass};
use crate::pack::{
    QUAD_DROP, QUAD_FACE, QUAD_FACE_BASE, QUAD_FLUID, QUAD_H, QUAD_SECTION, QUAD_W, QUAD_WORDS,
    QUAD_X, QUAD_Y, QUAD_Z,
};

use super::Sink;
use super::scratch::{Columns, Scratch, column_index, face_axis};

pub(super) const PASS_KEY_BITS: u8 = 2;
pub(super) const PASS_KEY: u8 = (1 << PASS_KEY_BITS) - 1;
pub(super) const FLUID_KEY: u8 = 1 << 7;

pub(super) type FaceAttr = fn(&[BlockInfo], &Scratch, [i32; 3], usize) -> Option<(u8, u32)>;

pub(super) fn sweep(
    catalog: &[BlockInfo],
    scratch: &mut Scratch,
    sink: &mut Sink,
    columns: Columns,
    face: usize,
    group_face: u64,
    attr: FaceAttr,
) {
    for pass in 0..Pass::COUNT {
        scratch.simple_by_pass[pass].clear();
    }
    let axes = FACE_AXES[face];
    let n_axis = axes[0] as usize;
    let n_positive = axes[1] == 1;

    let mut occupied = 0u32;
    for gv in 0..SECTION_SIZE {
        for gu in 0..SECTION_SIZE {
            let mut local = [0i32; 3];
            face_axis(face, &mut local, gu, gv);
            let column = column_index(n_axis, local[0], local[1], local[2]);
            let visible = scratch.visible(columns, n_axis, column, n_positive);
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
                face_axis(face, &mut local, gu, gv);
                local[n_axis] = n as i32;
                match attr(catalog, scratch, local, face) {
                    Some((key, packed)) => {
                        scratch.used[slot] = false;
                        scratch.passes[slot] = key;
                        scratch.attrs[slot] = packed;
                        any = true;
                    }
                    None => scratch.used[slot] = true,
                }
            }
        }
        if any {
            merge_slice(scratch, face, n, sink.section());
        }
    }

    for pass in 0..Pass::COUNT {
        let quads = std::mem::take(&mut scratch.simple_by_pass[pass]);
        sink.simple(pass, group_face, &quads);
        scratch.simple_by_pass[pass] = quads;
    }
}

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

#[cfg(test)]
mod tests {
    use super::{pack_quad, quad_anchor};
    use crate::anvil::{Palette, World, one_section_region};
    use crate::atlas::SpriteRef;
    use crate::blocks::{BlockInfo, CORNER_UV, CubeFace, FACE_AXES, Pass, cube_corner};
    use crate::mesh::{Scratch, mesh_render_region};
    use crate::pack::{
        FACE_ARRAY, FACE_LAYER, QUAD_FACE, QUAD_FACE_BASE, QUAD_H, QUAD_SECTION, QUAD_W, QUAD_X,
        QUAD_Y, QUAD_Z, RegionGrid, SECTION_FACE_TABLE, pack_section,
    };
    use bevy::math::Vec3;

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
                world[axes[2] as usize] =
                    anchor[axes[2] as usize] as f32 + if axes[3] == 1 { cu } else { -cu };
                world[axes[4] as usize] =
                    anchor[axes[4] as usize] as f32 + if axes[5] == 1 { cv } else { -cv };
                let expected = cube_corner(crate::bake::Dir::ALL[face], corner);
                assert!(
                    Vec3::from(world).distance(expected) < 1e-5,
                    "face {face} corner {corner}: greedy {world:?} vs baked {expected:?}"
                );
            }
        }
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
