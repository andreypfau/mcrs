use bevy_math::Vec3;
use mcrs_minecraft_core::Direction;

use crate::ambient;
use crate::block::{BlockInfo, face_hidden};
use crate::pack::{
    MODEL_STEPS,
    FACE_AO, FACE_AO_CORNER_BITS, FACE_BLOCK_LIGHT, FACE_LIGHT_CORNER_BITS, FACE_SKY_LIGHT,
    FACE_SPRITE, FACE_TINT, FACE_WORDS,
};

use super::scratch::{Columns, Scratch, border_index};
use super::sweep::{PASS_KEY_BITS, sweep};
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
) -> Option<(u8, [u32; FACE_WORDS])> {
    let here = scratch.states[border_index(local[0], local[1], local[2])];
    let info = &catalog[here as usize];
    let cube = info.cube.as_ref()?;
    let lowered_top = info.drop != 0 && face == Direction::Up as usize;
    if info.drop != 0 && face >= 2 {
        return None;
    }
    let normal = face_normal(face);
    let front = [
        local[0] + normal[0],
        local[1] + normal[1],
        local[2] + normal[2],
    ];
    let front_index = border_index(front[0], front[1], front[2]);
    // A lowered top lies inside its own cell, touches nothing above it and has no cullface.
    if !lowered_top && face_hidden(info, &catalog[scratch.states[front_index] as usize], face) {
        return None;
    }
    if info.self_culls && scratch.states[front_index] == here {
        return None;
    }

    let cube = cube[face];
    let height = 1.0 - info.drop as f32 / MODEL_STEPS;
    let (ao, light) = if info.ambient_occlusion {
        let dir = Direction::all()[face];
        let positions = std::array::from_fn(|i| {
            ambient::corner(dir, i, Vec3::ZERO, Vec3::new(1.0, height, 1.0))
        });
        let lit = ambient::smooth(
            &positions,
            dir,
            info.neighbour.full_block,
            scratch.coords[border_index(local[0], local[1], local[2])],
            |o| scratch.sample(local[0] + o.x, local[1] + o.y, local[2] + o.z),
        );
        (lit.shade.map(ao_code), lit.light)
    } else {
        let lit_from = if lowered_top {
            border_index(local[0], local[1], local[2])
        } else {
            front_index
        };
        let light = ambient::light_coords(info.emissive, info.emission, scratch.light[lit_from]);
        ([0; 4], [light; 4])
    };

    let mut words = [0u32; FACE_WORDS];
    FACE_SPRITE.set(&mut words, cube.sprite as u64);
    FACE_TINT.set(&mut words, cube.tint.index() as u64);
    set_corners(&mut words, ao, light);
    Some((cube.pass | info.drop << PASS_KEY_BITS, words))
}

/// Full cube faces keep vanilla's occlusion byte as its distance below 255 in steps of 51, the
/// only bytes a face lying on its block's boundary can reach.
fn ao_code(ao: f32) -> u32 {
    let byte = ambient::shade_byte(ao, 1.0) as u32;
    debug_assert_eq!((255 - byte) % 51, 0, "{ao} is no full cube occlusion");
    (255 - byte) / 51
}

pub(super) fn set_corners(words: &mut [u32; FACE_WORDS], ao: [u32; 4], light: [u32; 4]) {
    let mut codes = 0u64;
    let mut block = 0u64;
    let mut sky = 0u64;
    for corner in 0..4 {
        let quarter = |units: u32| {
            debug_assert_eq!(
                units % 4,
                0,
                "{units} is finer than a full cube face reaches"
            );
            u64::from(units / 4)
        };
        codes |= u64::from(ao[corner]) << (corner as u32 * FACE_AO_CORNER_BITS);
        block |= quarter(ambient::block_light(light[corner]))
            << (corner as u32 * FACE_LIGHT_CORNER_BITS);
        sky |=
            quarter(ambient::sky_light(light[corner])) << (corner as u32 * FACE_LIGHT_CORNER_BITS);
    }
    FACE_AO.set(words, codes);
    FACE_BLOCK_LIGHT.set(words, block);
    FACE_SKY_LIGHT.set(words, sky);
}
