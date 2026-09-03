use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d};
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderQueue, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset};

use crate::mesh::STREAM_NAMES;
use crate::probe::{self, GpuTimings, Queries};
use crate::sky_render::CloudDraw;

use super::draws::PARAMS_STRIDE;
use super::layer::LayerGroup;
use super::stats::{DRAW_ARGS_SIZE, DrawnTriangles, copy_args};
use super::terrain::Terrain;
use super::{Raster, Streams, Wireframe};

pub(super) fn extract_cave_visibility(
    cave: Extract<Res<crate::cave::CaveCull>>,
    terrain: Option<Res<Terrain>>,
    queue: Res<RenderQueue>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    if !cave.is_changed() {
        return;
    }
    queue.write_buffer(&terrain.frame.cave, 0, bytemuck::cast_slice(&cave.bits[..]));
}

pub(super) fn cull_terrain(
    terrain: Option<Res<Terrain>>,
    pipeline_cache: Res<PipelineCache>,
    triangles: Res<DrawnTriangles>,
    queries: Option<Res<Queries>>,
    timings: Res<GpuTimings>,
    streams: Res<Streams>,
    mut ctx: RenderContext,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let (Some(compacting), Some(stable)) = (
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull),
        pipeline_cache.get_compute_pipeline(terrain.pipelines.cull_stable),
    ) else {
        return;
    };

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
    for group in LayerGroup::ALL {
        cull_group(
            &mut pass,
            &terrain,
            group,
            if group.culls_in_order() {
                stable
            } else {
                compacting
            },
            &streams,
        );
    }
    span.end(&mut pass);
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
            draw.group_count.div_ceil(CULL_THREADS).min(terrain.cull_grid),
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

/// The clouds go in between the opaque and the blended terrain: after the opaque so the depth
/// test throws away every cloud pixel a hill or a tree already covers, before the blended so
/// water still lies over them.
pub(super) fn draw_terrain(
    view: ViewQuery<(
        &ViewTarget,
        &ViewDepthTexture,
        &ExtractedView,
        &ViewUniformOffset,
    )>,
    terrain: Option<Res<Terrain>>,
    streams: Res<Streams>,
    wireframe: Res<Wireframe>,
    raster: Res<Raster>,
    pipeline_cache: Res<PipelineCache>,
    queries: Option<Res<Queries>>,
    timings: Res<GpuTimings>,
    clouds: CloudDraw,
    mut ctx: RenderContext,
) {
    let Some(terrain) = terrain else {
        return;
    };
    if !terrain.pipelines.ready() {
        return;
    }
    let (target, depth, extracted, view_offset) = view.into_inner();
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

    for group in LayerGroup::ALL {
        if group == LayerGroup::Translucent {
            clouds.draw(&mut pass, view_offset.offset, &pipeline_cache);
        }
        draw_layer_group(
            &mut pass,
            &terrain,
            group,
            &pipeline_cache,
            &streams,
            wireframe.0,
        );
    }

    span.end(&mut pass);
}

fn draw_layer_group<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    pipeline_cache: &'pass PipelineCache,
    streams: &Streams,
    wireframe: bool,
) {
    pass.set_bind_group(1, &terrain.binds.draw, &[]);
    let mut open = None;
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
        pass.draw_indirect(&terrain.frame.args, index as u64 * DRAW_ARGS_SIZE);
    }
    if open.is_some() {
        pass.pop_debug_group();
    }
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
