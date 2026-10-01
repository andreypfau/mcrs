use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;

use mcrs_minecraft_mesh::{Draw, STREAMS, stream_is_model, stream_pass};

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
    counter: u32,
    flags: u32,
    /// Keeps the struct a 16-byte multiple, which every backend lays a uniform
    /// out to whatever the member alignments alone would allow.
    padding: [u32; 2],
}

/// The draw's quads are models, with four corners of their own rather than a greedy rectangle.
const MODEL: u32 = 1;
/// The draw writes depth and culls back faces, so a quad is tested on its own: blended quads
/// keep the order the blend needs and go through by group.
const QUAD_CULL: u32 = 2;

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
                counter: 2 * STREAMS as u32 + index as u32,
                flags: if stream_is_model(draw.stream) { MODEL } else { 0 }
                    | if stream_pass(draw.stream).writes_depth() {
                        QUAD_CULL
                    } else {
                        0
                    },
                padding: [0; 2],
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
        let Some(last) = self.params.len().checked_sub(1) else {
            return;
        };
        // One write rather than one per draw: every write stages through a buffer of its own.
        let mut bytes = vec![0u8; last * PARAMS_STRIDE as usize + PARAMS_SIZE as usize];
        for (index, entry) in self.params.iter().enumerate() {
            let at = index * PARAMS_STRIDE as usize;
            bytes[at..at + PARAMS_SIZE as usize].copy_from_slice(bytemuck::bytes_of(entry));
        }
        queue.write_buffer(params, 0, &bytes);
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
