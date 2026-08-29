use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use super::draws::PARAMS_STRIDE;
use super::stats::DrawArgs;
use crate::mesh::STREAMS;
use crate::pack::MAX_SECTIONS;
use crate::sky::SkyUniform;

pub(super) struct Frame {
    pub params: Buffer,
    pub sky: Buffer,
    pub cave: Buffer,
    pub args: Buffer,
    // Copied over `args` once a frame so the cull pass starts from zeroed instance counts
    // without one small clear per draw.
    pub args_reset: Buffer,
    pub args_readback: Buffer,
}

impl Frame {
    pub fn new(device: &RenderDevice) -> Self {
        let args_init = vec![DrawArgs::quad_strip(); STREAMS];
        Self {
            params: device.create_buffer(&BufferDescriptor {
                label: Some("terrain draw params"),
                size: STREAMS as u64 * PARAMS_STRIDE as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            sky: device.create_buffer(&BufferDescriptor {
                label: Some("terrain sky"),
                size: size_of::<SkyUniform>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            cave: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain cave visibility"),
                contents: bytemuck::cast_slice(&vec![u32::MAX; MAX_SECTIONS / 32]),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            }),
            args: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain draw args"),
                contents: bytemuck::cast_slice(&args_init),
                usage: BufferUsages::STORAGE
                    | BufferUsages::INDIRECT
                    | BufferUsages::COPY_DST
                    | BufferUsages::COPY_SRC,
            }),
            args_reset: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain draw args reset"),
                contents: bytemuck::cast_slice(&args_init),
                usage: BufferUsages::COPY_SRC,
            }),
            args_readback: device.create_buffer(&BufferDescriptor {
                label: Some("terrain draw args readback"),
                size: size_of_val(args_init.as_slice()) as u64,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
}
