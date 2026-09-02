use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;

use crate::mesh::{Draw, STREAMS, stream_is_model};
use crate::pack::MODEL_OVERHANG;

pub(super) const PARAMS_STRIDE: u32 = 256;
pub(super) const PARAMS_SIZE: u64 = 32;
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
    /// Keeps the struct a 16-byte multiple, which every backend lays a uniform
    /// out to whatever the member alignments alone would allow.
    padding: u32,
}

pub(super) struct DrawList {
    pub draws: Vec<Draw>,
    pub group_counts: Vec<u32>,
    pub wireframe: u32,
    /// Entries the visible list has to hold for the draws as they stand.
    ///
    /// A blended draw addresses the list by the slot the mesher gave each
    /// group, so its span is its whole quad count whatever the cull leaves
    /// alive; an opaque one packs into the front of its span but can fill it.
    /// Handing a draw anything less drops quads by where they sit in the arena
    /// rather than by where they sit in the world.
    pub visible_entries: usize,
    params: Vec<Params>,
    dirty: bool,
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            draws: Vec::new(),
            group_counts: Vec::new(),
            wireframe: 0,
            visible_entries: 0,
            params: Vec::with_capacity(STREAMS),
            dirty: false,
        }
    }

    pub fn rebuild(&mut self, animated_from: u32) {
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
                padding: 0,
            });
            visible_base += draw.quad_count;
            self.group_counts.push(draw.group_count);
        }
        self.visible_entries = visible_base as usize;
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
    device: Res<bevy::render::renderer::RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
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
    terrain.rebuild_params(&device, &pipeline_cache);
    terrain.list.flush(&terrain.frame.params, &queue);
}
