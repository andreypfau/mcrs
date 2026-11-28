use bevy::asset::embedded_asset;
use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::core_pipeline::schedule::Core3d;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::ViewDepthStencilTexture;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;

use crate::frame::write_frame_view;
use crate::frame_view::FrameView;
use crate::gbuffer::{DeferredFrame, WORLD_IMAGE_FORMAT};
use crate::pass::loaded;
use crate::pipeline::descriptor;
use crate::shared_bindings::{world_text_textures, world_text_view};
use crate::shared_types::{TEXT_FRAME_SIZE, TextFrame};
use crate::terrain::Terrain;
use crate::{DEPTH_COMPARE, WorldPass};

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    pub block: [i32; 3],
    pub offset: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
    pub light: [f32; 2],
}

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
        embedded_asset!(app, "shaders/world_text.wesl");
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
                        .after(write_frame_view)
                        .run_if(resource_exists::<TextPass>)
                        .run_if(resource_exists::<WorldText>),
                    prepare_text_bind_groups
                        .in_set(RenderSystems::PrepareBindGroups)
                        .run_if(resource_exists::<TextPass>)
                        .run_if(resource_exists::<WorldText>),
                ),
            )
            .add_systems(Core3d, draw_world_text.in_set(WorldPass::Overlay));
    }
}

#[derive(Resource)]
pub(crate) struct TextPass {
    view_layout: BindGroupLayoutDescriptor,
    texture_layout: BindGroupLayoutDescriptor,
    pipeline: CachedRenderPipelineId,
    buffer: Option<Buffer>,
    capacity: u64,
    revision: Option<u64>,
    count: u32,
    view_buffer: Buffer,
    view: Option<BindGroup>,
    texture: Option<(AssetId<Image>, TextureViewId, BindGroup)>,
}

fn init_text(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let view_layout = world_text_view::layout();
    let texture_layout = world_text_textures::layout();
    let shader = asset_server.load("embedded://mcrs_minecraft_render/shaders/world_text.wesl");
    let pipeline = queue_pipeline(&view_layout, &texture_layout, &shader, &cache);
    commands.insert_resource(TextPass {
        view_layout,
        texture_layout,
        view_buffer: device.create_buffer(&BufferDescriptor {
            label: Some("world text view"),
            size: TEXT_FRAME_SIZE,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        pipeline,
        buffer: None,
        capacity: 0,
        revision: None,
        count: 0,
        view: None,
        texture: None,
    });
}

const VERTEX_BYTES: u64 = size_of::<TextVertex>() as u64;

fn queue_pipeline(
    view_layout: &BindGroupLayoutDescriptor,
    texture_layout: &BindGroupLayoutDescriptor,
    shader: &Handle<Shader>,
    cache: &PipelineCache,
) -> CachedRenderPipelineId {
    let layout = vec![view_layout.clone(), texture_layout.clone()];
    let descriptor = descriptor(
        "world text".into(),
        layout,
        shader,
        "vertex".into(),
        "fragment".into(),
        WORLD_IMAGE_FORMAT,
        None,
    );
    cache.queue_render_pipeline(RenderPipelineDescriptor {
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
            cull_mode: Some(Face::Back),
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
    })
}

fn prepare_text(
    pass: Option<ResMut<TextPass>>,
    text: Option<Res<WorldText>>,
    frame_view: Option<Res<FrameView>>,
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
                _pad_76: 0,
            }),
        );
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "the system writes a cached bind group only after it finds the entry missing or stale, so each write is a change"
    )
)]
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
            &world_text_view::entries(world_text_view::Entries {
                frame: &pass.view_buffer,
            }),
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
        pass.texture = None;
        return;
    };
    let bind_group = device.create_bind_group(
        "world text texture",
        &cache.get_bind_group_layout(&pass.texture_layout),
        &world_text_textures::entries(world_text_textures::Entries {
            font_page: &gpu.texture_view,
            font_sampler: &gpu.sampler,
            lightmap_levels: lightmap,
            lightmap_sampler: &terrain.sprites.lightmap_sampler,
        }),
    );
    pass.texture = Some((texture.id(), lightmap.id(), bind_group));
}

pub(crate) fn draw_world_text(
    view: ViewQuery<&ViewDepthStencilTexture>,
    frame: Option<Res<DeferredFrame>>,
    pass: Res<TextPass>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(frame), Some(id), Some(buffer), Some(view_group), Some((_, _, texture_group))) = (
        frame,
        Some(pass.pipeline),
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
    let depth = view.into_inner();
    let color_attachments = [Some(loaded(&frame.world))];
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
