//! Textured quads in world space, drawn over the lit frame against the opaque world's depth.

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
use bevy::render::view::{ExtractedView, ViewDepthStencilTexture, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use std::num::NonZeroU64;

use crate::frame::write_frame_view;
use crate::frame_view::FrameView;
use crate::terrain::Terrain;
use crate::{DEPTH_COMPARE, WorldPass, pipeline_descriptor};

/// One corner of a quad: the block it belongs to and its place inside that block, a point of the
/// texture, a linear colour, and the block and sky light to look up in the lightmap of the
/// terrain. The block and the offset stay apart so that no absolute coordinate reaches f32.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    pub block: [i32; 3],
    pub offset: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
    /// Block light and sky light on the scale of the lightmap: a level times 16.
    pub light: [f32; 2],
}

/// The triangles to draw, six vertices to a quad, and the texture they sample. The pass keeps
/// what it uploaded while `revision` stays the same. The list is copied into the render world
/// when the resource changes, so a writer touches it only when the vertices change.
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct WorldText {
    pub vertices: Vec<TextVertex>,
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
                    prepare_text
                        .in_set(RenderSystems::Prepare)
                        .after(write_frame_view),
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
    /// The view against the origin of the camera section, written every frame.
    view_buffer: Buffer,
    view: Option<BindGroup>,
    texture: Option<(AssetId<Image>, TextureViewId, BindGroup)>,
}

/// The view of the frame as the terrain has it: nothing here is an absolute world coordinate.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TextFrame {
    clip_from_relative: [f32; 16],
    section: [i32; 3],
    _pad: i32,
}

const VIEW_BYTES: u64 = size_of::<TextFrame>() as u64;

fn init_text(mut commands: Commands, asset_server: Res<AssetServer>, device: Res<RenderDevice>) {
    commands.insert_resource(TextPass {
        view_layout: BindGroupLayoutDescriptor::new(
            "world text view",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX,
                (uniform_buffer_sized(false, NonZeroU64::new(VIEW_BYTES)),),
            ),
        ),
        texture_layout: BindGroupLayoutDescriptor::new(
            "world text texture",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        ),
        view_buffer: device.create_buffer(&BufferDescriptor {
            label: Some("world text view"),
            size: VIEW_BYTES,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
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
    frame_view: Option<Res<FrameView>>,
    views: Query<&ExtractedView, With<Camera3d>>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(mut pass), Some(text)) = (pass, text) else {
        return;
    };
    if let Some(frame_view) = frame_view {
        queue.write_buffer(
            &pass.view_buffer,
            0,
            bytemuck::bytes_of(&TextFrame {
                clip_from_relative: frame_view.clip_from_relative.to_cols_array(),
                section: frame_view.camera_section.to_array(),
                _pad: 0,
            }),
        );
    }
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
                            format: VertexFormat::Sint32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 24,
                            shader_location: 2,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x3,
                            offset: 32,
                            shader_location: 3,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 44,
                            shader_location: 4,
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
    terrain: Option<Res<Terrain>>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let (Some(mut pass), Some(text)) = (pass, text) else {
        return;
    };
    if pass.view.is_none() {
        let bind_group = device.create_bind_group(
            "world text view",
            &cache.get_bind_group_layout(&pass.view_layout),
            &BindGroupEntries::sequential((pass.view_buffer.as_entire_buffer_binding(),)),
        );
        pass.view = Some(bind_group);
    }
    let (Some(texture), Some(terrain)) = (&text.texture, terrain) else {
        return;
    };
    let lightmap = &terrain.sprites.lightmap_view;
    if pass
        .texture
        .as_ref()
        .is_some_and(|(id, view, _)| *id == texture.id() && *view == lightmap.id())
    {
        return;
    }
    let Some(gpu) = images.get(texture.id()) else {
        return;
    };
    let bind_group = device.create_bind_group(
        "world text texture",
        &cache.get_bind_group_layout(&pass.texture_layout),
        &BindGroupEntries::sequential((
            &gpu.texture_view,
            &gpu.sampler,
            lightmap,
            &terrain.sprites.lightmap_sampler,
        )),
    );
    pass.texture = Some((texture.id(), lightmap.id(), bind_group));
}

type TextTarget = (&'static ViewTarget, &'static ViewDepthStencilTexture);

fn draw_world_text(
    view: ViewQuery<TextTarget>,
    pass: Res<TextPass>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(id), Some(buffer), Some(view_group), Some((_, _, texture_group))) = (
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
    let (target, depth) = view.into_inner();
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
    render.set_bind_group(0, view_group, &[]);
    render.set_bind_group(1, texture_group, &[]);
    render.set_vertex_buffer(0, buffer.slice(..));
    render.draw(0..pass.count, 0..1);
}
