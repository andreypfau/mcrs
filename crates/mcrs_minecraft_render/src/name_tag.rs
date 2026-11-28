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
use crate::world_text::draw_world_text;
use crate::{DEPTH_COMPARE, WorldPass};

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NameTagVertex {
    pub block: [i32; 3],
    pub offset: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub light: [f32; 2],
}

pub const SOLID_UV: [f32; 2] = [-1.0, -1.0];

#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct NameTags {
    pub vertices: Vec<NameTagVertex>,
    pub through: u32,
    pub texture: Option<Handle<Image>>,
    pub revision: u64,
}

pub struct NameTagRenderPlugin;

impl Plugin for NameTagRenderPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/name_tag.wesl");
        app.init_resource::<NameTags>()
            .add_plugins(ExtractResourcePlugin::<NameTags>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_pass)
            .add_systems(
                Render,
                (
                    prepare
                        .in_set(RenderSystems::Prepare)
                        .after(write_frame_view)
                        .run_if(resource_exists::<NameTagPass>)
                        .run_if(resource_exists::<NameTags>),
                    prepare_bind_groups
                        .in_set(RenderSystems::PrepareBindGroups)
                        .run_if(resource_exists::<NameTagPass>)
                        .run_if(resource_exists::<NameTags>),
                ),
            )
            .add_systems(
                Core3d,
                draw_name_tags
                    .in_set(WorldPass::Overlay)
                    .after(draw_world_text),
            );
    }
}

#[derive(Resource)]
struct NameTagPass {
    view_layout: BindGroupLayoutDescriptor,
    texture_layout: BindGroupLayoutDescriptor,
    pipelines: [CachedRenderPipelineId; 2],
    buffer: Option<Buffer>,
    capacity: u64,
    revision: Option<u64>,
    count: u32,
    through: u32,
    view_buffer: Buffer,
    view: Option<BindGroup>,
    texture: Option<(AssetId<Image>, TextureViewId, BindGroup)>,
}

fn init_pass(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let view_layout = world_text_view::layout();
    let texture_layout = world_text_textures::layout();
    let shader = asset_server.load("embedded://mcrs_minecraft_render/shaders/name_tag.wesl");
    let pipelines = [
        queue_pipeline(
            &view_layout,
            &texture_layout,
            &shader,
            &cache,
            "name tag through",
            CompareFunction::Always,
        ),
        queue_pipeline(
            &view_layout,
            &texture_layout,
            &shader,
            &cache,
            "name tag",
            DEPTH_COMPARE,
        ),
    ];
    commands.insert_resource(NameTagPass {
        view_layout,
        texture_layout,
        view_buffer: device.create_buffer(&BufferDescriptor {
            label: Some("name tag view"),
            size: TEXT_FRAME_SIZE,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        pipelines,
        buffer: None,
        capacity: 0,
        revision: None,
        count: 0,
        through: 0,
        view: None,
        texture: None,
    });
}

const VERTEX_BYTES: u64 = size_of::<NameTagVertex>() as u64;

fn attribute(format: VertexFormat, offset: u64, shader_location: u32) -> VertexAttribute {
    VertexAttribute {
        format,
        offset,
        shader_location,
    }
}

fn queue_pipeline(
    view_layout: &BindGroupLayoutDescriptor,
    texture_layout: &BindGroupLayoutDescriptor,
    shader: &Handle<Shader>,
    cache: &PipelineCache,
    label: &str,
    depth_compare: CompareFunction,
) -> CachedRenderPipelineId {
    let layout = vec![view_layout.clone(), texture_layout.clone()];
    let descriptor = descriptor(
        label.into(),
        layout,
        shader,
        "vertex".into(),
        "fragment".into(),
        WORLD_IMAGE_FORMAT,
        Some(BlendState::ALPHA_BLENDING),
    );
    cache.queue_render_pipeline(RenderPipelineDescriptor {
        vertex: VertexState {
            buffers: vec![VertexBufferLayout {
                array_stride: VERTEX_BYTES,
                step_mode: VertexStepMode::Vertex,
                attributes: vec![
                    attribute(VertexFormat::Sint32x3, 0, 0),
                    attribute(VertexFormat::Float32x3, 12, 1),
                    attribute(VertexFormat::Float32x2, 24, 2),
                    attribute(VertexFormat::Float32x4, 32, 3),
                    attribute(VertexFormat::Float32x2, 48, 4),
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
            depth_write_enabled: Some(false),
            depth_compare: Some(depth_compare),
            stencil: default(),
            bias: default(),
        }),
        ..descriptor
    })
}

fn prepare(
    pass: Option<ResMut<NameTagPass>>,
    tags: Option<Res<NameTags>>,
    frame_view: Option<Res<FrameView>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(mut pass), Some(tags)) = (pass, tags) else {
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
    if pass.revision == Some(tags.revision) {
        return;
    }
    pass.revision = Some(tags.revision);
    pass.count = tags.vertices.len() as u32;
    pass.through = tags.through.min(pass.count);
    let bytes: &[u8] = bytemuck::cast_slice(tags.vertices.as_slice());
    if bytes.is_empty() {
        return;
    }
    if pass.buffer.is_none() || pass.capacity < bytes.len() as u64 {
        let capacity = (bytes.len() as u64 * 2).next_power_of_two();
        pass.buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some("name tag vertices"),
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
fn prepare_bind_groups(
    pass: Option<ResMut<NameTagPass>>,
    tags: Option<Res<NameTags>>,
    images: Res<RenderAssets<GpuImage>>,
    terrain: Option<Res<Terrain>>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let (Some(mut pass), Some(tags)) = (pass, tags) else {
        return;
    };
    if pass.view.is_none() {
        let bind_group = device.create_bind_group(
            "name tag view",
            &cache.get_bind_group_layout(&pass.view_layout),
            &world_text_view::entries(world_text_view::Entries {
                frame: &pass.view_buffer,
            }),
        );
        pass.view = Some(bind_group);
    }
    let (Some(texture), Some(terrain)) = (&tags.texture, terrain) else {
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
        "name tag texture",
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

fn draw_name_tags(
    view: ViewQuery<&ViewDepthStencilTexture>,
    frame: Option<Res<DeferredFrame>>,
    pass: Res<NameTagPass>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(frame), Some(buffer), Some(view_group), Some((_, _, texture_group))) = (
        frame,
        pass.buffer.as_ref(),
        pass.view.as_ref(),
        pass.texture.as_ref(),
    ) else {
        return;
    };
    let (Some(through), Some(world)) = (
        cache.get_render_pipeline(pass.pipelines[0]),
        cache.get_render_pipeline(pass.pipelines[1]),
    ) else {
        return;
    };
    if pass.count == 0 {
        return;
    }
    let depth = view.into_inner();
    let color_attachments = [Some(loaded(&frame.world))];
    let mut render = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("name tags"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    render.set_bind_group(0, view_group, &[]);
    render.set_bind_group(1, texture_group, &[]);
    render.set_vertex_buffer(0, buffer.slice(..));
    render.set_render_pipeline(through);
    render.draw(0..pass.through, 0..1);
    render.set_render_pipeline(world);
    render.draw(pass.through..pass.count, 0..1);
}
