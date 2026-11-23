use std::sync::atomic::Ordering;

use bevy::camera::ClearColorConfig;
use bevy::color::{LinearRgba, Srgba};
use bevy::core_pipeline::core_3d::{AlphaMask3d, Opaque3d};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthStencilTexture, ViewTarget, ViewUniformOffset};

use mcrs_minecraft_mesh::{DrawKind, GROUP_QUADS, Group};

use super::draws::PARAMS_STRIDE;
use super::gbuffer::DeferredFrame;
use super::layer::LayerGroup;
use super::pipeline::{DeferredPipelines, TransparencyPipelines, terrain_slot};
use super::shared_types::DrawArgRow;
use super::show::{DepthDisplay, DepthSource};
use super::sky::SkyDraws;
use super::stats::{FrameCounts, VERTICES_PER_QUAD};
use super::terrain::Terrain;
use super::transparency_targets::TransparencyTargets;
use super::upload::{UploadParams, apply_uploads};
use super::views::{DeferredViews, Display, Lighting, SelectedView};
use super::{DrawMask, LightTint, Occlusion, PassTimestamps, WorldFog};

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "Bevy queues items into the binned phases of every view each frame, and the world pass never draws them"
    )
)]
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

const GROUP_VERTICES: u32 = GROUP_QUADS as u32 * VERTICES_PER_QUAD;
const GROUP_BYTES: u64 = size_of::<Group>() as u64;

pub fn draw_layer_group<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    group: LayerGroup,
    row: DrawArgRow,
    mask: &DrawMask,
    pipeline_of: impl Fn(DrawKind) -> Option<&'pass RenderPipeline>,
) -> u32 {
    pass.set_bind_group(1, &terrain.binds.draw, &[]);
    let mut open = None;
    let mut draws = 0;
    for (index, draw) in terrain.list.drawn(group, mask) {
        let Some(pipeline) = pipeline_of(draw.kind) else {
            continue;
        };
        if open != Some(draw.kind) {
            if open.is_some() {
                pass.pop_debug_group();
            }
            pass.push_debug_group(draw.kind.label());
            open = Some(draw.kind);
        }
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &terrain.binds.view, &[index as u32 * PARAMS_STRIDE]);
        match (&terrain.cull, terrain.arenas.groups.words.buffer()) {
            (Some(cull), _) => pass.draw_indirect(cull.args(), row.offset_bytes(draw.kind)),
            (None, Some(groups)) => {
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
    sky: SkyDraws<'w>,
}

struct Transparency<'a> {
    targets: &'a TransparencyTargets,
    pipelines: &'a TransparencyPipelines,
}

impl DeferredParams<'_> {
    fn transparency(&self) -> Option<Transparency<'_>> {
        let targets = self.frame.as_deref()?.transparency.as_ref()?;
        let pipelines = self.pipelines.transparency.as_ref()?;
        (self.display().lighting == Lighting::Lit
            && pipelines.ready(&self.cache)
            && self.sky.cloud_image_ready(&self.cache))
        .then_some(Transparency { targets, pipelines })
    }

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
        row: DrawArgRow,
    ) {
        let wireframe = self.display().wireframe;
        let draws = draw_layer_group(pass, terrain, LayerGroup::Opaque, row, &self.mask, |kind| {
            self.pipelines
                .terrain(terrain_slot(kind, wireframe), &self.cache)
        });
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

fn color_attachment(
    view: &TextureView,
    load: LoadOp<wgpu::Color>,
) -> RenderPassColorAttachment<'_> {
    RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load,
            store: StoreOp::Store,
        },
    }
}

pub(crate) fn loaded(view: &TextureView) -> RenderPassColorAttachment<'_> {
    color_attachment(view, LoadOp::Load)
}

type SkyView = (
    &'static ViewDepthStencilTexture,
    &'static ViewUniformOffset,
    &'static ExtractedCamera,
);

pub(crate) fn draw_gbuffer(
    view: ViewQuery<SkyView>,
    params: DeferredParams,
    fog: Option<Res<WorldFog>>,
    clear_color: Res<ClearColor>,
    mut ctx: RenderContext,
) {
    let Some(frame) = params.frame.as_deref() else {
        return;
    };
    let (depth, view_offset, camera) = view.into_inner();
    let transparency = params.transparency();
    let color_attachments = [Some(match &transparency {
        Some(transparency) => {
            let [red, green, blue] = fog.map_or([0.0; 3], |fog| fog.color);
            let clear = Srgba::rgb(red, green, blue);
            color_attachment(&transparency.targets.sky, LoadOp::Clear(clear.into()))
        }
        None => {
            let clear = match camera.clear_color {
                ClearColorConfig::Custom(color) => Some(color),
                ClearColorConfig::None => None,
                _ => Some(clear_color.0),
            };
            let load = clear.map_or(LoadOp::Load, |color| {
                LoadOp::Clear(Srgba::from(color).into())
            });
            color_attachment(&frame.world, load)
        }
    })];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("sky"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    params
        .sky
        .draw_sky(&mut pass, view_offset.offset, &params.cache);
    drop(pass);

    if let Some(transparency) = &transparency {
        let color_attachments = [Some(color_attachment(
            &transparency.targets.clouds,
            LoadOp::Clear(LinearRgba::NONE.into()),
        ))];
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("cloud image"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &transparency.targets.cloud_depth,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(0.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        params
            .sky
            .draw_cloud_image(&mut pass, view_offset.offset, &params.cache);
    }

    let mut pass = gbuffer_pass(&mut ctx, "gbuffer", frame, depth, true);
    if let Some(terrain) = params.terrain.as_deref() {
        params.draw_opaque(&mut pass, terrain, DrawArgRow::FirstPassDraws);
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
    params.draw_opaque(&mut pass, terrain, DrawArgRow::SecondPassDraws);
    drop(pass);
    if let Some(cull) = &terrain.cull {
        cull.hiz.build(&params.cache, None, ctx.command_encoder());
    }
}

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
    let (_, depth, _, _) = view.into_inner();
    let transparency = params.transparency();
    let target = &frame.world;
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
        Lighting::Lit => match &transparency {
            Some(transparency) => params
                .cache
                .get_render_pipeline(transparency.pipelines.lighting),
            None => params
                .pipelines
                .lighting
                .and_then(|id| params.cache.get_render_pipeline(id)),
        },
    };
    let Some(pipeline) = pipeline else {
        return;
    };
    let color_attachments = [Some(loaded(target))];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("lighting"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let is_gbuffer_view = matches!(
        params.display().lighting,
        Lighting::Variant(variant) if variant.is_gbuffer_view()
    );
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, terrain.view_bind_group(), &[0]);
    if is_gbuffer_view {
        pass.set_bind_group(1, &frame.bind_group, &[]);
    } else {
        pass.set_bind_group(1, terrain.draw_bind_group(), &[]);
        let gbuffer = transparency
            .as_ref()
            .map_or(&frame.bind_group, |transparency| {
                &transparency.targets.lighting
            });
        pass.set_bind_group(2, gbuffer, &[]);
        if let Some(tint) = tint {
            pass.set_bind_group(3, tint, &[]);
        }
    }
    pass.draw(0..3, 0..1);
}

pub(crate) fn draws_forward(params: DeferredParams) -> bool {
    params.display().forward
}

pub(crate) fn draw_forward_deferred(
    view: ViewQuery<WorldView>,
    params: DeferredParams,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(frame)) = (params.terrain.as_deref(), params.frame.as_deref()) else {
        return;
    };
    let rows: &[DrawArgRow] = if terrain.second_pass(&params.occlusion, &params.cache) {
        &[DrawArgRow::FirstPassDraws, DrawArgRow::SecondPassDraws]
    } else {
        &[DrawArgRow::FirstPassDraws]
    };
    let (_, depth, _, view_offset) = view.into_inner();
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let color_attachments = [Some(loaded(&frame.world))];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("forward"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Discard)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "forward");
    let transparency = params.transparency();
    let mut draws = match &transparency {
        Some(transparency) => {
            pass.set_bind_group(2, &transparency.targets.forward, &[]);
            0
        }
        None => params
            .sky
            .draw_clouds(&mut pass, view_offset.offset, &params.cache),
    };
    for &row in rows {
        draws += draw_layer_group(
            &mut pass,
            terrain,
            LayerGroup::Translucent,
            row,
            &params.mask,
            |kind| match &transparency {
                Some(transparency) => transparency
                    .pipelines
                    .translucent(kind.format, &params.cache),
                None => params
                    .pipelines
                    .terrain(terrain_slot(kind, false), &params.cache),
            },
        );
    }
    params
        .counts
        .terrain_draws
        .fetch_add(draws, Ordering::Relaxed);
    span.end(&mut pass);
}
