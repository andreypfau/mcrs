use glam::Vec3;
use mcrs_minecraft_core::Direction;

use crate::block::{BlockInfo, face_direction, face_hidden};
use crate::pack::{
    FACE_AO, FACE_AO_CORNER_BITS, FACE_BLOCK_LIGHT, FACE_LIGHT_CORNER_BITS, FACE_SKY_LIGHT,
    FACE_SPRITE, FACE_TINT, FACE_UV, FACE_WORDS, MODEL_STEPS,
};
use crate::smooth_light::{self, LightPath, VertexLight};

use super::scratch::{Columns, Scratch, block_info, border_index, cell};
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
    let here_index = border_index(local[0], local[1], local[2]);
    let here = cell(&*scratch.states, here_index);
    let info = block_info(catalog, here);
    let (cube, drop) = info.cube_of(cell(&*scratch.choices, here_index));
    let cube = cube?;
    let lowered_top = drop != 0 && face == Direction::Up as usize;
    if drop != 0 && face >= 2 {
        return None;
    }
    let normal = face_normal(face);
    let front = [
        local[0] + normal[0],
        local[1] + normal[1],
        local[2] + normal[2],
    ];
    let front_index = border_index(front[0], front[1], front[2]);
    let front_state = cell(&*scratch.states, front_index);
    // A lowered top lies inside its own cell, touches nothing above it and has no cullface.
    if !lowered_top && face_hidden(info, block_info(catalog, front_state), face) {
        return None;
    }
    if info.self_culls && front_state == here {
        return None;
    }

    let cube = *cube.get(face)?;
    let height = 1.0 - drop as f32 / MODEL_STEPS;
    let direction = face_direction(face);
    let positions = smooth_light::corners(direction, Vec3::ZERO, Vec3::new(1.0, height, 1.0));
    let path = LightPath::choose(scratch.smooth_light, &info.light, info.ambient_occlusion);
    // A lowered top lies inside its own cell and has no cull face.
    let cull = (!lowered_top).then_some(direction);
    let values = smooth_light::light_quad(path, &positions, direction, cull, |offset| {
        scratch.sample(
            local[0] + offset.x,
            local[1] + offset.y,
            local[2] + offset.z,
        )
    });
    let ao = values.occlusion.map(ao_code);
    let light = values.light.map(|light| light.with_emission(cube.emission));

    let mut words = [0u32; FACE_WORDS];
    FACE_SPRITE.set(&mut words, cube.sprite as u64);
    FACE_TINT.set(&mut words, cube.tint.index() as u64);
    FACE_UV.set(&mut words, u64::from(cube.uv.bits()));
    set_corners(&mut words, ao, light);
    Some((cube.pass | drop << PASS_KEY_BITS, words))
}

/// A cube face keeps its occlusion byte as its distance below 255 in steps of 51: a corner shade
/// is the mean of four occlusion shades of `0.2` or `1.0`, so no other byte occurs.
fn ao_code(occlusion: f32) -> u32 {
    let byte = u32::from(smooth_light::shade_byte(occlusion, 1.0));
    debug_assert_eq!(
        (255 - byte) % 51,
        0,
        "{occlusion} is a mean of four shades of 0.2 or 1.0"
    );
    (255 - byte) / 51
}

pub(super) fn set_corners(words: &mut [u32; FACE_WORDS], ao: [u32; 4], light: [VertexLight; 4]) {
    let mut codes = 0u64;
    let mut block = 0u64;
    let mut sky = 0u64;
    for (corner, (ao, light)) in ao.into_iter().zip(light).enumerate() {
        let quarter = |value: u8| {
            debug_assert_eq!(
                value % 4,
                0,
                "a face that spans its cell sums four whole levels, so {value} is a multiple of 4"
            );
            u64::from(value / 4)
        };
        codes |= u64::from(ao) << (corner as u32 * FACE_AO_CORNER_BITS);
        block |= quarter(light.block) << (corner as u32 * FACE_LIGHT_CORNER_BITS);
        sky |= quarter(light.sky) << (corner as u32 * FACE_LIGHT_CORNER_BITS);
    }
    FACE_AO.set(words, codes);
    FACE_BLOCK_LIGHT.set(words, block);
    FACE_SKY_LIGHT.set(words, sky);
}
