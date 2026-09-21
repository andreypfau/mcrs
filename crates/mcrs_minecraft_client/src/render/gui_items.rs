use std::num::NonZeroU64;

use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, texture_2d, texture_2d_array, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{ExtractedView, ViewDepthTexture, ViewTarget};
use bevy::shader::Shader;

use super::terrain::Terrain;
use super::{DEPTH_COMPARE, pipeline_descriptor, uniform_buffer};
use crate::gui::scene::{GLINT_ALPHA, GuiAtlas, GuiBatch};

#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct GuiUniform {
    framebuffer: [f32; 2],
    glint_offset: [f32; 2],
    scale: f32,
    glint_alpha: f32,
    _pad: [f32; 2],
}

#[derive(Resource)]
pub(super) struct GuiPass {
    layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    uniform: Buffer,
    vertices: Option<Buffer>,
    pipelines: Option<(
        TextureFormat,
        CachedRenderPipelineId,
        CachedRenderPipelineId,
    )>,
    textures: Option<GuiTextures>,
    bind_group: Option<([TextureId; 4], BindGroup)>,
}

struct GuiTextures {
    atlas_view: TextureView,
    glint_view: TextureView,
    glint_sampler: Sampler,
}

pub(super) fn init_gui_pass(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    device: Res<RenderDevice>,
) {
    commands.insert_resource(GuiPass {
        layout: BindGroupLayoutDescriptor::new(
            "gui items",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX_FRAGMENT,
                (
                    uniform_buffer_sized(false, NonZeroU64::new(size_of::<GuiUniform>() as u64)),
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    storage_buffer_read_only_sized(false, None),
                    storage_buffer_read_only_sized(false, None),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        ),
        shader: asset_server
            .load("embedded://mcrs_minecraft_client/render/shaders/core/gui_items.wgsl"),
        uniform: uniform_buffer("gui items", size_of::<GuiUniform>() as u64, &device),
        vertices: None,
        pipelines: None,
        textures: None,
        bind_group: None,
    });
}

fn srgb_texture(
    label: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
    device: &RenderDevice,
    queue: &RenderQueue,
) -> TextureView {
    let texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        pixels,
    );
    texture.create_view(&TextureViewDescriptor::default())
}

pub(super) fn prepare_gui_pass(
    pass: Option<ResMut<GuiPass>>,
    atlas: Option<Res<GuiAtlas>>,
    terrain: Option<Res<Terrain>>,
    views: Query<&ExtractedView>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipeline_cache: Res<PipelineCache>,
) {
    let (Some(mut pass), Some(terrain)) = (pass, terrain) else {
        return;
    };
    let Some(view) = views.iter().next() else {
        return;
    };
    let pass = &mut *pass;
    if pass.textures.is_none() {
        let Some(atlas) = atlas.as_deref() else {
            return;
        };
        let atlas = &*atlas.0;
        pass.textures = Some(GuiTextures {
            atlas_view: srgb_texture(
                "gui atlas",
                atlas.width,
                atlas.height,
                &atlas.pixels,
                &device,
                &queue,
            ),
            glint_view: srgb_texture(
                "enchanted glint",
                atlas.glint.0,
                atlas.glint.1,
                &atlas.glint.2,
                &device,
                &queue,
            ),
            glint_sampler: device.create_sampler(&SamplerDescriptor {
                label: Some("enchanted glint"),
                address_mode_u: AddressMode::Repeat,
                address_mode_v: AddressMode::Repeat,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                ..default()
            }),
        });
    }
    if pass
        .pipelines
        .as_ref()
        .is_none_or(|(format, ..)| *format != view.target_format)
    {
        let queue_pipeline = |item: bool| {
            let mut descriptor = pipeline_descriptor(
                if item { "gui items" } else { "gui flat" }.into(),
                vec![pass.layout.clone()],
                &pass.shader,
                "vs_gui".into(),
                "fs_gui".into(),
                view,
                Some(BlendState::ALPHA_BLENDING),
            );
            descriptor.vertex.buffers = vec![VertexBufferLayout::from_vertex_formats(
                VertexStepMode::Vertex,
                [
                    VertexFormat::Float32x3,
                    VertexFormat::Float32x2,
                    VertexFormat::Unorm8x4,
                    VertexFormat::Uint32,
                ],
            )];
            descriptor.primitive = PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: item.then_some(Face::Back),
                ..default()
            };
            descriptor.depth_stencil = Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(item),
                depth_compare: Some(if item {
                    DEPTH_COMPARE
                } else {
                    CompareFunction::Always
                }),
                stencil: default(),
                bias: default(),
            });
            pipeline_cache.queue_render_pipeline(descriptor)
        };
        pass.pipelines = Some((
            view.target_format,
            queue_pipeline(true),
            queue_pipeline(false),
        ));
    }
    let sprites = &terrain.sprites;
    let atlas_ids = std::array::from_fn(|i| sprites.atlases[i].texture.id());
    if pass
        .bind_group
        .as_ref()
        .is_none_or(|(ids, _)| *ids != atlas_ids)
    {
        let textures = pass.textures.as_ref().expect("built above");
        let bind_group = device.create_bind_group(
            "gui items",
            &pipeline_cache.get_bind_group_layout(&pass.layout),
            &BindGroupEntries::sequential((
                pass.uniform.as_entire_buffer_binding(),
                &sprites.atlases[0].view,
                &sprites.atlases[1].view,
                &sprites.atlases[2].view,
                &sprites.atlases[3].view,
                &sprites.atlas_sampler,
                sprites.frames.as_entire_buffer_binding(),
                sprites.table.as_entire_buffer_binding(),
                &textures.atlas_view,
                &textures.glint_view,
                &textures.glint_sampler,
            )),
        );
        pass.bind_group = Some((atlas_ids, bind_group));
    }
}

pub(super) fn write_gui_buffers(
    pass: Option<ResMut<GuiPass>>,
    batch: Option<Res<GuiBatch>>,
    views: Query<&ExtractedView>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(mut pass), Some(batch)) = (pass, batch) else {
        return;
    };
    let Some(view) = views.iter().next() else {
        return;
    };
    let pass = &mut *pass;
    let viewport = view.viewport.zw().as_vec2();
    queue.write_buffer(
        &pass.uniform,
        0,
        bytemuck::bytes_of(&GuiUniform {
            framebuffer: viewport.to_array(),
            glint_offset: batch.glint_offset,
            scale: batch.scale as f32,
            glint_alpha: GLINT_ALPHA,
            _pad: [0.0; 2],
        }),
    );
    if batch.vertices.is_empty() {
        return;
    }
    let bytes: &[u8] = bytemuck::cast_slice(&batch.vertices);
    if pass
        .vertices
        .as_ref()
        .is_none_or(|buffer| buffer.size() < bytes.len() as u64)
    {
        pass.vertices = Some(device.create_buffer(&BufferDescriptor {
            label: Some("gui vertices"),
            size: (bytes.len() as u64).next_power_of_two(),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    queue.write_buffer(pass.vertices.as_ref().expect("sized above"), 0, bytes);
}

/// The world's depth is cleared first: the items' private bands must not be tested
/// against terrain, and vanilla likewise draws each GUI item into a fresh atlas slot.
/// Every item element then gets its own slice of the depth range, so its own geometry is
/// depth-tested against itself and later elements land over earlier ones by draw order.
pub(super) fn draw_gui(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &ExtractedView)>,
    gui: Option<Res<GuiPass>>,
    batch: Option<Res<GuiBatch>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, depth, extracted) = view.into_inner();
    let color_attachments = [Some(RenderPassColorAttachment {
        view: target.main_texture_view(),
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Load,
            store: StoreOp::Store,
        },
    })];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("gui"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: depth.view(),
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
    let (Some(gui), Some(batch)) = (gui.as_deref(), batch.as_deref()) else {
        return;
    };
    if batch.draws.is_empty() {
        return;
    }
    let (Some((_, item_id, flat_id)), Some((_, bind_group)), Some(vertices)) = (
        gui.pipelines,
        gui.bind_group.as_ref(),
        gui.vertices.as_ref(),
    ) else {
        return;
    };
    let (Some(item), Some(flat)) = (
        pipeline_cache.get_render_pipeline(item_id),
        pipeline_cache.get_render_pipeline(flat_id),
    ) else {
        return;
    };
    let size = extracted.viewport.zw().as_vec2();
    let items = batch.draws.iter().filter(|draw| draw.item).count().max(1) as f32;
    pass.set_bind_group(0, bind_group, &[]);
    pass.set_vertex_buffer(0, vertices.slice(..));
    let mut band = 0;
    for draw in &batch.draws {
        if draw.item {
            pass.set_render_pipeline(item);
            pass.set_viewport(
                0.0,
                0.0,
                size.x,
                size.y,
                band as f32 / items,
                (band + 1) as f32 / items,
            );
            band += 1;
        } else {
            pass.set_render_pipeline(flat);
            pass.set_viewport(0.0, 0.0, size.x, size.y, 0.0, 1.0);
        }
        pass.draw(draw.range.clone(), 0..1);
    }
}
