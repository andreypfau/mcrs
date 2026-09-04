use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use crate::mesh::Group;
use crate::pack::QUAD_WORDS;

use super::stats::INDICES_PER_QUAD;
use super::upload::Pending;
use super::{Budget, SECTION_BYTES, VISIBLE_BYTES};

/// Entries the visible list starts at, and the step it grows by.
const VISIBLE_STEP: usize = 1 << 20;

pub(super) struct Arenas {
    pub quads: Buffer,
    pub vertices: Buffer,
    pub faces: Buffer,
    pub groups: Buffer,
    pub sections: Buffer,
    pub visible: Buffer,
    /// Six indices a visible slot, naming its quad's four corners as two triangles.
    pub indices: Buffer,
    /// One count per batch of `CULL_THREADS` groups, which the ordered cull turns into where
    /// each batch's survivors start.
    pub batches: Buffer,
    /// One word per group: left to the second cull pass by the first.
    pub candidates: Buffer,
    pub pending: Option<Pending>,
}

fn arena(label: &str, bytes: u64, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: bytes.max(size_of::<[u32; QUAD_WORDS]>() as u64),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn visible_list(entries: usize, device: &RenderDevice) -> Buffer {
    arena(
        "terrain visible list",
        (entries * VISIBLE_BYTES) as u64,
        device,
    )
}

/// Wound as the strip the quads used to be drawn as: corners 1, 2, 0 and then 0, 2, 3.
fn quad_indices(entries: usize, device: &RenderDevice) -> Buffer {
    let mut indices = Vec::with_capacity(entries * INDICES_PER_QUAD as usize);
    for quad in 0..entries as u32 {
        let base = quad * 4;
        indices.extend_from_slice(&[base + 1, base + 2, base, base, base + 2, base + 3]);
    }
    device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain quad indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: BufferUsages::INDEX,
    })
}

impl Arenas {
    pub fn new(budget: &Budget, device: &RenderDevice) -> Self {
        let arena = |label, bytes| arena(label, bytes, device);
        Self {
            quads: arena("terrain quads", (budget.quads * QUAD_WORDS * 4) as u64),
            vertices: arena(
                "terrain vertices",
                (budget.models * super::MODEL_BYTES) as u64,
            ),
            faces: arena("terrain faces", (budget.faces * 4) as u64),
            groups: arena(
                "terrain groups",
                (budget.groups * size_of::<Group>()) as u64,
            ),
            sections: arena("terrain sections", (budget.sections * SECTION_BYTES) as u64),
            visible: visible_list(VISIBLE_STEP, device),
            indices: quad_indices(VISIBLE_STEP, device),
            batches: arena(
                "terrain cull batches",
                (budget.groups.div_ceil(super::pass::CULL_THREADS as usize) * 4) as u64,
            ),
            candidates: arena("terrain cull candidates", (budget.groups * 4) as u64),
            pending: None,
        }
    }

    /// Grows the visible list to hold `entries`, answering whether it had to —
    /// a new buffer is a new binding, so the caller rebuilds the bind groups
    /// that name it.
    ///
    /// The list is never shared out or clamped: a draw that does not fit would
    /// lose quads by their place in the arena, which reads as terrain blinking
    /// out rather than as a budget being spent.
    pub fn grow_visible(&mut self, entries: usize, device: &RenderDevice) -> bool {
        if (entries * VISIBLE_BYTES) as u64 <= self.visible.size() {
            return false;
        }
        let entries = entries.next_multiple_of(VISIBLE_STEP);
        bevy::log::info!(entries, "growing the visible list");
        self.visible = visible_list(entries, device);
        self.indices = quad_indices(entries, device);
        true
    }
}
