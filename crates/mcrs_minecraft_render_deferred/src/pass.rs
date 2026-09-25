use std::sync::atomic::Ordering;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset};
use mcrs_minecraft_render::sky::SkyDraws;
use mcrs_minecraft_render::{
    DepthDisplay, DepthSource, FrameCounts, LayerGroup, Occlusion, SelectedView, Streams, Terrain,
    draw_layer_group, stream_slot,
};

use crate::ParityMask;
use crate::gbuffer::DeferredFrame;
use crate::pipelines::DeferredPipelines;
use crate::views::{DeferredViews, Display, Lighting};

type WorldView = (
    &'static ViewTarget,
    &'static ViewDepthTexture,
    &'static ExtractedView,
    &'static ViewUniformOffset,
);

#[derive(SystemParam)]
pub(crate) struct DeferredParams<'w> {
    terrain: Option<Res<'w, Terrain>>,
    frame: Option<Res<'w, DeferredFrame>>,
    pipelines: Res<'w, DeferredPipelines>,
    cache: Res<'w, PipelineCache>,
    streams: Res<'w, Streams>,
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
            &self.streams,
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
    if let Some(terrain) = params.terrain.as_deref().filter(|terrain| terrain.ready()) {
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
}

/// The fragment leaves every pixel at depth 0 alone, which is the sky.
pub(crate) fn draw_lighting(
    view: ViewQuery<WorldView>,
    params: DeferredParams,
    display: Res<DepthDisplay>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(frame)) = (params.terrain.as_deref(), params.frame.as_deref()) else {
        return;
    };
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
    mask: Res<ParityMask>,
    mut ctx: RenderContext,
) {
    let Some(terrain) = params.terrain.as_deref().filter(|terrain| terrain.ready()) else {
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
    // Clouds carry no corner light, so in the parity mask they would only hide faces.
    let mut draws = if mask.0 {
        0
    } else {
        sky.draw_clouds(&mut pass, view_offset.offset, &params.cache)
    };
    for &phase in phases {
        draws += draw_layer_group(
            &mut pass,
            terrain,
            LayerGroup::Translucent,
            phase,
            &params.streams,
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
