use bevy::prelude::*;
use bevy::render::render_resource::binding_types::storage_buffer_sized;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::shader::ShaderDefVal;

use crate::probe::{self, GpuTimings, Queries};

use super::terrain::Terrain;

const HEAT_THREADS: u32 = 256;

/// A measurement knob: arithmetic burnt ahead of the frame's own passes so a GPU that would
/// otherwise idle most of a composited frame stays at speed while the passes are timed. It
/// writes a zero into the draw args' first vertex, which is zero already, so that the driver
/// orders it after the last world pass and before the cull rather than overlapping either.
#[derive(Resource)]
pub(super) struct Heat {
    pipeline: CachedComputePipelineId,
    bind: BindGroup,
    workgroups: u32,
}

pub(super) fn init_heat(
    mut commands: Commands,
    device: Res<RenderDevice>,
    terrain: Res<Terrain>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(workgroups) = crate::config::gpu_hot() else {
        return;
    };
    let workgroups = workgroups.min(device.limits().max_compute_workgroups_per_dimension);
    let layout = BindGroupLayoutDescriptor::new(
        "gpu heat",
        &BindGroupLayoutEntries::single(ShaderStages::COMPUTE, storage_buffer_sized(false, None)),
    );
    let bind = device.create_bind_group(
        "gpu heat",
        &pipeline_cache.get_bind_group_layout(&layout),
        &BindGroupEntries::single(terrain.frame.args.as_entire_buffer_binding()),
    );
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("gpu heat".into()),
        layout: vec![layout],
        shader: asset_server.load("embedded://mcrs_minecraft_client/render/shaders/core/heat.wgsl"),
        shader_defs: vec![ShaderDefVal::UInt("HEAT_THREADS".into(), HEAT_THREADS)],
        entry_point: Some("heat".into()),
        ..default()
    });
    commands.insert_resource(Heat {
        pipeline,
        bind,
        workgroups,
    });
}

impl Heat {
    pub fn dispatch(
        &self,
        pipeline_cache: &PipelineCache,
        queries: Option<&Queries>,
        timings: &GpuTimings,
        encoder: &mut CommandEncoder,
    ) {
        let Some(pipeline) = pipeline_cache.get_compute_pipeline(self.pipeline) else {
            return;
        };
        let timestamps = queries.map(|q| q.compute(probe::HEAT, timings));
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("gpu heat"),
            timestamp_writes: timestamps,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(self.workgroups, 1, 1);
    }
}
