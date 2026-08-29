use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;

use crate::mesh::{Draw, STREAMS, stream_is_model};
use crate::pack::MODEL_OVERHANG;

use super::Budget;

pub(super) const PARAMS_STRIDE: u32 = 256;
pub(super) const PARAMS_SIZE: u64 = 48;
const _: () = assert!(size_of::<Params>() as u64 == PARAMS_SIZE);

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(super) struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    wireframe: u32,
    overhang: f32,
    animated_from: u32,
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    pad: u32,
}

pub(super) struct DrawList {
    pub draws: Vec<Draw>,
    pub group_counts: Vec<u32>,
    pub wireframe: u32,
    params: Vec<Params>,
    dirty: bool,
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            draws: Vec::new(),
            group_counts: Vec::new(),
            wireframe: 0,
            params: Vec::with_capacity(STREAMS),
            dirty: false,
        }
    }

    pub fn rebuild(&mut self, budget: &Budget, animated_from: u32) {
        self.params.clear();
        self.group_counts.clear();
        let mut visible_base = 0u32;
        for (index, draw) in self.draws.iter().enumerate() {
            self.params.push(Params {
                group_base: draw.first_group,
                group_count: draw.group_count,
                visible_base,
                args_index: index as u32,
                wireframe: self.wireframe,
                overhang: if stream_is_model(draw.stream) {
                    MODEL_OVERHANG
                } else {
                    0.0
                },
                animated_from,
                tint_origin_x: budget.tint_origin[0],
                tint_origin_z: budget.tint_origin[1],
                tint_span_x: budget.tint_size[0] as f32,
                tint_span_z: budget.tint_size[1] as f32,
                pad: 0,
            });
            visible_base += draw.quad_count;
            self.group_counts.push(draw.group_count);
        }
        self.dirty = true;
    }

    pub fn flush(&mut self, params: &Buffer, queue: &RenderQueue) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let stride = PARAMS_STRIDE as usize;
        let mut bytes = vec![0u8; self.params.len() * stride];
        for (index, entry) in self.params.iter().enumerate() {
            let at = index * stride;
            bytes[at..at + PARAMS_SIZE as usize].copy_from_slice(bytemuck::bytes_of(entry));
        }
        if !bytes.is_empty() {
            queue.write_buffer(params, 0, &bytes);
        }
    }
}

pub(super) fn prepare_wireframe(
    wireframe: Res<super::Wireframe>,
    mut terrain: Option<ResMut<super::terrain::Terrain>>,
    queue: Res<RenderQueue>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    let flag = u32::from(wireframe.0);
    if terrain.list.wireframe == flag {
        return;
    }
    let terrain = terrain.as_mut();
    terrain.list.wireframe = flag;
    terrain.rebuild_params();
    terrain.list.flush(&terrain.frame.params, &queue);
}
