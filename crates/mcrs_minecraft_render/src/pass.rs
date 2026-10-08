use std::sync::atomic::Ordering;

use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset};

use mcrs_minecraft_mesh::{STREAM_NAMES, STREAMS};
use wgpu::ComputePassTimestampWrites;

use super::draws::PARAMS_STRIDE;
use super::gbuffer::DeferredFrame;
use super::layer::LayerGroup;
use super::pipeline::{DeferredPipelines, stream_slot};
use super::show::{DepthDisplay, DepthSource};
use super::sky::SkyDraws;
use super::stats::{DISPATCH_BYTES, DISPATCHES, DRAW_ARGS_SIZE, FrameCounts};
use super::terrain::Terrain;
use super::upload::{UploadParams, apply_uploads};
use super::views::{DeferredViews, Display, Lighting, SelectedView};
use super::{DrawMask, LightTint, Occlusion, PassSlot, PassTimestamps};

fn cull_terrain(
    terrain: &Terrain,
    pipeline_cache: &PipelineCache,
    timestamps: &PassTimestamps,
    mask: &DrawMask,
    encoder: &mut CommandEncoder,
) {
    let pipelines = &terrain.pipelines;
    let (Some(sections), Some(groups), Some(quads), Some(count), Some(scan), Some(scatter)) = (
        pipeline_cache.get_compute_pipeline(pipelines.cull_sections),
        pipeline_cache.get_compute_pipeline(pipelines.cull_groups),
        pipeline_cache.get_compute_pipeline(pipelines.cull_quads),
        pipeline_cache.get_compute_pipeline(pipelines.cull_count),
        pipeline_cache.get_compute_pipeline(pipelines.cull_scan),
        pipeline_cache.get_compute_pipeline(pipelines.cull_scatter),
    ) else {
        return;
    };

    let reset = terrain.frame.args_reset.size();
    encoder.copy_buffer_to_buffer(&terrain.frame.args_reset, 0, &terrain.frame.args, 0, reset);

    {
        let mut pass = timed_compute(
            encoder,
            "terrain cull sections",
            timestamps.compute(PassSlot::CullSections),
        );
        pass.set_pipeline(sections);
        pass.set_bind_group(0, &terrain.binds.view, &[0]);
        pass.set_bind_group(1, &terrain.binds.quad_cull, &[]);
        pass.dispatch_workgroups(
            (terrain.budget.sections as u32)
                .div_ceil(CULL_THREADS)
                .min(terrain.cull_grid),
            1,
            1,
        );
    }
    // Blending is not commutative, so translucent draws have to reach the rasteriser in the order
    // the list holds them; opaque ones may be compacted.
    {
        let mut pass = timed_compute(
            encoder,
            "terrain cull blended",
            timestamps.compute(PassSlot::CullBlended),
        );
        pass.set_bind_group(1, &terrain.binds.cull, &[]);
        let blended = LayerGroup::Translucent;
        cull_group(
            &mut pass,
            terrain,
            blended,
            count,
            mask,
            CULL_THREADS,
            terrain.cull_grid,
        );
        pass.set_pipeline(scan);
        for (index, _) in terrain.list.drawn(blended, mask) {
            pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        cull_group(
            &mut pass,
            terrain,
            blended,
            scatter,
            mask,
            CULL_THREADS,
            terrain.cull_grid,
        );
    }
    {
        let mut pass = timed_compute(encoder, "terrain cull", timestamps.compute(PassSlot::Cull));
        pass.set_bind_group(1, &terrain.binds.quad_cull, &[]);
        let opaque = LayerGroup::Opaque;
        cull_group(
            &mut pass,
            terrain,
            opaque,
            groups,
            mask,
            GROUP_THREADS,
            terrain.group_grid,
        );
    }
    copy_dispatches(terrain, encoder);
    {
        let mut pass = timed_compute(
            encoder,
            "terrain cull quads",
            timestamps.compute(PassSlot::CullQuads),
        );
        pass.set_bind_group(1, &terrain.binds.quad_cull, &[]);
        cull_indirect(
            &mut pass,
            terrain,
            LayerGroup::Opaque,
            quads,
            mask,
            QUADS_FIRST,
        );
    }
}

/// Where each kernel reading a list finds its dispatch size in the copied table.
const QUADS_FIRST: u64 = 0;
const QUADS_SECOND: u64 = STREAMS as u64;
const GROUPS_SECOND: u64 = 2 * STREAMS as u64;

fn copy_dispatches(terrain: &Terrain, encoder: &mut CommandEncoder) {
    encoder.copy_buffer_to_buffer(
        &terrain.frame.args,
        DISPATCHES,
        &terrain.frame.dispatch,
        0,
        DISPATCH_BYTES,
    );
}

/// Dispatches `pipeline` once a draw of `group`, as many workgroups as the cull counted into the
/// draw's row of `table`.
fn cull_indirect<'pass>(
    pass: &mut ComputePass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    pipeline: &'pass ComputePipeline,
    mask: &DrawMask,
    table: u64,
) {
    pass.set_pipeline(pipeline);
    for (index, draw) in terrain.list.drawn(group, mask) {
        pass.push_debug_group(STREAM_NAMES[draw.stream as usize]);
        pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
        pass.dispatch_workgroups_indirect(
            &terrain.frame.dispatch,
            (table + index as u64) * DRAW_ARGS_SIZE,
        );
        pass.pop_debug_group();
    }
}

fn timed_compute<'e>(
    encoder: &'e mut CommandEncoder,
    label: &'static str,
    timestamp_writes: Option<ComputePassTimestampWrites<'_>>,
) -> ComputePass<'e> {
    encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some(label),
        timestamp_writes,
    })
}

pub(super) const CULL_THREADS: u32 = 32;
pub(super) const GROUP_THREADS: u32 = 256;
/// Groups a quad-cull workgroup tests at once.
pub(super) const GROUPS_PER_STEP: u32 = 2;

// The quad cull gives each quad of a group a lane, and each quad a pass leaves a bit of one word.
const _: () =
    assert!(mcrs_minecraft_mesh::GROUP_QUADS <= CULL_THREADS as usize && CULL_THREADS <= 32);

/// `cull.wgsl` strides over the group arena, so the dispatch is capped by the device instead of
/// sized by the world and a bigger render distance can no longer walk it into wgpu's 65535
/// workgroups per dimension. wgpu reports no core count, so the cap is stated in invocations and
/// clamped by the limits it does report; a workgroup left with no group exits at once, but not
/// for free, so a draw holding fewer groups than the cap still dispatches only what it needs.
pub(super) fn cull_grid(limits: &WgpuLimits, threads: u32) -> u32 {
    const RESIDENT_INVOCATIONS: u32 = 1 << 19;
    let threads = threads
        .min(limits.max_compute_invocations_per_workgroup)
        .max(1);
    (RESIDENT_INVOCATIONS / threads)
        .min(limits.max_compute_workgroups_per_dimension)
        .max(1)
}

/// Dispatches `pipeline` once a draw of `group`, a workgroup to every `per_workgroup` groups up to
/// `grid` workgroups.
fn cull_group<'pass>(
    pass: &mut ComputePass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    pipeline: &'pass ComputePipeline,
    mask: &DrawMask,
    per_workgroup: u32,
    grid: u32,
) {
    pass.set_pipeline(pipeline);
    let mut open = None;
    for (index, draw) in terrain.list.drawn(group, mask) {
        if open != Some(draw.stream) {
            if open.is_some() {
                pass.pop_debug_group();
            }
            pass.push_debug_group(STREAM_NAMES[draw.stream as usize]);
            open = Some(draw.stream);
        }
        pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
        pass.dispatch_workgroups(draw.group_count.div_ceil(per_workgroup).min(grid), 1, 1);
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
    mask: Res<'w, DrawMask>,
    occlusion: Res<'w, Occlusion>,
    counts: Res<'w, FrameCounts>,
    timestamps: Res<'w, PassTimestamps>,
}

type WorldView = (
    &'static ViewTarget,
    &'static ViewDepthTexture,
    &'static ExtractedView,
    &'static ViewUniformOffset,
);

pub(super) fn upload_frame(
    view: ViewQuery<WorldView>,
    mut uploads: UploadParams,
    frame: FrameParams,
    mut ctx: RenderContext,
) {
    let (_, depth, _, _) = view.into_inner();
    apply_uploads(&mut uploads, ctx.command_encoder());
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
    frame.counts.terrain_draws.store(0, Ordering::Relaxed);
}

pub(super) fn cull_frame(
    _view: ViewQuery<WorldView>,
    terrain: Option<Res<Terrain>>,
    frame: FrameParams,
    mut ctx: RenderContext,
) {
    let Some(terrain) = terrain.as_deref() else {
        return;
    };
    cull_terrain(
        terrain,
        &frame.pipeline_cache,
        &frame.timestamps,
        &frame.mask,
        ctx.command_encoder(),
    );
}

pub(super) fn build_occlusion(
    _view: ViewQuery<WorldView>,
    terrain: Option<Res<Terrain>>,
    frame: FrameParams,
    mut ctx: RenderContext,
) {
    let Some(terrain) = terrain
        .as_deref()
        .filter(|terrain| frame.occlusion.0 && terrain.list.visible_entries != 0)
    else {
        return;
    };
    terrain.hiz.build(
        &frame.pipeline_cache,
        frame.timestamps.compute(PassSlot::Hiz),
        ctx.command_encoder(),
    );
    let (Some(groups), Some(quads)) = (
        frame
            .pipeline_cache
            .get_compute_pipeline(terrain.pipelines.cull_groups_second),
        frame
            .pipeline_cache
            .get_compute_pipeline(terrain.pipelines.cull_quads_second),
    ) else {
        return;
    };
    let encoder = ctx.command_encoder();
    copy_dispatches(terrain, encoder);
    {
        let mut pass = timed_compute(
            encoder,
            "terrain cull second",
            frame.timestamps.compute(PassSlot::CullSecond),
        );
        pass.set_bind_group(1, &terrain.binds.quad_cull, &[]);
        for group in LayerGroup::ALL {
            cull_indirect(
                &mut pass,
                terrain,
                group,
                groups,
                &frame.mask,
                GROUPS_SECOND,
            );
        }
    }
    copy_dispatches(terrain, encoder);
    let mut pass = timed_compute(
        encoder,
        "terrain cull quads second",
        frame.timestamps.compute(PassSlot::CullQuadsSecond),
    );
    pass.set_bind_group(1, &terrain.binds.quad_cull, &[]);
    for group in LayerGroup::ALL {
        cull_indirect(&mut pass, terrain, group, quads, &frame.mask, QUADS_SECOND);
    }
}

/// `phase` picks the first pass's args or the second's, which follow them. A stream `pipeline_of`
/// has no pipeline for is skipped.
pub fn draw_layer_group<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    phase: u64,
    mask: &DrawMask,
    pipeline_of: impl Fn(u32) -> Option<&'pass RenderPipeline>,
) -> u32 {
    pass.set_bind_group(1, &terrain.binds.draw, &[]);
    let mut open = None;
    let mut draws = 0;
    for (index, draw) in terrain.list.drawn(group, mask) {
        let Some(pipeline) = pipeline_of(draw.stream) else {
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
        pass.draw_indirect(
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

#[derive(SystemParam)]
pub(crate) struct DeferredParams<'w> {
    terrain: Option<Res<'w, Terrain>>,
    frame: Option<Res<'w, DeferredFrame>>,
    pipelines: Res<'w, DeferredPipelines>,
    cache: Res<'w, PipelineCache>,
    mask: Res<'w, DrawMask>,
    occlusion: Res<'w, Occlusion>,
    counts: Res<'w, FrameCounts>,
    views: Res<'w, DeferredViews>,
    selected: Res<'w, SelectedView>,
}

impl DeferredParams<'_> {
    /// A view whose program is still compiling shows final shading.
    fn display(&self) -> Display {
        let display = self.views.display(*self.selected);
        match display.lighting {
            Lighting::Variant(variant)
                if self.pipelines.variant(variant, &self.cache).is_none() =>
            {
                Display::FINAL
            }
            _ => display,
        }
    }

    fn draw_opaque<'pass>(
        &'pass self,
        pass: &mut TrackedRenderPass<'pass>,
        terrain: &'pass Terrain,
        phase: u64,
    ) {
        let wireframe = self.display().wireframe;
        let draws = draw_layer_group(
            pass,
            terrain,
            LayerGroup::Opaque,
            phase,
            &self.mask,
            |stream| {
                self.pipelines
                    .terrain(stream_slot(stream, wireframe), &self.cache)
            },
        );
        self.counts
            .terrain_draws
            .fetch_add(draws, Ordering::Relaxed);
    }
}

fn gbuffer_pass<'a>(
    ctx: &'a mut RenderContext,
    label: &'static str,
    frame: &DeferredFrame,
    depth: &ViewDepthTexture,
    clear: bool,
) -> TrackedRenderPass<'a> {
    let color_attachments = frame.targets.each_ref().map(|view| {
        Some(RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: if clear {
                    LoadOp::Clear(default())
                } else {
                    LoadOp::Load
                },
                store: StoreOp::Store,
            },
        })
    });
    ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some(label),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: depth.view(),
            depth_ops: Some(Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// The sky is drawn into the view target first, in a pass of its own: its pipelines draw finite
/// geometry with no depth test, so it cannot go behind the lit terrain later.
pub(crate) fn draw_gbuffer(
    view: ViewQuery<WorldView>,
    params: DeferredParams,
    sky: SkyDraws,
    mut ctx: RenderContext,
) {
    let Some(frame) = params.frame.as_deref() else {
        return;
    };
    let (target, depth, _, view_offset) = view.into_inner();
    let color_attachments = [Some(target.get_color_attachment())];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("sky"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    sky.draw_sky(&mut pass, view_offset.offset, &params.cache);
    drop(pass);

    let mut pass = gbuffer_pass(&mut ctx, "gbuffer", frame, depth, true);
    if let Some(terrain) = params.terrain.as_deref() {
        params.draw_opaque(&mut pass, terrain, 0);
    }
}

pub(crate) fn draw_gbuffer_second(
    view: ViewQuery<WorldView>,
    params: DeferredParams,
    mut ctx: RenderContext,
) {
    let (Some(frame), Some(terrain)) = (params.frame.as_deref(), params.terrain.as_deref()) else {
        return;
    };
    if !terrain.second_pass(&params.occlusion, &params.cache) {
        return;
    }
    let (_, depth, _, _) = view.into_inner();
    let mut pass = gbuffer_pass(&mut ctx, "gbuffer second", frame, depth, false);
    params.draw_opaque(&mut pass, terrain, 1);
    drop(pass);
    // The next frame's first pass tests against this: built from the whole frame's depth,
    // what this pass revived is not hidden again there only to be revived once more.
    terrain
        .hiz
        .build(&params.cache, None, ctx.command_encoder());
}

/// The fragment leaves every pixel at depth 0 alone, which is the sky.
pub(crate) fn draw_lighting(
    view: ViewQuery<WorldView>,
    params: DeferredParams,
    tint: Option<Res<LightTint>>,
    display: Res<DepthDisplay>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(frame)) = (params.terrain.as_deref(), params.frame.as_deref()) else {
        return;
    };
    let tinted = params.pipelines.tinted;
    let tint = tint
        .as_deref()
        .and_then(|tint| tint.bind_group.as_ref())
        .filter(|_| tinted);
    if tinted && tint.is_none() {
        return;
    }
    let (target, depth, _, _) = view.into_inner();
    let pipeline = match params.display().lighting {
        Lighting::Depth => {
            let source = DepthSource::Depth(depth);
            return display.draw(&mut ctx, target, source, &params.cache, &device);
        }
        Lighting::Pyramid => {
            let source = DepthSource::Pyramid(terrain);
            return display.draw(&mut ctx, target, source, &params.cache, &device);
        }
        Lighting::Variant(variant) => params.pipelines.variant(variant, &params.cache),
        Lighting::Lit => params
            .pipelines
            .lighting
            .and_then(|id| params.cache.get_render_pipeline(id)),
    };
    let Some(pipeline) = pipeline else {
        return;
    };
    let color_attachments = [Some(target.get_color_attachment())];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("lighting"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, terrain.view_bind_group(), &[0]);
    pass.set_bind_group(1, terrain.draw_bind_group(), &[]);
    pass.set_bind_group(2, &frame.bind_group, &[]);
    if let Some(tint) = tint {
        pass.set_bind_group(3, tint, &[]);
    }
    pass.draw(0..3, 0..1);
}

pub(crate) fn draws_forward(params: DeferredParams) -> bool {
    params.display().forward
}

/// Drawn over the lit frame the way the classic path draws it, with the lighting pass's shading.
pub(crate) fn draw_forward_deferred(
    view: ViewQuery<WorldView>,
    params: DeferredParams,
    sky: SkyDraws,
    mut ctx: RenderContext,
) {
    let Some(terrain) = params.terrain.as_deref() else {
        return;
    };
    let phases: &[u64] = if terrain.second_pass(&params.occlusion, &params.cache) {
        &[0, 1]
    } else {
        &[0]
    };
    let (target, depth, _, view_offset) = view.into_inner();
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let color_attachments = [Some(target.get_color_attachment())];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("forward"),
        color_attachments: &color_attachments,
        // The last reader of this frame's depth: the GUI pass clears it next.
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Discard)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "forward");
    let mut draws = sky.draw_clouds(&mut pass, view_offset.offset, &params.cache);
    for &phase in phases {
        draws += draw_layer_group(
            &mut pass,
            terrain,
            LayerGroup::Translucent,
            phase,
            &params.mask,
            |stream| {
                params
                    .pipelines
                    .terrain(stream_slot(stream, false), &params.cache)
            },
        );
    }
    params
        .counts
        .terrain_draws
        .fetch_add(draws, Ordering::Relaxed);
    span.end(&mut pass);
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
            for threads in [CULL_THREADS, GROUP_THREADS] {
                let grid = cull_grid(&limits, threads);
                assert!(grid >= 1);
                assert!(grid <= limits.max_compute_workgroups_per_dimension.max(1));
            }
        }
    }
}
