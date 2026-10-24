use crate::block::{BlockInfo, face_axes};
use crate::pack::{
    FACE_WORDS, QUAD_DROP, QUAD_FACE, QUAD_FACE_BASE, QUAD_FLUID, QUAD_H, QUAD_W, QUAD_WORDS,
    QUAD_X, QUAD_Y, QUAD_Z,
};
use crate::{SECTION_SIZE, axis_mut};

use super::Sink;
use super::scratch::{Columns, Scratch, column_index, face_axis};

pub(super) const PASS_KEY_BITS: u8 = 2;
pub(super) const PASS_KEY: u8 = (1 << PASS_KEY_BITS) - 1;
pub(super) const FLUID_KEY: u8 = 1 << 7;

pub(super) type FaceAttr =
    fn(&[BlockInfo], &Scratch, [i32; 3], usize) -> Option<(u8, [u32; FACE_WORDS])>;

pub(super) fn sweep(
    catalog: &[BlockInfo],
    scratch: &mut Scratch,
    sink: &mut Sink,
    columns: Columns,
    face: usize,
    group_face: u64,
    attr: FaceAttr,
) {
    for quads in &mut scratch.simple_by_pass {
        quads.clear();
    }
    let [n_axis, n_positive, ..] = face_axes(face);
    let n_positive = n_positive == 1;

    let mut occupied = 0u32;
    for gv in 0..SECTION_SIZE {
        for gu in 0..SECTION_SIZE {
            let mut local = [0i32; 3];
            face_axis(face, &mut local, gu, gv);
            let column = column_index(n_axis as usize, local[0], local[1], local[2]);
            let visible = scratch.visible(columns, n_axis as usize, column, n_positive);
            if let Some(faces) = scratch.faces.get_mut(gv * SECTION_SIZE + gu) {
                *faces = visible;
            }
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
                let shown = scratch
                    .faces
                    .get(slot)
                    .is_some_and(|faces| faces & bit != 0);
                let found = if shown {
                    let mut local = [0i32; 3];
                    face_axis(face, &mut local, gu, gv);
                    *axis_mut(&mut local, n_axis) = n as i32;
                    attr(catalog, scratch, local, face)
                } else {
                    None
                };
                let (Some(used), Some(pass), Some(attrs)) = (
                    scratch.used.get_mut(slot),
                    scratch.passes.get_mut(slot),
                    scratch.attrs.get_mut(slot),
                ) else {
                    continue;
                };
                match found {
                    Some((key, packed)) => {
                        *used = false;
                        *pass = key;
                        *attrs = packed;
                        any = true;
                    }
                    None => *used = true,
                }
            }
        }
        if any {
            merge_slice(scratch, face, n);
        }
    }

    for (pass, quads) in scratch.simple_by_pass.iter().enumerate() {
        sink.simple(pass, group_face, quads);
    }
}

fn merge_slice(scratch: &mut Scratch, face: usize, n: usize) {
    for gv in 0..SECTION_SIZE {
        let mut gu = 0usize;
        while gu < SECTION_SIZE {
            let slot = gv * SECTION_SIZE + gu;
            let Some(key) = scratch.open(slot) else {
                gu += 1;
                continue;
            };
            let mut w = 1;
            while gu + w < SECTION_SIZE && scratch.open(slot + w) == Some(key) {
                w += 1;
            }
            // A side cut down by a drop is cut at the top of its own cell, so only one row of
            // them can share a quad.
            let dropped_side = face >= 2 && (key & !FLUID_KEY) >> PASS_KEY_BITS != 0;
            let mut h = 1;
            'grow: while !dropped_side && gv + h < SECTION_SIZE {
                for i in 0..w {
                    if scratch.open((gv + h) * SECTION_SIZE + gu + i) != Some(key) {
                        break 'grow;
                    }
                }
                h += 1;
            }
            let base = scratch.section_faces.len() as u32;
            for dv in 0..h {
                let row = (gv + dv) * SECTION_SIZE + gu;
                if let (Some(used), Some(attrs)) = (
                    scratch.used.get_mut(row..row + w),
                    scratch.attrs.get(row..row + w),
                ) {
                    used.fill(true);
                    scratch.section_faces.extend_from_slice(attrs);
                }
            }
            if let Some(quads) = scratch.simple_by_pass.get_mut((key & PASS_KEY) as usize) {
                quads.push(pack_quad(
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
            }
            gu += w;
        }
    }
}

#[inline]
fn quad_anchor(face: usize, n: usize, gu: usize, gv: usize) -> [i32; 3] {
    let [n_axis, n_positive, u_axis, u_positive, v_axis, v_positive] = face_axes(face);
    let mut local = [0i32; 3];
    *axis_mut(&mut local, n_axis) = n as i32 + if n_positive == 1 { 1 } else { 0 };
    *axis_mut(&mut local, u_axis) = if u_positive == 1 {
        gu as i32
    } else {
        SECTION_SIZE as i32 - gu as i32
    };
    *axis_mut(&mut local, v_axis) = if v_positive == 1 {
        gv as i32
    } else {
        SECTION_SIZE as i32 - gv as i32
    };
    local
}

#[expect(
    clippy::too_many_arguments,
    reason = "the face, slice, grid position, size, base, drop and fluid flag that are the fields of one packed quad"
)]
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
    use crate::block::{BlockInfo, CubeFace, Pass};
    use crate::pack::{
        FACE_SPRITE, QUAD_FACE, QUAD_FACE_BASE, QUAD_H, QUAD_W, QUAD_X, QUAD_Y, QUAD_Z,
    };
    use crate::{Scratch, mesh_world, one_section_world};

    #[test]
    fn the_face_runs_of_a_batch_tile_it_exactly() {
        const TEST_BLOCK: u16 = 37;
        let world = one_section_world(|_, _, _| TEST_BLOCK);
        let mut blocks: Vec<BlockInfo> = (0..=TEST_BLOCK).map(|_| BlockInfo::default()).collect();
        blocks[TEST_BLOCK as usize].cube = Some(
            [CubeFace {
                sprite: 263,
                pass: Pass::Solid as u8,
                tint: crate::tint::Tint::None,
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
            assert_eq!(FACE_SPRITE.read(attr), 263, "sprite");
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
