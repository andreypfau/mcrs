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
    pub(crate) ticks: Buffer,
    pub(crate) previous_ticks: Buffer,
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
            ticks: storage("entity ticks", count * size_of::<TickRecord>() as u64),
            previous_ticks: storage(
                "entity previous ticks",
                count * size_of::<TickRecord>() as u64,
            ),
            tick_of_record: storage("entity tick of record", count * 4),
            culls: storage("entity culls", count * size_of::<CullRecord>() as u64),
            states: storage("entity states", count * size_of::<StateRecord>() as u64),
            inputs: storage("entity inputs", count * size_of::<InputRecord>() as u64),
        }
    }

    pub(crate) fn copy_into(&self, other: &Self, encoder: &mut CommandEncoder) {
        for (from, to) in [
            (&self.ticks, &other.ticks),
            (&self.previous_ticks, &other.previous_ticks),
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
        let storage = |label: &str, bytes: &[u8]| {
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
                usage: BufferUsages::STORAGE,
            })
        };
        Self {
            models: storage("entity models", bytemuck::cast_slice(&library.models)),
            bones: storage("entity bones", bytemuck::cast_slice(&library.bones)),
            cuboids: storage("entity cuboids", &library.cuboids),
            variants: storage("entity variants", bytemuck::cast_slice(&library.variants)),
            cutout: storage("entity cutout", &library.cutout),
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
                (definitions * 6 + arrays * 4) * 4,
                storage | BufferUsages::INDIRECT,
            ),
            scatter: buffer(
                "entity scatter",
                u64::from(scatter_capacity.max(1)) * size_of::<ScatterEntry>() as u64,
                storage | BufferUsages::COPY_DST,
            ),
            scatter_capacity: scatter_capacity.max(1),
        }
    }
}
