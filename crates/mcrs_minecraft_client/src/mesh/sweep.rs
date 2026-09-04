use crate::blocks::{BlockInfo, FACE_AXES, Pass};
use crate::pack::{
    QUAD_DROP, QUAD_FACE, QUAD_FACE_BASE, QUAD_FLUID, QUAD_H, QUAD_W, QUAD_WORDS, QUAD_X, QUAD_Y,
    QUAD_Z,
};
use mcrs_minecraft_network::columns::SECTION_SIZE;

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
            merge_slice(scratch, face, n);
        }
    }

    for pass in 0..Pass::COUNT {
        let quads = std::mem::take(&mut scratch.simple_by_pass[pass]);
        sink.simple(pass, group_face, &quads);
        scratch.simple_by_pass[pass] = quads;
    }
}

fn merge_slice(scratch: &mut Scratch, face: usize, n: usize) {
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
    QUAD_FACE_BASE.set(&mut words, face_base as u64);
    words
}

#[cfg(test)]
mod tests {
    use super::{pack_quad, quad_anchor};
    use crate::atlas::SpriteRef;
    use crate::blocks::{BlockInfo, CubeFace, Pass};
    use crate::mesh::{Scratch, mesh_world, one_section_world};
    use crate::pack::{
        FACE_ARRAY, FACE_LAYER, QUAD_FACE, QUAD_FACE_BASE, QUAD_H, QUAD_W, QUAD_X, QUAD_Y, QUAD_Z,
    };

    #[test]
    fn the_face_runs_of_a_batch_tile_it_exactly() {
        const TEST_BLOCK: u16 = 37;
        let world = one_section_world(|_, _, _| TEST_BLOCK);
        let mut blocks: Vec<BlockInfo> = (0..=TEST_BLOCK).map(|_| BlockInfo::default()).collect();
        blocks[TEST_BLOCK as usize].cube = Some(
            [CubeFace {
                sprite: SpriteRef { array: 1, layer: 7 },
                pass: Pass::Solid as u8,
                tinted: false,
            }; 6],
        );
        blocks[TEST_BLOCK as usize].occludes = true;

        let mut scratch = Scratch::new();
        let batch = mesh_world(&world, &blocks, &[[0, 0, 0]], &mut scratch);
        let mut runs: Vec<(u64, u64)> = batch
            .simple
            .iter()
            .enumerate()
            .map(|(index, quad)| {
                let slot = batch.quad_section[index] as usize;
                (
                    batch.face_base[slot] as u64 + QUAD_FACE_BASE.read(quad),
                    (QUAD_W.read(quad) + 1) * (QUAD_H.read(quad) + 1),
                )
            })
            .collect();
        runs.sort();
        let mut at = 0u64;
        for (base, len) in runs {
            assert_eq!(
                base, at,
                "a face run does not start where the last one ended"
            );
            at += len;
        }
        assert_eq!(
            at as usize,
            batch.faces.len(),
            "the runs leave the buffer uncovered"
        );
        for attr in &batch.faces {
            assert_eq!(FACE_LAYER.get(*attr as u64), 7, "sprite layer");
            assert_eq!(FACE_ARRAY.get(*attr as u64), 1, "sprite array");
        }
        let quads = batch.simple.len();
        assert_eq!(
            quads, 6,
            "a lone solid section is six merged faces, one per side"
        );
    }

    #[test]
    fn a_packed_quad_round_trips_every_field() {
        let words = pack_quad(3, 9, 3, 7, 12, 16, 24_575, 0, false);
        let anchor = quad_anchor(3, 9, 3, 7);
        assert_eq!(QUAD_X.read(&words), anchor[0] as u64, "x");
        assert_eq!(QUAD_Y.read(&words), anchor[1] as u64, "y");
        assert_eq!(QUAD_Z.read(&words), anchor[2] as u64, "z");
        assert_eq!(QUAD_FACE.read(&words), 3, "face");
        assert_eq!(QUAD_W.read(&words) + 1, 12, "w");
        assert_eq!(QUAD_H.read(&words) + 1, 16, "h");
        assert_eq!(QUAD_FACE_BASE.read(&words), 24_575, "face base");
    }
}
