use crate::anvil::SECTION_SIZE;
use crate::blocks::{BlockInfo, CORNER_UV, FACE_AXES};
use crate::pack::{FACE_AO, FACE_ARRAY, FACE_BLOCK_LIGHT, FACE_LAYER, FACE_SKY_LIGHT, FACE_TINT};

use super::scratch::{Columns, Scratch, border_index};
use super::sweep::sweep;
use super::{Sink, face_normal};

pub(super) fn greedy(catalog: &[BlockInfo], scratch: &mut Scratch, sink: &mut Sink) {
    for face in 0..6usize {
        sweep(
            catalog,
            scratch,
            sink,
            Columns::Cubes,
            face,
            face as u64,
            face_attr,
        );
    }
}

fn face_attr(
    catalog: &[BlockInfo],
    scratch: &Scratch,
    local: [i32; 3],
    face: usize,
) -> Option<(u8, u32)> {
    let here = scratch.states[border_index(local[0], local[1], local[2])];
    let info = &catalog[here as usize];
    let cube = info.cube.as_ref()?;
    let normal = face_normal(face);
    let front = [
        local[0] + normal[0],
        local[1] + normal[1],
        local[2] + normal[2],
    ];
    let front_index = border_index(front[0], front[1], front[2]);
    if scratch.occludes[front_index] {
        return None;
    }
    if info.self_culls && scratch.states[front_index] == here {
        return None;
    }

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
    let mut words = [0u32; 1];
    FACE_LAYER.set(&mut words, cube.sprite.layer as u64);
    FACE_ARRAY.set(&mut words, cube.sprite.array as u64);
    if cube.tinted {
        FACE_TINT.set(&mut words, info.tint_kind as u64 + 1);
    }
    FACE_BLOCK_LIGHT.set(&mut words, (raw >> 4).max(info.emission as u32) as u64);
    FACE_SKY_LIGHT.set(&mut words, (raw & 0xf) as u64);
    FACE_AO.set(&mut words, ao as u64);
    Some((cube.pass, words[0]))
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
