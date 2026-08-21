use bevy::prelude::*;
use bevy::render::renderer::RenderQueue;

use crate::pack::MODEL_OVERHANG;

use super::Wireframe;
use super::terrain::Terrain;

pub(super) const PARAMS_STRIDE: u32 = 256;
pub(super) const PARAMS_SIZE: u64 = 64;
const _: () = assert!(size_of::<Params>() as u64 == PARAMS_SIZE);

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(super) struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    cave_base: u32,
    wireframe: u32,
    overhang: f32,
    animated_from: u32,
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    face_origin: u32,
}

pub(super) fn rebuild(terrain: &mut Terrain) {
    let animated_from = terrain.animated_from;
    let wireframe = terrain.wireframe;
    let tint_origin = terrain.layout.tint_origin;
    let tint_size = terrain.layout.tint_size;
    terrain.params_cpu.clear();
    terrain.group_counts.clear();
    let mut visible_base = 0u32;
    for (index, draw) in terrain.draws.iter().enumerate() {
        terrain.params_cpu.push(Params {
            group_base: draw.first_group,
            group_count: draw.group_count,
            visible_base,
            args_index: index as u32,
            origin_x: draw.origin[0],
            origin_y: draw.origin[1],
            origin_z: draw.origin[2],
            cave_base: draw.cave_base,
            wireframe,
            overhang: if draw.stream % 2 == 1 {
                MODEL_OVERHANG
            } else {
                0.0
            },
            animated_from,
            tint_origin_x: tint_origin[0],
            tint_origin_z: tint_origin[1],
            tint_span_x: tint_size[0] as f32,
            tint_span_z: tint_size[1] as f32,
            face_origin: draw.face_base,
        });
        visible_base += draw.quad_count;
        terrain.group_counts.push(draw.group_count);
    }
    terrain.params_dirty = true;
}

pub(super) fn write(terrain: &mut Terrain, queue: &RenderQueue) {
    terrain.params_dirty = false;
    let stride = PARAMS_STRIDE as usize;
    let mut bytes = vec![0u8; terrain.params_cpu.len() * stride];
    for (index, entry) in terrain.params_cpu.iter().enumerate() {
        let at = index * stride;
        bytes[at..at + PARAMS_SIZE as usize].copy_from_slice(bytemuck::bytes_of(entry));
    }
    if !bytes.is_empty() {
        queue.write_buffer(&terrain.params, 0, &bytes);
    }
}

pub(super) fn prepare_wireframe(
    wireframe: Res<Wireframe>,
    mut terrain: Option<ResMut<Terrain>>,
    queue: Res<RenderQueue>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    let flag = u32::from(wireframe.0);
    if terrain.wireframe == flag {
        return;
    }
    terrain.wireframe = flag;
    rebuild(terrain);
    write(terrain, &queue);
}
