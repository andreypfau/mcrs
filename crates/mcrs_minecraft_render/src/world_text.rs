//! Textured quads in world space, drawn over the lit frame with the depth of the opaque world.
//! A text on a sign is one run of glyph quads; the pass keeps no state beyond the vertex list
//! the main world hands it, and uploads the list again only when its revision changes.

use bevy::asset::embedded_asset;
use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::core_pipeline::schedule::Core3d;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use std::num::NonZeroU64;
use std::sync::Arc;

use crate::{DEPTH_COMPARE, WorldPass, pipeline_descriptor};

/// One corner of a quad: a world position, a point of the texture, and a linear colour.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// The triangles to draw, six vertices to a quad, and the texture they sample. The pass keeps
/// what it uploaded while `revision` stays the same.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct WorldText {
    pub vertices: Arc<Vec<TextVertex>>,
    pub texture: Option<Handle<Image>>,
    pub revision: u64,
}

pub struct WorldTextPlugin;

impl Plugin for WorldTextPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/world_text.wgsl");
        app.init_resource::<WorldText>()
            .add_plugins(ExtractResourcePlugin::<WorldText>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_text)
            .add_systems(
                Render,
                (
                    prepare_text.in_set(RenderSystems::Prepare),
                    prepare_text_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(Core3d, draw_world_text.in_set(WorldPass::Overlay));
    }
}

#[derive(Resource)]
struct TextPass {
    view_layout: BindGroupLayoutDescriptor,
    texture_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    pipeline: Option<CachedRenderPipelineId>,
    buffer: Option<Buffer>,
    capacity: u64,
    revision: Option<u64>,
    count: u32,
    view: Option<(BufferId, BindGroup)>,
    texture: Option<(AssetId<Image>, BindGroup)>,
}

fn init_text(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(TextPass {
        view_layout: BindGroupLayoutDescriptor::new(
            "world text view",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX,
                (uniform_buffer_sized(true, Some(ViewUniform::min_size())),),
            ),
        ),
        texture_layout: BindGroupLayoutDescriptor::new(
            "world text texture",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        ),
        shader: asset_server.load("embedded://mcrs_minecraft_render/shaders/world_text.wgsl"),
        pipeline: None,
        buffer: None,
        capacity: 0,
        revision: None,
        count: 0,
        view: None,
        texture: None,
    });
}

const VERTEX_BYTES: u64 = size_of::<TextVertex>() as u64;

fn prepare_text(
    pass: Option<ResMut<TextPass>>,
    text: Option<Res<WorldText>>,
    views: Query<&ExtractedView, With<Camera3d>>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(mut pass), Some(text)) = (pass, text) else {
        return;
    };
    if pass.pipeline.is_none() {
        let Some(view) = views.iter().next() else {
            return;
        };
        let layout = vec![pass.view_layout.clone(), pass.texture_layout.clone()];
        let descriptor = pipeline_descriptor(
            "world text".into(),
            layout,
            &pass.shader,
            "vertex".into(),
            "fragment".into(),
            view,
            None,
        );
        pass.pipeline = Some(cache.queue_render_pipeline(RenderPipelineDescriptor {
            vertex: VertexState {
                buffers: vec![VertexBufferLayout {
                    array_stride: VERTEX_BYTES,
                    step_mode: VertexStepMode::Vertex,
                    attributes: vec![
                        VertexAttribute {
                            format: VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 12,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 20,
                            shader_location: 2,
                        },
                    ],
                }],
                ..descriptor.vertex
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: default(),
                bias: default(),
            }),
            ..descriptor
        }));
    }
    if pass.revision == Some(text.revision) {
        return;
    }
    pass.revision = Some(text.revision);
    pass.count = text.vertices.len() as u32;
    let bytes: &[u8] = bytemuck::cast_slice(text.vertices.as_slice());
    if bytes.is_empty() {
        return;
    }
    if pass.buffer.is_none() || pass.capacity < bytes.len() as u64 {
        // Room to grow, so a sign appearing does not reallocate every time.
        let capacity = (bytes.len() as u64 * 2).next_power_of_two();
        pass.buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some("world text vertices"),
            size: capacity,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        pass.capacity = capacity;
    }
    if let Some(buffer) = &pass.buffer {
        queue.write_buffer(buffer, 0, bytes);
    }
}

fn prepare_text_bind_groups(
    pass: Option<ResMut<TextPass>>,
    text: Option<Res<WorldText>>,
    images: Res<RenderAssets<GpuImage>>,
    view_uniforms: Res<ViewUniforms>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let (Some(mut pass), Some(text)) = (pass, text) else {
        return;
    };
    if let Some(view_buffer) = view_uniforms.uniforms.buffer()
        && pass
            .view
            .as_ref()
            .is_none_or(|(id, _)| *id != view_buffer.id())
    {
        let bind_group = device.create_bind_group(
            "world text view",
            &cache.get_bind_group_layout(&pass.view_layout),
            &BindGroupEntries::sequential((BufferBinding {
                buffer: view_buffer,
                offset: 0,
                size: NonZeroU64::new(ViewUniform::min_size().get()),
            },)),
        );
        pass.view = Some((view_buffer.id(), bind_group));
    }
    let Some(texture) = &text.texture else {
        return;
    };
    if pass
        .texture
        .as_ref()
        .is_some_and(|(id, _)| *id == texture.id())
    {
        return;
    }
    let Some(gpu) = images.get(texture.id()) else {
        return;
    };
    let bind_group = device.create_bind_group(
        "world text texture",
        &cache.get_bind_group_layout(&pass.texture_layout),
        &BindGroupEntries::sequential((&gpu.texture_view, &gpu.sampler)),
    );
    pass.texture = Some((texture.id(), bind_group));
}

type TextView = (
    &'static ViewTarget,
    &'static ViewDepthTexture,
    &'static ViewUniformOffset,
);

fn draw_world_text(
    view: ViewQuery<TextView>,
    pass: Res<TextPass>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(id), Some(buffer), Some((_, view_group)), Some((_, texture_group))) = (
        pass.pipeline,
        pass.buffer.as_ref(),
        pass.view.as_ref(),
        pass.texture.as_ref(),
    ) else {
        return;
    };
    let Some(pipeline) = cache.get_render_pipeline(id) else {
        return;
    };
    if pass.count == 0 {
        return;
    }
    let (target, depth, offset) = view.into_inner();
    let color_attachments = [Some(target.get_color_attachment())];
    let mut render = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("world text"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    render.set_render_pipeline(pipeline);
    render.set_bind_group(0, view_group, &[offset.offset]);
    render.set_bind_group(1, texture_group, &[]);
    render.set_vertex_buffer(0, buffer.slice(..));
    render.draw(0..pass.count, 0..1);
}
