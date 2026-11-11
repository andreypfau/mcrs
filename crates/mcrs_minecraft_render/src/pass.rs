use std::sync::atomic::Ordering;

use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthStencilTexture, ViewTarget, ViewUniformOffset};

use mcrs_minecraft_mesh::{GROUP_QUADS, Group, STREAMS};

use super::cull::stream_name;
use super::draws::PARAMS_STRIDE;
use super::gbuffer::DeferredFrame;
use super::layer::LayerGroup;
use super::pipeline::{DeferredPipelines, stream_slot};
use super::show::{DepthDisplay, DepthSource};
use super::sky::SkyDraws;
use super::stats::{DRAW_ARGS_SIZE, FrameCounts, VERTICES_PER_QUAD};
use super::terrain::Terrain;
use super::upload::{UploadParams, apply_uploads};
use super::views::{DeferredViews, Display, Lighting, SelectedView};
use super::{DrawMask, LightTint, Occlusion, PassTimestamps};

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
    &'static ViewDepthStencilTexture,
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
        && let Some(cull) = &mut terrain.cull
        && cull.hiz.fit(depth, &frame.device, &frame.pipeline_cache)
    {
        cull.rebind(&terrain.arenas, &frame.device, &frame.pipeline_cache);
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
    let Some(cull) = &terrain.cull else {
        return;
    };
    cull.cull(
        &terrain.binds,
        &terrain.list,
        terrain.budget.sections,
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
    let Some(terrain) = terrain.as_deref() else {
        return;
    };
    let Some(cull) = terrain
        .cull
        .as_ref()
        .filter(|_| frame.occlusion.0 && terrain.list.visible_entries != 0)
    else {
        return;
    };
    cull.cull_second(
        &terrain.binds,
        &terrain.list,
        &frame.pipeline_cache,
        &frame.timestamps,
        &frame.mask,
        ctx.command_encoder(),
    );
}

/// Vertices of one instance on the minimal path: room for every quad a group may hold.
const GROUP_VERTICES: u32 = GROUP_QUADS as u32 * VERTICES_PER_QUAD;
const GROUP_BYTES: u64 = size_of::<Group>() as u64;

/// `phase` picks the first pass's args or the second's, which follow them; the minimal path has
/// one phase and draws every group of a draw in it. A stream `pipeline_of` has no pipeline for is
/// skipped.
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
            pass.push_debug_group(stream_name(draw.stream));
            open = Some(draw.stream);
        }
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
        match (&terrain.cull, terrain.arenas.groups.words.buffer()) {
            (Some(cull), _) => pass.draw_indirect(
                cull.args(),
                (phase * STREAMS as u64 + index as u64) * DRAW_ARGS_SIZE,
            ),
            (None, Some(groups)) => {
                // The draw's block of the group arena is its instances, from instance zero: a
                // first instance is not something every backend adds to the instance index.
                let first = u64::from(draw.first_group) * GROUP_BYTES;
                let end = first + u64::from(draw.group_count) * GROUP_BYTES;
                pass.set_vertex_buffer(0, groups.slice(first..end));
                pass.draw(0..GROUP_VERTICES, 0..draw.group_count);
            }
            (None, None) => continue,
        }
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
    depth: &ViewDepthStencilTexture,
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
            view: depth.attachment.depth_stencil_views().attachment_view(),
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
    if let Some(cull) = &terrain.cull {
        cull.hiz.build(&params.cache, None, ctx.command_encoder());
    }
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
