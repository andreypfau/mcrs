use std::num::NonZeroU64;
use std::ops::Range;

use bevy::asset::AssetEventSystems;
use bevy::camera::NormalizedRenderTarget;
use bevy::core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, main_transparent_pass_3d};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_2d_array, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, ViewDepthStencilTexture, ViewTarget};
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use mcrs_minecraft_entity_render::{FigureDraw, FigureTargets, FigureViewSystems};
use mcrs_minecraft_render::{
    DEPTH_COMPARE, RenderPath, Terrain, Words, WorldPass, pipeline_descriptor, uniform_buffer,
};

use crate::font_unihex::CELL;
use crate::glyph_cache::{self, GlyphCache};
use crate::glyph_uploads::{GlyphCell, GlyphUploads, glyphs_pending, queue_glyphs};
use crate::gui_batch::{GuiBatch, GuiDraw, GuiDrawKind};
use crate::gui_batch_extract::{extract_gui_batch, gui_batch_extract_due};
use crate::gui_glint::{GLINT_ALPHA, GuiGlint};
use crate::gui_images::GuiImages;
use crate::gui_phase::GuiPhase;

pub struct GuiItemsPlugin;

impl Plugin for GuiItemsPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "gui_items.wesl");
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        app.init_resource::<GlyphUploads>().add_systems(
            PostUpdate,
            queue_glyphs
                .after(GuiPhase::Batch)
                .after(AssetEventSystems)
                .run_if(
                    resource_exists_and_changed::<GlyphCache>
                        .or_else(on_message::<AssetEvent<Image>>)
                        .or_else(resource_exists_and_changed::<GuiImages>),
                ),
        );
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<GuiBatch>()
            .init_resource::<GlyphUploads>()
            .add_systems(
                ExtractSchedule,
                extract_gui_batch.run_if(gui_batch_extract_due),
            )
            .add_systems(RenderStartup, init_gui_pass)
            .add_systems(
                Render,
                (
                    prepare_gui_pass.run_if(gui_pass_stale),
                    upload_glyphs.run_if(glyphs_pending),
                    write_gui_buffers.run_if(resource_changed::<GuiBatch>),
                    write_gui_uniform.run_if(
                        resource_changed::<GuiBatch>
                            .or_else(resource_exists_and_changed::<GuiGlint>)
                            .or_else(gui_viewport_changed),
                    ),
                )
                    .chain()
                    .in_set(RenderSystems::Prepare),
            )
            .add_systems(
                Core3d,
                draw_gui
                    .after(WorldPass::Forward)
                    .after(mcrs_minecraft_render::PanoramaPass)
                    .after(FigureViewSystems)
                    .before(main_transparent_pass_3d)
                    .in_set(Core3dSystems::MainPass),
            );
    }
}

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
pub struct GuiPass {
    layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    uniform: Buffer,
    vertices: Option<Buffer>,
    pipelines: Option<GuiPipelines>,
    key: Option<GuiPassKey>,
    viewport: Option<UVec2>,
    bind_group: Option<BindGroup>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct GuiPassKey {
    format: TextureFormat,
    terrain: [TextureId; 4],
    atlas: TextureViewId,
    glint: TextureViewId,
}

impl GuiPassKey {
    fn of(view: &ExtractedView, terrain: &Terrain, atlas: &GpuImage, glint: &GpuImage) -> Self {
        Self {
            format: view.target_format,
            terrain: terrain
                .sprites
                .atlases
                .each_ref()
                .map(|slot| slot.texture.id()),
            atlas: atlas.texture_view.id(),
            glint: glint.texture_view.id(),
        }
    }
}

struct GuiPipelines {
    format: TextureFormat,
    flat: CachedRenderPipelineId,
    item: CachedRenderPipelineId,
    invert_red_green: CachedRenderPipelineId,
    full_blue: CachedRenderPipelineId,
}

const INVERT: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::OneMinusDst,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent::REPLACE,
};

pub(super) fn init_gui_pass(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    device: Res<RenderDevice>,
    path: Res<RenderPath>,
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
                    Words::layout(*path),
                    Words::layout(*path),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        ),
        shader: asset_server.load("embedded://mcrs_minecraft_client_gui/gui_items.wesl"),
        uniform: uniform_buffer("gui items", size_of::<GuiUniform>() as u64, &device),
        vertices: None,
        pipelines: None,
        key: None,
        viewport: None,
        bind_group: None,
    });
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn gui_pass_stale(
    pass: Option<Res<GuiPass>>,
    terrain: Option<Res<Terrain>>,
    images: Option<Res<GuiImages>>,
    gpu: Res<RenderAssets<GpuImage>>,
    views: Query<(&ExtractedView, &ExtractedCamera)>,
) -> bool {
    let (Some(pass), Some(terrain), Some(images)) = (pass, terrain, images) else {
        return false;
    };
    let (Some(view), Some(atlas), Some(glint)) = (
        window_view(&views),
        gpu.get(images.atlas),
        gpu.get(images.glint),
    ) else {
        return false;
    };
    pass.key != Some(GuiPassKey::of(view, &terrain, atlas, glint))
}

#[expect(
    clippy::too_many_lines,
    reason = "a render prepare step creates each target in dependency order"
)]
pub(super) fn prepare_gui_pass(
    mut commands: Commands,
    pass: Option<ResMut<GuiPass>>,
    images: Option<Res<GuiImages>>,
    gpu: Res<RenderAssets<GpuImage>>,
    terrain: Option<Res<Terrain>>,
    views: Query<(&ExtractedView, &ExtractedCamera)>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
) {
    let (Some(mut pass), Some(terrain), Some(images)) = (pass, terrain, images) else {
        return;
    };
    let (Some(view), Some(atlas), Some(glint)) = (
        window_view(&views),
        gpu.get(images.atlas),
        gpu.get(images.glint),
    ) else {
        return;
    };
    let pass = &mut *pass;
    if pass
        .pipelines
        .as_ref()
        .is_none_or(|pipelines| pipelines.format != view.target_format)
    {
        let queue_pipeline = |kind: GuiDrawKind| {
            let item = kind == GuiDrawKind::Item;
            let (label, blend, write_mask) = match kind {
                GuiDrawKind::Flat | GuiDrawKind::Figure { .. } => (
                    "gui flat",
                    Some(BlendState::ALPHA_BLENDING),
                    ColorWrites::ALL,
                ),
                GuiDrawKind::Item => (
                    "gui items",
                    Some(BlendState::ALPHA_BLENDING),
                    ColorWrites::ALL,
                ),
                GuiDrawKind::InvertRedGreen => (
                    "gui invert",
                    Some(INVERT),
                    ColorWrites::RED | ColorWrites::GREEN,
                ),
                GuiDrawKind::FullBlue => ("gui blue", None, ColorWrites::BLUE),
            };
            let mut descriptor = pipeline_descriptor(
                label.into(),
                vec![pass.layout.clone()],
                &pass.shader,
                "vs_gui".into(),
                "fs_gui".into(),
                view,
                blend,
            );
            descriptor.vertex.shader_defs = terrain.path().shader_defs();
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs = terrain.path().shader_defs();
                for target in fragment.targets.iter_mut().flatten() {
                    target.write_mask = write_mask;
                }
            }
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
        commands.insert_resource(FigureTargets {
            color: view.target_format,
            depth: CORE_3D_DEPTH_FORMAT,
            depth_compare: DEPTH_COMPARE,
            sample_count: 1,
        });
        pass.pipelines = Some(GuiPipelines {
            format: view.target_format,
            flat: queue_pipeline(GuiDrawKind::Flat),
            item: queue_pipeline(GuiDrawKind::Item),
            invert_red_green: queue_pipeline(GuiDrawKind::InvertRedGreen),
            full_blue: queue_pipeline(GuiDrawKind::FullBlue),
        });
    }
    let sprites = &terrain.sprites;
    pass.bind_group = Some(device.create_bind_group(
        "gui items",
        &pipeline_cache.get_bind_group_layout(&pass.layout),
        &BindGroupEntries::sequential((
            pass.uniform.as_entire_buffer_binding(),
            &sprites.atlases[0].view,
            &sprites.atlases[1].view,
            &sprites.atlases[2].view,
            &sprites.atlases[3].view,
            &sprites.atlas_sampler,
            sprites.frames.binding(),
            sprites.table.binding(),
            &atlas.texture_view,
            &glint.texture_view,
            &glint.sampler,
        )),
    ));
    pass.key = Some(GuiPassKey::of(view, &terrain, atlas, glint));
}

pub(super) fn upload_glyphs(
    images: Option<Res<GuiImages>>,
    gpu: Res<RenderAssets<GpuImage>>,
    mut uploads: ResMut<GlyphUploads>,
    queue: Res<RenderQueue>,
) {
    let Some(images) = images else {
        return;
    };
    let Some(atlas) = gpu.get(images.atlas) else {
        return;
    };
    for GlyphCell { cell, pixels } in uploads.cells.drain(..) {
        let (x, y) = glyph_cache::cell_origin(cell);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: images.glyph_origin.x + x,
                    y: images.glyph_origin.y + y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(CELL as u32 * 4),
                rows_per_image: Some(CELL as u32),
            },
            Extent3d {
                width: CELL as u32,
                height: CELL as u32,
                depth_or_array_layers: 1,
            },
        );
    }
}

fn on_window(camera: &ExtractedCamera) -> bool {
    matches!(camera.target, Some(NormalizedRenderTarget::Window(_)))
}

fn window_view<'a>(
    views: &'a Query<(&ExtractedView, &ExtractedCamera)>,
) -> Option<&'a ExtractedView> {
    views
        .iter()
        .find_map(|(view, camera)| on_window(camera).then_some(view))
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn gui_viewport_changed(
    pass: Option<Res<GuiPass>>,
    views: Query<(&ExtractedView, &ExtractedCamera)>,
) -> bool {
    let (Some(pass), Some(view)) = (pass, window_view(&views)) else {
        return false;
    };
    pass.viewport != Some(view.viewport.zw())
}

pub(super) fn write_gui_uniform(
    pass: Option<ResMut<GuiPass>>,
    batch: Res<GuiBatch>,
    glint: Option<Res<GuiGlint>>,
    views: Query<(&ExtractedView, &ExtractedCamera)>,
    queue: Res<RenderQueue>,
) {
    let Some(mut pass) = pass else {
        return;
    };
    let Some(view) = window_view(&views) else {
        return;
    };
    let size = view.viewport.zw();
    if pass.viewport != Some(size) {
        pass.viewport = Some(size);
    }
    let viewport = size.as_vec2();
    queue.write_buffer(
        &pass.uniform,
        0,
        bytemuck::bytes_of(&GuiUniform {
            framebuffer: viewport.to_array(),
            glint_offset: glint.map_or([0.0; 2], |glint| glint.offset),
            scale: batch.scale as f32,
            glint_alpha: GLINT_ALPHA,
            _pad: [0.0; 2],
        }),
    );
}

pub(super) fn write_gui_buffers(
    pass: Option<ResMut<GuiPass>>,
    batch: Res<GuiBatch>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let Some(mut pass) = pass else {
        return;
    };
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
    if let Some(vertices) = pass.vertices.as_ref() {
        queue.write_buffer(vertices, 0, bytes);
    }
}

pub fn draw_gui(
    view: ViewQuery<(
        &ViewTarget,
        &ViewDepthStencilTexture,
        &ExtractedView,
        &ExtractedCamera,
    )>,
    gui: Option<Res<GuiPass>>,
    batch: Option<Res<GuiBatch>>,
    figure: Option<FigureDraw>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, depth, extracted, camera) = view.into_inner();
    if !on_window(camera) {
        return;
    }
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
            view: depth.attachment.depth_stencil_views().attachment_view(),
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
    let (Some(ids), Some(bind_group)) = (gui.pipelines.as_ref(), gui.bind_group.as_ref()) else {
        return;
    };
    let vertices = gui.vertices.as_ref();
    let (Some(flat), Some(item)) = (
        pipeline_cache.get_render_pipeline(ids.flat),
        pipeline_cache.get_render_pipeline(ids.item),
    ) else {
        return;
    };
    let invert_red_green = pipeline_cache.get_render_pipeline(ids.invert_red_green);
    let full_blue = pipeline_cache.get_render_pipeline(ids.full_blue);
    let size = extracted.viewport.zw();
    let banded =
        |draw: &&GuiDraw| matches!(draw.kind, GuiDrawKind::Item | GuiDrawKind::Figure { .. });
    let bands = batch.draws.iter().filter(banded).count().max(1) as f32;
    pass.set_bind_group(0, bind_group, &[]);
    if let Some(vertices) = vertices {
        pass.set_vertex_buffer(0, vertices.slice(..));
    }
    let mut band = 0;
    let band_range = |band: u32| band as f32 / bands..(band + 1) as f32 / bands;
    for draw in &batch.draws {
        let pipeline = match draw.kind {
            GuiDrawKind::Flat => Some(flat),
            GuiDrawKind::Item => Some(item),
            GuiDrawKind::InvertRedGreen => invert_red_green,
            GuiDrawKind::FullBlue => full_blue,
            GuiDrawKind::Figure { rect } => {
                let depth = band_range(band);
                band += 1;
                if let Some(figure) = figure.as_ref() {
                    let scissor = IRect::from_corners(
                        rect.min * batch.scale as i32,
                        rect.max * batch.scale as i32,
                    );
                    draw_figure(&mut pass, figure, scissor, size, depth);
                    pass.set_bind_group(0, bind_group, &[]);
                    if let Some(vertices) = vertices {
                        pass.set_vertex_buffer(0, vertices.slice(..));
                    }
                }
                continue;
            }
        };
        let (Some(pipeline), Some(_)) = (pipeline, vertices) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        if draw.kind == GuiDrawKind::Item {
            let depth = band_range(band);
            pass.set_viewport(
                0.0,
                0.0,
                size.x as f32,
                size.y as f32,
                depth.start,
                depth.end,
            );
            band += 1;
        } else {
            pass.set_viewport(0.0, 0.0, size.x as f32, size.y as f32, 0.0, 1.0);
        }
        pass.draw(draw.range.clone(), 0..1);
    }
}

fn draw_figure(
    pass: &mut TrackedRenderPass,
    figure: &FigureDraw,
    rect: IRect,
    target: UVec2,
    depth: Range<f32>,
) {
    let scissor = rect.intersect(IRect::from_corners(IVec2::ZERO, target.as_ivec2()));
    if scissor.is_empty() {
        return;
    }
    let min = scissor.min.as_uvec2();
    let scissor_size = scissor.size().as_uvec2();
    let size = target.as_vec2();
    pass.set_viewport(0.0, 0.0, size.x, size.y, depth.start, depth.end);
    pass.set_scissor_rect(min.x, min.y, scissor_size.x, scissor_size.y);
    figure.encode(pass.wgpu_pass());
    pass.set_scissor_rect(0, 0, target.x, target.y);
}
