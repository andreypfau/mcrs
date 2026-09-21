use std::num::NonZeroU64;

use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::ecs::system::SystemParam;
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::render_phase::TrackedRenderPass;
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
use crate::item_model::resolve::GuiVertex;

#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct GuiUniform {
    framebuffer: [f32; 2],
    scale: f32,
    animated_from: u32,
    glint_offset: [f32; 2],
    glint_alpha: f32,
    _pad: f32,
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
    draws: usize,
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
        draws: 0,
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
            descriptor.vertex.buffers = vec![VertexBufferLayout {
                array_stride: size_of::<GuiVertex>() as u64,
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
                        format: VertexFormat::Unorm8x4,
                        offset: 20,
                        shader_location: 2,
                    },
                    VertexAttribute {
                        format: VertexFormat::Uint32,
                        offset: 24,
                        shader_location: 3,
                    },
                ],
            }];
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
    terrain: Option<Res<Terrain>>,
    views: Query<&ExtractedView>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let (Some(mut pass), Some(batch), Some(terrain)) = (pass, batch, terrain) else {
        return;
    };
    let Some(view) = views.iter().next() else {
        return;
    };
    let pass = &mut *pass;
    pass.draws = 0;
    let sprites = &terrain.sprites;
    let viewport = view.viewport.zw().as_vec2();
    queue.write_buffer(
        &pass.uniform,
        0,
        bytemuck::bytes_of(&GuiUniform {
            framebuffer: viewport.to_array(),
            scale: batch.scale as f32,
            animated_from: sprites.animated_from,
            glint_offset: batch.glint_offset,
            glint_alpha: GLINT_ALPHA,
            _pad: 0.0,
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
    pass.draws = batch.draws.len();
}

#[derive(SystemParam)]
pub(super) struct GuiDraws<'w> {
    pass: Option<Res<'w, GuiPass>>,
    batch: Option<Res<'w, GuiBatch>>,
    pipeline_cache: Res<'w, PipelineCache>,
}

impl GuiDraws<'_> {
    /// Every item element gets its own slice of the depth range, so its own geometry is
    /// depth-tested against itself and later elements land over earlier ones by draw order.
    pub fn draw<'pass>(&'pass self, pass: &mut TrackedRenderPass<'pass>, size: Vec2) {
        let (Some(gui), Some(batch)) = (self.pass.as_deref(), self.batch.as_deref()) else {
            return;
        };
        if gui.draws == 0 {
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
            self.pipeline_cache.get_render_pipeline(item_id),
            self.pipeline_cache.get_render_pipeline(flat_id),
        ) else {
            return;
        };
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
}

/// The world's depth is cleared first: the items' private bands must not be tested
/// against terrain, and vanilla likewise draws each GUI item into a fresh atlas slot.
pub(super) fn draw_gui(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &ExtractedView)>,
    draws: GuiDraws,
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
    draws.draw(&mut pass, extracted.viewport.zw().as_vec2());
}
