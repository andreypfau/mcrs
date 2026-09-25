use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget, ViewUniformOffset};
use mcrs_minecraft_render::sky::SkyDraws;
use mcrs_minecraft_render::{
    FrameCounts, LayerGroup, Raster, Streams, Terrain, draw_layer_group, restrict_to_raster,
    stream_slot,
};
use std::sync::atomic::Ordering;

use crate::gbuffer::DeferredFrame;
use crate::pipelines::DeferredPipelines;

type WorldView = (
    &'static ViewTarget,
    &'static ViewDepthTexture,
    &'static ExtractedView,
    &'static ViewUniformOffset,
);

#[derive(SystemParam)]
pub(crate) struct GBufferParams<'w> {
    terrain: Option<Res<'w, Terrain>>,
    frame: Option<Res<'w, DeferredFrame>>,
    pipelines: Res<'w, DeferredPipelines>,
    cache: Res<'w, PipelineCache>,
    streams: Res<'w, Streams>,
    raster: Res<'w, Raster>,
    counts: Res<'w, FrameCounts>,
}

/// The sky is drawn into the view target first, in a pass of its own: its pipelines draw finite
/// geometry with no depth test, so it cannot go behind the lit terrain later.
pub(crate) fn draw_gbuffer(
    view: ViewQuery<WorldView>,
    params: GBufferParams,
    sky: SkyDraws,
    mut ctx: RenderContext,
) {
    let Some(frame) = params.frame.as_deref() else {
        return;
    };
    let (target, depth, extracted, view_offset) = view.into_inner();
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

    let color_attachments = frame.targets.each_ref().map(|view| {
        Some(RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(default()),
                store: StoreOp::Store,
            },
        })
    });
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("gbuffer"),
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
    });
    let Some(terrain) = params.terrain.as_deref().filter(|terrain| terrain.ready()) else {
        return;
    };
    restrict_to_raster(&mut pass, &params.raster, extracted);
    let draws = draw_layer_group(
        &mut pass,
        terrain,
        LayerGroup::Opaque,
        0,
        &params.streams,
        |stream| {
            params
                .pipelines
                .gbuffer(stream_slot(stream, false), &params.cache)
        },
    );
    params
        .counts
        .terrain_draws
        .fetch_add(draws, Ordering::Relaxed);
}

/// Not restricted to the raster viewport: the fragment leaves every pixel at depth 0 alone, which
/// is the sky and everything outside the viewport.
pub(crate) fn draw_lighting(
    view: ViewQuery<WorldView>,
    terrain: Option<Res<Terrain>>,
    frame: Option<Res<DeferredFrame>>,
    pipelines: Res<DeferredPipelines>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(frame)) = (terrain, frame) else {
        return;
    };
    let Some(pipeline) = pipelines
        .lighting
        .and_then(|id| cache.get_render_pipeline(id))
    else {
        return;
    };
    let (target, _, _, _) = view.into_inner();
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
