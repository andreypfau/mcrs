use std::num::NonZeroU64;

use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d};
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::globals::GlobalsBuffer;
use bevy::render::render_phase::ViewBinnedRenderPhases;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset, ViewUniforms,
};

use crate::probe::{self, GpuTimings, Queries};

use super::params::{PARAMS_SIZE, PARAMS_STRIDE};
use super::pipeline::UNTESTED_PIPELINE;
use super::sky;
use super::stats::{DRAW_ARGS_SIZE, DrawnTriangles, INSTANCE_COUNT_OFFSET, copy_args};
use super::terrain::Terrain;
use super::{BLENDED_STREAM, Clouds, Raster, Streams, Wireframe};

#[derive(Resource)]
pub(super) struct ViewBindGroup(BindGroup);

pub(super) fn extract_cave_visibility(
    cave: Extract<Res<crate::cave::CaveCull>>,
    terrain: Option<Res<Terrain>>,
    queue: Res<RenderQueue>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    queue.write_buffer(&terrain.cave, 0, bytemuck::cast_slice(&cave.bits[..]));
}

pub(super) fn prepare_view_bind_group(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    globals: Res<GlobalsBuffer>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(globals_binding) = globals.buffer.binding() else {
        return;
    };
    commands.insert_resource(ViewBindGroup(device.create_bind_group(
        "terrain view",
        &pipeline_cache.get_bind_group_layout(&terrain.view_layout),
        &BindGroupEntries::sequential((
            view_binding,
            BufferBinding {
                buffer: &terrain.params,
                offset: 0,
                size: NonZeroU64::new(PARAMS_SIZE),
            },
            globals_binding,
            terrain.sky.as_entire_buffer_binding(),
        )),
    )));
}

pub(super) fn cull_terrain(
    view: ViewQuery<&ViewUniformOffset>,
    terrain: Option<Res<Terrain>>,
    view_bind_group: Option<Res<ViewBindGroup>>,
    pipeline_cache: Res<PipelineCache>,
    triangles: Res<DrawnTriangles>,
    queries: Option<Res<Queries>>,
    timings: Res<GpuTimings>,
    streams: Res<Streams>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(view_bind_group)) = (terrain, view_bind_group) else {
        return;
    };
    let (Some(compacting), Some(stable)) = (
        pipeline_cache.get_compute_pipeline(terrain.cull_pipeline),
        pipeline_cache.get_compute_pipeline(terrain.cull_stable_pipeline),
    ) else {
        return;
    };
    let view_offset = view.into_inner().offset;

    copy_args(&terrain, &triangles, ctx.command_encoder());

    for index in 0..terrain.draws.len() {
        ctx.command_encoder().clear_buffer(
            &terrain.args,
            index as u64 * DRAW_ARGS_SIZE + INSTANCE_COUNT_OFFSET,
            NonZeroU64::new(4).map(|n| n.get()),
        );
    }

    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let timestamps = queries.as_ref().map(|q| q.compute(probe::CULL, &timings));
    let mut pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("terrain cull"),
            timestamp_writes: timestamps,
        });
    let span = diagnostics.pass_span(&mut pass, "terrain_cull");
    pass.set_bind_group(1, &terrain.cull_bind_group, &[]);
    let mut blended = false;
    pass.set_pipeline(compacting);
    for (index, &workgroups) in terrain.group_counts.iter().enumerate() {
        if workgroups == 0 || !streams.drawn(terrain.draws[index].stream) {
            continue;
        }
        if !blended && terrain.draws[index].stream >= BLENDED_STREAM {
            blended = true;
            pass.set_pipeline(stable);
        }
        pass.set_bind_group(
            0,
            &view_bind_group.0,
            &[view_offset, index as u32 * PARAMS_STRIDE],
        );
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
    span.end(&mut pass);
}

pub(super) fn drop_unused_bins(
    mut opaque: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    mut alpha_mask: ResMut<ViewBinnedRenderPhases<AlphaMask3d>>,
) {
    opaque.clear();
    alpha_mask.clear();
}

pub(super) fn draw_terrain(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &ViewUniformOffset, &ExtractedView)>,
    terrain: Option<Res<Terrain>>,
    view_bind_group: Option<Res<ViewBindGroup>>,
    clouds: Res<Clouds>,
    streams: Res<Streams>,
    wireframe: Res<Wireframe>,
    raster: Res<Raster>,
    pipeline_cache: Res<PipelineCache>,
    queries: Option<Res<Queries>>,
    timings: Res<GpuTimings>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(view_bind_group)) = (terrain, view_bind_group) else {
        return;
    };
    if terrain.pipelines.is_none() {
        return;
    }
    let (target, depth, view_offset, extracted) = view.into_inner();
    let color_attachments = [Some(target.get_color_attachment())];
    let depth_attachment = Some(depth.get_attachment(StoreOp::Store));
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let timestamps = queries.as_ref().map(|q| q.render(probe::TERRAIN, &timings));
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("terrain"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: timestamps,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "terrain_draw");

    if raster.0 < 1.0 {
        let size = extracted.viewport.zw().as_vec2() * raster.0;
        pass.set_viewport(0.0, 0.0, size.x.max(1.0), size.y.max(1.0), 0.0, 1.0);
    }

    let view_offset = view_offset.offset;
    sky::draw(&mut pass, &terrain, &view_bind_group.0, view_offset, &pipeline_cache, false);

    pass.set_bind_group(1, &terrain.draw_bind_group, &[]);

    let mut clouds_drawn = false;
    for (index, draw) in terrain.draws.iter().enumerate() {
        if draw.quad_count == 0 || !streams.drawn(draw.stream) {
            continue;
        }
        if clouds.0 && !clouds_drawn && draw.stream >= BLENDED_STREAM {
            clouds_drawn = true;
            sky::draw(&mut pass, &terrain, &view_bind_group.0, view_offset, &pipeline_cache, true);
            pass.set_bind_group(1, &terrain.draw_bind_group, &[]);
        }
        let Some(pipeline) = terrain_pipeline(&terrain, draw.stream, wireframe.0, &pipeline_cache)
        else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(
            0,
            &view_bind_group.0,
            &[view_offset, index as u32 * PARAMS_STRIDE],
        );
        pass.draw_indirect(&terrain.args, index as u64 * DRAW_ARGS_SIZE);
    }

    if clouds.0 && !clouds_drawn {
        sky::draw(&mut pass, &terrain, &view_bind_group.0, view_offset, &pipeline_cache, true);
    }

    span.end(&mut pass);
}

fn terrain_pipeline<'cache>(
    terrain: &Terrain,
    stream: u32,
    wireframe: bool,
    pipeline_cache: &'cache PipelineCache,
) -> Option<&'cache RenderPipeline> {
    let stream = stream as usize;
    let index = match stream {
        0 | 1 if !wireframe => UNTESTED_PIPELINE + stream,
        _ => (stream / 4) * 2 + stream % 2,
    };
    pipeline_cache.get_render_pipeline(terrain.pipelines?[index])
}
