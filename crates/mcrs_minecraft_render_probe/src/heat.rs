use bevy::prelude::*;
use bevy::render::render_resource::binding_types::storage_buffer_sized;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::shader::ShaderDefVal;
use mcrs_minecraft_render::Terrain;
use wgpu::ComputePassTimestampWrites;

const HEAT_THREADS: u32 = 256;

#[derive(Resource)]
pub(super) struct Heat {
    pipeline: CachedComputePipelineId,
    bind: BindGroup,
    workgroups: u32,
}

#[derive(Resource)]
pub(super) struct HeatWorkgroups(pub u32);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::per_frame_insert_resource,
        reason = "the run condition stops the system once the heat pipeline exists, so the insert happens once"
    )
)]
pub(super) fn init_heat(
    mut commands: Commands,
    workgroups: Res<HeatWorkgroups>,
    device: Res<RenderDevice>,
    terrain: Option<Res<Terrain>>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(args) = terrain.as_deref().and_then(Terrain::draw_args) else {
        return;
    };
    let workgroups = workgroups
        .0
        .min(device.limits().max_compute_workgroups_per_dimension);
    let layout = BindGroupLayoutDescriptor::new(
        "gpu heat",
        &BindGroupLayoutEntries::single(ShaderStages::COMPUTE, storage_buffer_sized(false, None)),
    );
    let bind = device.create_bind_group(
        "gpu heat",
        &pipeline_cache.get_bind_group_layout(&layout),
        &BindGroupEntries::single(args.as_entire_buffer_binding()),
    );
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("gpu heat".into()),
        layout: vec![layout],
        shader: asset_server.load("embedded://mcrs_minecraft_render_probe/shaders/heat.wesl"),
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
        timestamps: Option<ComputePassTimestampWrites<'_>>,
        encoder: &mut CommandEncoder,
    ) {
        let Some(pipeline) = pipeline_cache.get_compute_pipeline(self.pipeline) else {
            return;
        };
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("gpu heat"),
            timestamp_writes: timestamps,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(self.workgroups, 1, 1);
    }
}
