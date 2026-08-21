use std::num::NonZeroU64;

use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d};
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::globals::GlobalsBuffer;
use bevy::render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset, ViewUniforms,
};

use crate::mesh::STREAM_NAMES;
use crate::probe::{self, GpuTimings, Queries};

use super::draws::{PARAMS_SIZE, PARAMS_STRIDE};
use super::layer::LayerGroup;
use super::sky::{self, SkyPart};
use super::stats::{DRAW_ARGS_SIZE, DrawnTriangles, copy_args};
use super::terrain::Terrain;
use super::{Clouds, Raster, Streams, Wireframe};

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
    queue.write_buffer(&terrain.frame.cave, 0, bytemuck::cast_slice(&cave.bits[..]));
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
        &pipeline_cache.get_bind_group_layout(&terrain.binds.view_layout),
        &BindGroupEntries::sequential((
            view_binding,
            BufferBinding {
                buffer: &terrain.frame.params,
                offset: 0,
                size: NonZeroU64::new(PARAMS_SIZE),
            },
            globals_binding,
            terrain.frame.sky.as_entire_buffer_binding(),
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
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull),
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull_stable),
    ) else {
        return;
    };
    let view_offset = view.into_inner().offset;

    copy_args(&terrain, &triangles, ctx.command_encoder());
    let reset = terrain.frame.args_reset.size();
    ctx.command_encoder().copy_buffer_to_buffer(
        &terrain.frame.args_reset,
        0,
        &terrain.frame.args,
        0,
        reset,
    );

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
    pass.set_bind_group(1, &terrain.binds.cull, &[]);
    cull_group(
        &mut pass,
        &terrain,
        LayerGroup::Opaque,
        compacting,
        &view_bind_group.0,
        view_offset,
        &streams,
    );
    cull_group(
        &mut pass,
        &terrain,
        LayerGroup::Translucent,
        stable,
        &view_bind_group.0,
        view_offset,
        &streams,
    );
    span.end(&mut pass);
}

fn cull_group<'pass>(
    pass: &mut ComputePass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    pipeline: &'pass ComputePipeline,
    view_bind_group: &'pass BindGroup,
    view_offset: u32,
    streams: &Streams,
) {
    pass.set_pipeline(pipeline);
    let mut open = None;
    for (index, &workgroups) in terrain.list.group_counts.iter().enumerate() {
        let stream = terrain.list.draws[index].stream;
        if workgroups == 0 || !group.holds(stream) || !streams.drawn(stream) {
            continue;
        }
        if open != Some(stream) {
            if open.is_some() {
                pass.pop_debug_group();
            }
            pass.push_debug_group(STREAM_NAMES[stream as usize]);
            open = Some(stream);
        }
        pass.set_bind_group(
            0,
            view_bind_group,
            &[view_offset, index as u32 * PARAMS_STRIDE],
        );
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
    if open.is_some() {
        pass.pop_debug_group();
    }
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
    if !terrain.pipelines.ready() {
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
    sky::draw(&mut pass, &terrain, &view_bind_group.0, view_offset, &pipeline_cache, SkyPart::Dome);
    draw_layer_group(
        &mut pass,
        &terrain,
        LayerGroup::Opaque,
        &view_bind_group.0,
        view_offset,
        &pipeline_cache,
        &streams,
        wireframe.0,
    );
    if clouds.0 {
        sky::draw(
            &mut pass,
            &terrain,
            &view_bind_group.0,
            view_offset,
            &pipeline_cache,
            SkyPart::Clouds,
        );
    }
    draw_layer_group(
        &mut pass,
        &terrain,
        LayerGroup::Translucent,
        &view_bind_group.0,
        view_offset,
        &pipeline_cache,
        &streams,
        wireframe.0,
    );

    span.end(&mut pass);
}

#[allow(clippy::too_many_arguments)]
fn draw_layer_group<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    view_bind_group: &'pass BindGroup,
    view_offset: u32,
    pipeline_cache: &'pass PipelineCache,
    streams: &Streams,
    wireframe: bool,
) {
    pass.set_bind_group(1, &terrain.binds.draw, &[]);
    let mut open = None;
    for (index, draw) in terrain.list.draws.iter().enumerate() {
        if draw.quad_count == 0 || !group.holds(draw.stream) || !streams.drawn(draw.stream) {
            continue;
        }
        let Some(pipeline) = terrain
            .pipelines
            .terrain(draw.stream, wireframe, pipeline_cache)
        else {
            continue;
        };
        if open != Some(draw.stream) {
            if open.is_some() {
                pass.pop_debug_group();
            }
            pass.push_debug_group(STREAM_NAMES[draw.stream as usize]);
            open = Some(draw.stream);
        }
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(
            0,
            view_bind_group,
            &[view_offset, index as u32 * PARAMS_STRIDE],
        );
        pass.draw_indirect(&terrain.frame.args, index as u64 * DRAW_ARGS_SIZE);
    }
    if open.is_some() {
        pass.pop_debug_group();
    }
}
