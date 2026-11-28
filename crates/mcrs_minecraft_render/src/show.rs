use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewDepthStencilTexture;
use bevy::shader::ShaderDefVal;

use crate::gbuffer::{WORLD_IMAGE_FORMAT, nearest_sampler};
use crate::pass::loaded;
use crate::pipeline::descriptor;
use crate::shared_bindings::show;
use crate::terrain::Terrain;

#[derive(Resource)]
pub struct DepthDisplay {
    layout: BindGroupLayoutDescriptor,
    nearest: Sampler,
    depth: CachedRenderPipelineId,
    pyramid: CachedRenderPipelineId,
}

pub enum DepthSource<'a> {
    Depth(&'a ViewDepthStencilTexture),
    Pyramid(&'a Terrain),
}

impl DepthDisplay {
    pub fn draw(
        &self,
        ctx: &mut RenderContext,
        target: &TextureView,
        source: DepthSource,
        cache: &PipelineCache,
        device: &RenderDevice,
    ) {
        let (pipeline, texture) = match source {
            DepthSource::Pyramid(terrain) => {
                let Some(pyramid) = terrain.depth_pyramid() else {
                    return;
                };
                (self.pyramid, pyramid.clone())
            }
            DepthSource::Depth(depth) => (
                self.depth,
                depth.texture().create_view(&TextureViewDescriptor {
                    label: Some("show depth"),
                    aspect: TextureAspect::DepthOnly,
                    ..default()
                }),
            ),
        };
        let Some(pipeline) = cache.get_render_pipeline(pipeline) else {
            return;
        };
        let bind_group = device.create_bind_group(
            "show depth",
            &cache.get_bind_group_layout(&self.layout),
            &show::entries(show::Entries {
                source: &texture,
                nearest: &self.nearest,
            }),
        );
        let color_attachments = [Some(loaded(target))];
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

pub(crate) fn init_depth_display(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let shader = asset_server.load("embedded://mcrs_minecraft_render/shaders/core/show.wesl");
    let layout = show::layout();
    let queue = |label: &str, defs: Vec<ShaderDefVal>| {
        let mut descriptor = RenderPipelineDescriptor {
            vertex: fullscreen.to_vertex_state(),
            ..descriptor(
                label.into(),
                vec![layout.clone()],
                &shader,
                String::new(),
                "show".into(),
                WORLD_IMAGE_FORMAT,
                None,
            )
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs = defs;
        }
        cache.queue_render_pipeline(descriptor)
    };
    let depth = queue("show depth", Vec::new());
    let pyramid = queue("show depth pyramid", vec!["PYRAMID".into()]);
    commands.insert_resource(DepthDisplay {
        layout,
        nearest: nearest_sampler(&device),
        depth,
        pyramid,
    });
}
