use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;

use mcrs_minecraft_mesh::{Draw, DrawKind, QuadFormat};

use super::DrawMask;
use super::layer::LayerGroup;
use super::shared_types::{DrawArgRow, PARAMS_MODEL, PARAMS_QUAD_CULL, PARAMS_SIZE, Params};

pub(super) const PARAMS_STRIDE: u32 = 256;
const _: () = assert!(size_of::<Params>() as u64 == PARAMS_SIZE);

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
            params: Vec::with_capacity(DrawKind::COUNT),
            dirty: false,
        }
    }

    pub fn rebuild(&mut self) {
        self.params.clear();
        let mut visible_base = 0u32;
        for draw in &self.draws {
            let mut flags = 0u32;
            if draw.kind.format == QuadFormat::Model {
                flags |= PARAMS_MODEL;
            }
            if draw.kind.writes_depth() {
                flags |= PARAMS_QUAD_CULL;
            }
            self.params.push(Params {
                group_base: draw.first_group,
                group_count: draw.group_count,
                visible_base,
                args_index: draw.kind.index() as u32,
                counter: DrawArgRow::HiddenQuads.slot(draw.kind) as u32,
                flags,
                _pad_24: [0; 2],
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
        for (chunk, entry) in bytes.chunks_mut(PARAMS_STRIDE as usize).zip(&self.params) {
            if let Some(at) = chunk.get_mut(..PARAMS_SIZE as usize) {
                at.copy_from_slice(bytemuck::bytes_of(entry));
            }
        }
        queue.write_buffer(params, 0, &bytes);
    }

    pub fn drawn<'a>(
        &'a self,
        group: LayerGroup,
        mask: &'a DrawMask,
    ) -> impl Iterator<Item = (usize, &'a Draw)> {
        self.draws.iter().enumerate().filter(move |(_, draw)| {
            draw.quad_count != 0 && group.holds(draw.kind) && mask.drawn(draw.kind)
        })
    }
}
