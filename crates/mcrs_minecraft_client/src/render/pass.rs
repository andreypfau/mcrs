use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset};

use crate::mesh::{STREAM_NAMES, STREAMS};
use crate::probe::{self, GpuTimings, Queries};
use crate::sky_render::SkyDraws;

use super::draws::PARAMS_STRIDE;
use super::heat::Heat;
use super::layer::LayerGroup;
use super::stats::{DRAW_ARGS_SIZE, DrawnTriangles, FrameCounts, copy_args};
use super::terrain::Terrain;
use super::upload::{UploadParams, apply_uploads};
use super::{Raster, Streams, Wireframe};

pub(super) fn extract_cave_visibility(
    cave: Extract<Res<crate::cave::CaveCull>>,
    terrain: Option<Res<Terrain>>,
    queue: Res<RenderQueue>,
    mut uploaded: Local<Option<u32>>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    if *uploaded == Some(cave.generation) {
        return;
    }
    *uploaded = Some(cave.generation);
    queue.write_buffer(&terrain.frame.cave, 0, bytemuck::cast_slice(&cave.bits[..]));
}

fn cull_terrain(
    terrain: &Terrain,
    pipeline_cache: &PipelineCache,
    triangles: &DrawnTriangles,
    queries: Option<&Queries>,
    timings: &GpuTimings,
    streams: &Streams,
    encoder: &mut CommandEncoder,
) {
    let (Some(compacting), Some(count), Some(scan), Some(scatter)) = (
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull),
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull_count),
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull_scan),
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull_scatter),
    ) else {
        return;
    };

    copy_args(terrain, triangles, encoder);
    let reset = terrain.frame.args_reset.size();
    encoder.copy_buffer_to_buffer(&terrain.frame.args_reset, 0, &terrain.frame.args, 0, reset);

    let timestamps = queries.map(|q| q.compute(probe::CULL, timings));
    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some("terrain cull"),
        timestamp_writes: timestamps,
    });
    pass.set_bind_group(1, &terrain.binds.cull, &[]);
    for group in LayerGroup::ALL {
        if group.culls_in_order() {
            cull_group(&mut pass, terrain, group, count, streams);
            pass.set_pipeline(scan);
            for (index, _) in terrain.list.drawn(group, streams) {
                pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            cull_group(&mut pass, terrain, group, scatter, streams);
        } else {
            cull_group(&mut pass, terrain, group, compacting, streams);
        }
    }
}

pub(super) const CULL_THREADS: u32 = 32;

/// `cull.wgsl` strides over the group arena, so the dispatch is capped by the device instead of
/// sized by the world and a bigger render distance can no longer walk it into wgpu's 65535
/// workgroups per dimension. wgpu reports no core count, so the cap is stated in invocations and
/// clamped by the limits it does report; a workgroup left with no group exits at once, but not
/// for free, so a draw holding fewer groups than the cap still dispatches only what it needs.
pub(super) fn cull_grid(limits: &WgpuLimits) -> u32 {
    const RESIDENT_INVOCATIONS: u32 = 1 << 19;
    let threads = CULL_THREADS
        .min(limits.max_compute_invocations_per_workgroup)
        .max(1);
    (RESIDENT_INVOCATIONS / threads)
        .min(limits.max_compute_workgroups_per_dimension)
        .max(1)
}

fn cull_group<'pass>(
    pass: &mut ComputePass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    pipeline: &'pass ComputePipeline,
    streams: &Streams,
) {
    pass.set_pipeline(pipeline);
    let mut open = None;
    for (index, draw) in terrain.list.drawn(group, streams) {
        if open != Some(draw.stream) {
            if open.is_some() {
                pass.pop_debug_group();
            }
            pass.push_debug_group(STREAM_NAMES[draw.stream as usize]);
            open = Some(draw.stream);
        }
        pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
        pass.dispatch_workgroups(
            draw.group_count
                .div_ceil(CULL_THREADS)
                .min(terrain.cull_grid),
            1,
            1,
        );
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

#[derive(SystemParam)]
pub(super) struct FrameParams<'w> {
    pipeline_cache: Res<'w, PipelineCache>,
    device: Res<'w, RenderDevice>,
    triangles: Res<'w, DrawnTriangles>,
    queries: Option<Res<'w, Queries>>,
    timings: Res<'w, GpuTimings>,
    streams: Res<'w, Streams>,
    wireframe: Res<'w, Wireframe>,
    raster: Res<'w, Raster>,
    counts: Res<'w, FrameCounts>,
    heat: Option<Res<'w, Heat>>,
}

/// The whole frame from one system, so it records into one encoder and submits one command
/// buffer: uploads, the timestamp resolve, the cull dispatch, then one render pass holding the
/// sky, the opaque terrain, the clouds and the blended terrain in that order.
pub(super) fn draw_frame(
    view: ViewQuery<(
        &ViewTarget,
        &ViewDepthTexture,
        &ExtractedView,
        &ViewUniformOffset,
    )>,
    mut uploads: UploadParams,
    frame: FrameParams,
    sky: SkyDraws,
    mut ctx: RenderContext,
) {
    let (target, depth, extracted, view_offset) = view.into_inner();
    apply_uploads(&mut uploads, ctx.command_encoder());
    probe::resolve(
        frame.queries.as_deref(),
        &frame.timings,
        ctx.command_encoder(),
    );
    if let Some(heat) = frame.heat.as_deref() {
        heat.dispatch(
            &frame.pipeline_cache,
            frame.queries.as_deref(),
            &frame.timings,
            ctx.command_encoder(),
        );
    }
    if let Some(terrain) = uploads.terrain.as_deref_mut()
        && terrain.hiz.fit(depth, &frame.device, &frame.pipeline_cache)
    {
        terrain.binds.rebuild_cull(
            &terrain.arenas,
            &terrain.frame,
            &terrain.hiz.view,
            &frame.device,
            &frame.pipeline_cache,
        );
    }
    let terrain = uploads
        .terrain
        .as_deref()
        .filter(|terrain| terrain.pipelines.ready());
    if let Some(terrain) = terrain {
        cull_terrain(
            terrain,
            &frame.pipeline_cache,
            &frame.triangles,
            frame.queries.as_deref(),
            &frame.timings,
            &frame.streams,
            ctx.command_encoder(),
        );
    } else {
        frame.counts.set_terrain_draws(0);
    }

    let color_attachments = [Some(target.get_color_attachment())];
    let depth_attachment = Some(depth.get_attachment(StoreOp::Store));
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let timestamps = frame
        .queries
        .as_deref()
        .map(|q| q.render(probe::WORLD, &frame.timings));
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("world"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: timestamps,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "world");

    sky.draw_sky(&mut pass, view_offset.offset, &frame.pipeline_cache);

    if let Some(terrain) = terrain {
        if frame.raster.0 < 1.0 {
            let size = extracted.viewport.zw().as_vec2() * frame.raster.0;
            pass.set_viewport(0.0, 0.0, size.x.max(1.0), size.y.max(1.0), 0.0, 1.0);
        }
        let mut draws = 0;
        for group in LayerGroup::ALL {
            if group == LayerGroup::Translucent {
                draws += sky.draw_clouds(&mut pass, view_offset.offset, &frame.pipeline_cache);
            }
            draws += draw_layer_group(
                &mut pass,
                terrain,
                group,
                0,
                &frame.pipeline_cache,
                &frame.streams,
                frame.wireframe.0,
            );
        }
        frame.counts.set_terrain_draws(draws);
    }

    span.end(&mut pass);
    drop(pass);
    let Some(terrain) = terrain else {
        return;
    };
    terrain.hiz.build(
        &frame.pipeline_cache,
        frame.queries.as_deref(),
        &frame.timings,
        ctx.command_encoder(),
    );
    let Some(second) = frame
        .pipeline_cache
        .get_compute_pipeline(terrain.pipelines.cull_second)
    else {
        return;
    };
    {
        let timestamps = frame
            .queries
            .as_deref()
            .map(|q| q.compute(probe::CULL_SECOND, &frame.timings));
        let mut pass = ctx.command_encoder().begin_compute_pass(&ComputePassDescriptor {
            label: Some("terrain cull second"),
            timestamp_writes: timestamps,
        });
        pass.set_bind_group(1, &terrain.binds.cull, &[]);
        for group in LayerGroup::ALL {
            cull_group(&mut pass, terrain, group, second, &frame.streams);
        }
    }
    let color_attachments = [Some(RenderPassColorAttachment {
        view: target.main_texture_view(),
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Load,
            store: StoreOp::Store,
        },
    })];
    let timestamps = frame
        .queries
        .as_deref()
        .map(|q| q.render(probe::WORLD_SECOND, &frame.timings));
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("world second"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: depth.view(),
            depth_ops: Some(Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: timestamps,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "world second");
    if frame.raster.0 < 1.0 {
        let size = extracted.viewport.zw().as_vec2() * frame.raster.0;
        pass.set_viewport(0.0, 0.0, size.x.max(1.0), size.y.max(1.0), 0.0, 1.0);
    }
    let mut draws = 0;
    for group in LayerGroup::ALL {
        draws += draw_layer_group(
            &mut pass,
            terrain,
            group,
            1,
            &frame.pipeline_cache,
            &frame.streams,
            frame.wireframe.0,
        );
    }
    frame.counts.set_terrain_draws(frame.counts.draws().0 + draws);
    span.end(&mut pass);
}

/// `phase` picks the first pass's args or the second's, which follow them.
fn draw_layer_group<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    phase: u64,
    pipeline_cache: &'pass PipelineCache,
    streams: &Streams,
    wireframe: bool,
) -> u32 {
    pass.set_bind_group(1, &terrain.binds.draw, &[]);
    let mut open = None;
    let mut draws = 0;
    for (index, draw) in terrain.list.drawn(group, streams) {
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
        pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
        pass.set_index_buffer(terrain.arenas.indices.slice(..), IndexFormat::Uint32);
        pass.draw_indexed_indirect(
            &terrain.frame.args,
            (phase * STREAMS as u64 + index as u64) * DRAW_ARGS_SIZE,
        );
        draws += 1;
    }
    if open.is_some() {
        pass.pop_debug_group();
    }
    draws
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cull_grid_fits_a_dispatch_whatever_the_device_reports() {
        for limits in [
            WgpuLimits::downlevel_webgl2_defaults(),
            WgpuLimits::downlevel_defaults(),
            WgpuLimits::default(),
            WgpuLimits {
                max_compute_workgroups_per_dimension: 1,
                ..WgpuLimits::default()
            },
        ] {
            let grid = cull_grid(&limits);
            assert!(grid >= 1);
            assert!(grid <= limits.max_compute_workgroups_per_dimension.max(1));
        }
    }
}
