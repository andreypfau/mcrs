use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{texture_2d, texture_depth_2d};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget};
use bevy::shader::ShaderDefVal;

use crate::pipeline::common;
use crate::terrain::Terrain;

/// Draws reverse-Z depth over the whole view as greyscale: near white, far dark, sky black.
#[derive(Resource)]
pub struct DepthDisplay {
    shader: Handle<Shader>,
    depth_layout: BindGroupLayoutDescriptor,
    pyramid_layout: BindGroupLayoutDescriptor,
    depth: Option<CachedRenderPipelineId>,
    pyramid: Option<CachedRenderPipelineId>,
}

pub enum DepthSource<'a> {
    Depth(&'a ViewDepthTexture),
    /// The first level of the terrain's depth pyramid.
    Pyramid(&'a Terrain),
}

impl DepthDisplay {
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        target: &ViewTarget,
        source: DepthSource,
        cache: &PipelineCache,
        device: &RenderDevice,
    ) {
        let (pipeline, layout, texture) = match source {
            DepthSource::Depth(depth) => (
                self.depth,
                &self.depth_layout,
                depth.texture.create_view(&TextureViewDescriptor {
                    label: Some("show depth"),
                    aspect: TextureAspect::DepthOnly,
                    ..default()
                }),
            ),
            DepthSource::Pyramid(terrain) => {
                (self.pyramid, &self.pyramid_layout, terrain.hiz.view.clone())
            }
        };
        let Some(pipeline) = pipeline.and_then(|id| cache.get_render_pipeline(id)) else {
            return;
        };
        let bind_group = device.create_bind_group(
            "show depth",
            &cache.get_bind_group_layout(layout),
            &BindGroupEntries::single(&texture),
        );
        let color_attachments = [Some(target.get_color_attachment())];
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("show depth"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub(crate) fn init_depth_display(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(DepthDisplay {
        shader: asset_server.load("embedded://mcrs_minecraft_render/shaders/core/show.wgsl"),
        depth_layout: BindGroupLayoutDescriptor::new(
            "show depth",
            &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, texture_depth_2d()),
        ),
        pyramid_layout: BindGroupLayoutDescriptor::new(
            "show depth pyramid",
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
        depth: None,
        pyramid: None,
    });
}

pub(crate) fn prepare_depth_display(
    mut display: ResMut<DepthDisplay>,
    views: Query<&ExtractedView>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    if display.depth.is_some() {
        return;
    }
    let Some(view) = views.iter().next() else {
        return;
    };
    let queue = |label: &str, layout: &BindGroupLayoutDescriptor, defs: Vec<ShaderDefVal>| {
        let mut descriptor = RenderPipelineDescriptor {
            vertex: fullscreen.to_vertex_state(),
            ..common(
                label.into(),
                vec![layout.clone()],
                &display.shader,
                String::new(),
                "show".into(),
                view,
                None,
            )
        };
        descriptor
            .fragment
            .as_mut()
            .expect("the display has a fragment stage")
            .shader_defs = defs;
        cache.queue_render_pipeline(descriptor)
    };
    let depth = queue("show depth", &display.depth_layout, Vec::new());
    let pyramid = queue(
        "show depth pyramid",
        &display.pyramid_layout,
        vec!["PYRAMID".into()],
    );
    display.depth = Some(depth);
    display.pyramid = Some(pyramid);
}
