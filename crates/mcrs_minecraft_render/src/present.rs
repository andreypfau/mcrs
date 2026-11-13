use bevy::prelude::Res;
use bevy::render::render_resource::binding_types::texture_2d;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::ViewTarget;

use crate::gbuffer::DeferredFrame;
use crate::pipeline::DeferredPipelines;

pub(crate) fn present_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "present",
        &BindGroupLayoutEntries::single(
            ShaderStages::FRAGMENT,
            texture_2d(TextureSampleType::Float { filterable: false }),
        ),
    )
}

pub(crate) fn present_bind_group(
    world: &TextureView,
    device: &RenderDevice,
    cache: &PipelineCache,
) -> BindGroup {
    device.create_bind_group(
        "present",
        &cache.get_bind_group_layout(&present_layout()),
        &BindGroupEntries::single(world),
    )
}

/// Copies the world image into the view target, the one place its image values are encoded.
pub(crate) fn present(
    view: ViewQuery<&ViewTarget>,
    frame: Option<Res<DeferredFrame>>,
    pipelines: Res<DeferredPipelines>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let Some(frame) = frame.as_deref() else {
        return;
    };
    let Some(pipeline) = pipelines
        .present
        .and_then(|id| cache.get_render_pipeline(id))
    else {
        return;
    };
    let target = view.into_inner();
    let color_attachments = [Some(target.get_color_attachment())];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("present"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &frame.present, &[]);
    pass.draw(0..3, 0..1);
}
