use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;

use crate::mesh::{Draw, STREAMS, stream_is_model};
use crate::pack::MODEL_OVERHANG;

use super::Streams;
use super::layer::LayerGroup;

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
    overhang: f32,
    /// Keeps the struct a 16-byte multiple, which every backend lays a uniform
    /// out to whatever the member alignments alone would allow.
    padding: [u32; 3],
}

pub(super) struct DrawList {
    pub draws: Vec<Draw>,
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
            visible_entries: 0,
            params: Vec::with_capacity(STREAMS),
            dirty: false,
        }
    }

    pub fn rebuild(&mut self) {
        self.params.clear();
        let mut visible_base = 0u32;
        for (index, draw) in self.draws.iter().enumerate() {
            self.params.push(Params {
                group_base: draw.first_group,
                group_count: draw.group_count,
                visible_base,
                args_index: index as u32,
                overhang: if stream_is_model(draw.stream) {
                    MODEL_OVERHANG
                } else {
                    0.0
                },
                padding: [0; 3],
            });
            visible_base += draw.quad_count;
        }
        self.visible_entries = visible_base as usize;
        self.dirty = true;
    }

    pub fn flush(&mut self, params: &Buffer, queue: &RenderQueue) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        for (index, entry) in self.params.iter().enumerate() {
            let at = index as u64 * PARAMS_STRIDE as u64;
            queue.write_buffer(params, at, bytemuck::bytes_of(entry));
        }
    }

    pub fn drawn<'a>(
        &'a self,
        group: LayerGroup,
        streams: &'a Streams,
    ) -> impl Iterator<Item = (usize, &'a Draw)> {
        self.draws.iter().enumerate().filter(move |(_, draw)| {
            draw.quad_count != 0 && group.holds(draw.stream) && streams.drawn(draw.stream)
        })
    }
}
