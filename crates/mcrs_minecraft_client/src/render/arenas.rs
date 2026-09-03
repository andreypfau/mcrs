use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use crate::mesh::Group;
use crate::pack::QUAD_WORDS;

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
        true
    }
}
