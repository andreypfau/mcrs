use wgpu::util::DeviceExt;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, CommandEncoder, Device};

use crate::entity_gpu::Capacities;
use crate::entity_library::{EntityLibrary, MAX_ARRAYS, MAX_DEFINITIONS};
use crate::frame_uniform::FrameUniform;
use crate::records::{CullRecord, InputRecord, ScatterEntry, StateRecord, TickRecord};

pub(crate) const COUNTERS: u64 = 64;
const BONE_MATRIX_BYTES: u64 = 48;
const INSTANCE_BYTES: u64 = 80;

pub(crate) struct SlotBuffers {
    pub(crate) capacity: u32,
    /// The tick record of each slot, then its previous one.
    pub(crate) ticks: Buffer,
    pub(crate) tick_of_record: Buffer,
    pub(crate) culls: Buffer,
    pub(crate) states: Buffer,
    pub(crate) inputs: Buffer,
}

impl SlotBuffers {
    pub(crate) fn new(device: &Device, capacity: u32) -> Self {
        let capacity = capacity.max(64);
        let storage = |label: &str, size: u64| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        let count = u64::from(capacity);
        Self {
            capacity,
            ticks: storage("entity ticks", 2 * count * size_of::<TickRecord>() as u64),
            tick_of_record: storage("entity tick of record", count * 4),
            culls: storage("entity culls", count * size_of::<CullRecord>() as u64),
            states: storage("entity states", count * size_of::<StateRecord>() as u64),
            inputs: storage("entity inputs", count * size_of::<InputRecord>() as u64),
        }
    }

    pub(crate) fn copy_into(&self, other: &Self, encoder: &mut CommandEncoder) {
        for (from, to) in [
            (&self.ticks, &other.ticks),
            (&self.tick_of_record, &other.tick_of_record),
            (&self.culls, &other.culls),
            (&self.states, &other.states),
            (&self.inputs, &other.inputs),
        ] {
            encoder.copy_buffer_to_buffer(from, 0, to, 0, from.size().min(to.size()));
        }
    }
}

pub(crate) struct StaticBuffers {
    pub(crate) models: Buffer,
    pub(crate) bones: Buffer,
    pub(crate) cuboids: Buffer,
    pub(crate) variants: Buffer,
    pub(crate) cutout: Buffer,
}

impl StaticBuffers {
    pub(crate) fn new(device: &Device, library: &EntityLibrary) -> Self {
        // An empty table still holds the one record that its shader array declares.
        let table = |label: &str, bytes: &[u8], usage: BufferUsages| {
            let padded;
            let contents = if bytes.is_empty() {
                padded = vec![0u8; 64];
                &padded
            } else {
                bytes
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let (uniform, storage) = (BufferUsages::UNIFORM, BufferUsages::STORAGE);
        Self {
            models: table(
                "entity models",
                bytemuck::cast_slice(&library.models),
                uniform,
            ),
            bones: table(
                "entity bones",
                bytemuck::cast_slice(&library.bones),
                uniform,
            ),
            cuboids: table("entity cuboids", &library.cuboids, storage),
            variants: table(
                "entity variants",
                bytemuck::cast_slice(&library.variants),
                uniform,
            ),
            cutout: table("entity cutout", &library.cutout, storage),
        }
    }
}

pub(crate) struct FrameBuffers {
    pub(crate) uniform: Buffer,
    pub(crate) visible: Buffer,
    pub(crate) bone_matrices: Buffer,
    pub(crate) instances: Buffer,
    pub(crate) faces: Buffer,
    pub(crate) counters: Buffer,
    pub(crate) counters_zero: Buffer,
    pub(crate) indirect: Buffer,
    pub(crate) scatter: Buffer,
    pub(crate) scatter_capacity: u32,
    /// The slot of each impostor of the frame.
    pub(crate) impostors: Buffer,
    /// The draw record of each impostor of the frame.
    pub(crate) impostor_draws: Buffer,
}

/// The bytes of the draw record of one impostor.
const IMPOSTOR_DRAW_BYTES: u64 = 128;

/// The draw records of the impostors of `slots` slots: the impostors that discard from the
/// start, the small ones from the end.
pub(crate) fn impostor_draws(device: &Device, slots: u32) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("entity impostor draws"),
        size: u64::from(slots.max(64)) * IMPOSTOR_DRAW_BYTES,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

/// The impostor list of `slots` slots.
pub(crate) fn impostor_list(device: &Device, slots: u32) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("entity impostors"),
        size: u64::from(slots.max(64)) * 4,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    })
}

impl FrameBuffers {
    pub(crate) fn new(
        device: &Device,
        capacities: Capacities,
        scatter_capacity: u32,
        definitions: usize,
        arrays: usize,
    ) -> Self {
        let buffer = |label: &str, size: u64, usage: BufferUsages| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = BufferUsages::STORAGE | BufferUsages::COPY_SRC;
        let definitions = definitions.clamp(1, MAX_DEFINITIONS) as u64;
        let arrays = arrays.clamp(1, MAX_ARRAYS) as u64;
        let rigs = u64::from(capacities.full_rigs);
        Self {
            uniform: buffer(
                "entity frame",
                size_of::<FrameUniform>() as u64,
                BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            ),
            visible: buffer("entity visible", definitions * rigs * 8, storage),
            bone_matrices: buffer(
                "entity bone matrices",
                u64::from(capacities.bones) * BONE_MATRIX_BYTES,
                storage,
            ),
            instances: buffer(
                "entity instances",
                definitions * rigs * INSTANCE_BYTES,
                storage,
            ),
            faces: buffer(
                "entity faces",
                arrays * u64::from(capacities.faces) * 8,
                storage,
            ),
            counters: buffer(
                "entity counters",
                COUNTERS * 4,
                storage | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            ),
            counters_zero: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("entity counters zero"),
                contents: &[0u8; (COUNTERS * 4) as usize],
                usage: BufferUsages::COPY_SRC,
            }),
            indirect: buffer(
                "entity indirect",
                (definitions * 6 + arrays * 8 + 12) * 4,
                storage | BufferUsages::INDIRECT,
            ),
            scatter: buffer(
                "entity scatter",
                u64::from(scatter_capacity.max(1)) * size_of::<ScatterEntry>() as u64,
                storage | BufferUsages::COPY_DST,
            ),
            scatter_capacity: scatter_capacity.max(1),
            impostors: impostor_list(device, scatter_capacity),
            impostor_draws: impostor_draws(device, scatter_capacity),
        }
    }
}
