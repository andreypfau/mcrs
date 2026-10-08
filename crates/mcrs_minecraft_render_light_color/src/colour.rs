use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, storage_buffer_sized, texture_storage_3d,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::ExtractedView;
use bevy::shader::ShaderCacheError;
use mcrs_minecraft_light_color::layout::{REGION_CELLS, WAVES};
use mcrs_minecraft_render::CameraOrigin;

use crate::volume::{BRICK_SIDE, Volume};

const WORKGROUP: u64 = 64;

/// Queued once. The deferred path does not wait for them: until they build, every section reads
/// the vanilla tint.
#[derive(Resource, Default)]
pub(crate) struct ColourPipelines {
    stages: Option<[CachedComputePipelineId; 3]>,
}

pub(crate) fn colour_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "light colour",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                texture_storage_3d(TextureFormat::Rgba8Unorm, StorageTextureAccess::WriteOnly),
            ),
        ),
    )
}

pub(crate) fn prepare_colour_pipelines(
    mut pipelines: ResMut<ColourPipelines>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
    mut failed: Local<HashSet<CachedComputePipelineId>>,
) {
    if pipelines.stages.is_none() {
        let shader =
            asset_server.load("embedded://mcrs_minecraft_render_light_color/shaders/colour.wgsl");
        pipelines.stages = Some(["gather", "waves", "resolve"].map(|entry| {
            cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(format!("light colour {entry}").into()),
                layout: vec![colour_layout()],
                shader: shader.clone(),
                entry_point: Some(entry.into()),
                ..default()
            })
        }));
    }
    for id in pipelines.stages.into_iter().flatten() {
        if let CachedPipelineState::Err(error) = cache.get_compute_pipeline_state(id)
            && !matches!(
                error,
                ShaderCacheError::ShaderNotLoaded(_)
                    | ShaderCacheError::ShaderImportNotYetAvailable
            )
            && failed.insert(id)
        {
            let label = cache.get_compute_pipeline_descriptor(id).label.as_deref();
            error!(?label, "a light colour pipeline cannot be built: {error}");
        }
    }
}

/// Group `g` reads `lanes[1 - g]` and writes `lanes[g]`.
fn bind_groups(volume: &Volume, device: &RenderDevice, cache: &PipelineCache) -> [BindGroup; 2] {
    let layout = cache.get_bind_group_layout(&colour_layout());
    [0, 1].map(|g| {
        device.create_bind_group(
            "light colour",
            &layout,
            &BindGroupEntries::sequential((
                volume.scratch.jobs.as_entire_binding(),
                volume.pool.as_entire_binding(),
                volume.scratch.cost.as_entire_binding(),
                volume.scratch.lanes[1 - g].as_entire_binding(),
                volume.scratch.lanes[g].as_entire_binding(),
                &volume.atlas,
            )),
        )
    })
}

/// Page entries are written after the pass is recorded: queue writes land before the frame's
/// commands, and this pass runs before lighting, so no entry names a slot not yet filled.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy render-graph system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn propagate_colour(
    _view: ViewQuery<&'static ExtractedView>,
    volume: Option<ResMut<Volume>>,
    pipelines: Res<ColourPipelines>,
    cache: Res<PipelineCache>,
    origin: Res<CameraOrigin>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut ctx: RenderContext,
) {
    let (Some(mut volume), Some(stages)) = (volume, pipelines.stages) else {
        return;
    };
    let [Some(gather), Some(waves), Some(resolve)] =
        stages.map(|id| cache.get_compute_pipeline(id))
    else {
        return;
    };
    let volume = &mut *volume;
    let jobs = volume.take_jobs(origin.section);
    if !jobs.records.is_empty() {
        if jobs.lane_words > volume.scratch.lane_words {
            volume.grow_lanes(jobs.lane_words, &device);
        }
        if volume.colour_groups.is_none() {
            volume.colour_groups = Some(bind_groups(volume, &device, &cache));
        }
        let groups = volume.colour_groups.as_ref().expect("made above");
        queue.write_buffer(&volume.scratch.jobs, 0, bytemuck::cast_slice(&jobs.records));
        let count = jobs.records.len() as u32;
        let workgroups = |cells: u64| cells.div_ceil(WORKGROUP) as u32;
        let diagnostics = ctx.diagnostic_recorder();
        let diagnostics = diagnostics.as_deref();
        let mut pass = ctx
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("light colour"),
                timestamp_writes: None,
            });
        let span = diagnostics.pass_span(&mut pass, "light colour");
        pass.set_pipeline(gather);
        pass.set_bind_group(0, &groups[0], &[]);
        pass.dispatch_workgroups(workgroups(REGION_CELLS as u64), count, 1);
        pass.set_pipeline(waves);
        let lanes = REGION_CELLS as u64 * jobs.widest as u64;
        for wave in 0..WAVES {
            pass.set_bind_group(0, &groups[(wave + 1) % 2], &[]);
            pass.dispatch_workgroups(workgroups(lanes), count, 1);
        }
        pass.set_pipeline(resolve);
        pass.set_bind_group(0, &groups[(WAVES + 1) % 2], &[]);
        pass.dispatch_workgroups(workgroups(BRICK_SIDE.pow(3) as u64), count, 1);
        span.end(&mut pass);
    }
    volume.publish(&queue);
}
