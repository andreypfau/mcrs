use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use mcrs_minecraft_mesh::Group;
use mcrs_minecraft_mesh::pack::QUAD_WORDS;

use super::upload::{Pending, Placement};
use super::{Budget, SECTION_BYTES, VISIBLE_BYTES};

/// Entries the visible list starts at, and the granularity it grows in.
const VISIBLE_STEP: usize = 1 << 20;

/// Room left above what a growth was asked for, so a list that creeps up by a
/// group at a time is not reallocated every frame. Rounding to the next power
/// of two would leave up to twice the entries unused, and at a full render
/// distance one entry over sixty-seven million costs another half gigabyte.
const VISIBLE_HEADROOM: usize = 4;

/// Bytes a geometry arena starts at, and the granularity it grows in. The allocator packs blocks
/// towards offset zero, so what is written stays near what is resident and the buffer only
/// has to reach the highest byte a placement touches, not the budget the allocator may use.
const GEOMETRY_STEP: u64 = 16 << 20;

pub(super) struct Geometry {
    pub buffer: Buffer,
    label: &'static str,
    limit: u64,
}

impl Geometry {
    fn new(label: &'static str, limit: u64, device: &RenderDevice) -> Self {
        Self {
            buffer: geometry_buffer(label, GEOMETRY_STEP.min(limit), device),
            label,
            limit,
        }
    }

    /// Grows the buffer to hold `end` bytes, carrying what it held across with a copy recorded
    /// ahead of the writes that follow in the same encoder, and answers whether it had to.
    fn fit(&mut self, end: u64, device: &RenderDevice, encoder: &mut CommandEncoder) -> bool {
        if end <= self.buffer.size() {
            return false;
        }
        let size = (end + end / VISIBLE_HEADROOM as u64)
            .next_multiple_of(GEOMETRY_STEP)
            .min(self.limit.max(end));
        bevy::log::info!(label = self.label, mb = size >> 20, "growing a terrain arena");
        let grown = geometry_buffer(self.label, size, device);
        encoder.copy_buffer_to_buffer(&self.buffer, 0, &grown, 0, self.buffer.size());
        self.buffer = grown;
        true
    }
}

pub(super) struct Rebind {
    pub draw: bool,
    pub cull: bool,
}

fn batch_bytes(groups: usize) -> u64 {
    (groups.div_ceil(super::pass::CULL_THREADS as usize) * 4) as u64
}

fn candidate_bytes(groups: usize) -> u64 {
    (groups * 12) as u64
}

fn geometry_buffer(label: &str, size: u64, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: size.next_multiple_of(COPY_BUFFER_ALIGNMENT),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

pub(super) struct Arenas {
    pub quads: Geometry,
    pub vertices: Geometry,
    pub faces: Geometry,
    pub groups: Geometry,
    pub sections: Buffer,
    pub visible: Buffer,
    /// One count per batch of `CULL_THREADS` groups, which the ordered cull turns into where
    /// each batch's survivors start.
    pub batches: Geometry,
    /// Three words per group: the quads the first cull pass left to the second, a place in the
    /// list of groups a pass's group cull kept, and one in the list of groups the first pass left
    /// quads of.
    pub candidates: Geometry,
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
            quads: Geometry::new(
                "terrain quads",
                (budget.quads * QUAD_WORDS * 4) as u64,
                device,
            ),
            vertices: Geometry::new(
                "terrain vertices",
                (budget.models * super::MODEL_BYTES) as u64,
                device,
            ),
            faces: Geometry::new(
                "terrain faces",
                (budget.faces * super::FACE_BYTES) as u64,
                device,
            ),
            groups: Geometry::new(
                "terrain groups",
                (budget.groups * size_of::<Group>()) as u64,
                device,
            ),
            sections: arena("terrain sections", (budget.sections * SECTION_BYTES) as u64),
            visible: visible_list(VISIBLE_STEP, device),
            batches: Geometry::new(
                "terrain cull batches",
                batch_bytes(budget.groups),
                device,
            ),
            candidates: Geometry::new(
                "terrain cull candidates",
                candidate_bytes(budget.groups),
                device,
            ),
            pending: None,
        }
    }

    /// Grows the arenas to reach the end of each write in `placement`, answering which of the
    /// draw and cull bind groups name a buffer that was replaced.
    pub fn fit(
        &mut self,
        placement: &Placement,
        device: &RenderDevice,
        encoder: &mut CommandEncoder,
    ) -> Rebind {
        fn end<T>((offset, data): &(u64, Vec<T>)) -> u64 {
            offset + size_of_val(&data[..]) as u64
        }
        let quads = self.quads.fit(end(&placement.quads), device, encoder);
        let vertices = self.vertices.fit(end(&placement.vertices), device, encoder);
        let faces = self.faces.fit(end(&placement.faces), device, encoder);
        let groups_end = placement
            .groups
            .iter()
            .map(|(offset, records)| offset + size_of_val(&records[..]) as u64)
            .max()
            .unwrap_or(0);
        let mut cull = self.groups.fit(groups_end, device, encoder);
        if cull {
            let groups = self.groups.buffer.size() as usize / size_of::<Group>();
            self.batches.fit(batch_bytes(groups), device, encoder);
            self.candidates.fit(candidate_bytes(groups), device, encoder);
            cull = true;
        }
        Rebind {
            draw: quads | vertices | faces,
            cull,
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
        let entries = (entries + entries / VISIBLE_HEADROOM)
            .next_multiple_of(VISIBLE_STEP)
            .max(VISIBLE_STEP);
        bevy::log::info!(entries, "growing the visible list");
        self.visible = visible_list(entries, device);
        true
    }
}
