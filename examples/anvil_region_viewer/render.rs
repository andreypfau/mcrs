//! The GPU side: one static arena, a compute culling pass, and one indirect draw per stream.
//!
//! Nothing about the region changes after load, so every buffer is filled once in `RenderStartup`
//! and never touched again. Per frame the only writes are a 24-byte clear of the draw counters and
//! two bind-group rebuilds; no allocation, no upload, no per-section entity, no `Handle<Mesh>`.

use std::num::NonZeroU64;
use std::sync::Arc;

use bevy::core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, main_opaque_pass_3d};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_2d_array,
    uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

use crate::mesh::{Group, STREAMS, StreamSpan};

/// Dynamic uniform offsets must be a multiple of the device's alignment; 256 satisfies every
/// backend we can land on, and the whole table is six entries.
const PARAMS_STRIDE: u32 = 256;
const PARAMS_SIZE: u64 = 32;
const TINT_SIZE: u32 = 512;
const TINT_LAYERS: u32 = 3;

/// Everything the renderer needs, built on the main thread before the app starts.
pub struct Geometry {
    pub simple: Vec<u64>,
    pub complex: Vec<u32>,
    pub groups: Vec<Group>,
    pub streams: [StreamSpan; STREAMS],
    pub min_section_y: i32,
    pub atlas_size: u32,
    pub atlas_layers: u32,
    /// Mip 0 first, then each smaller level, layer-major within a level.
    pub atlas_mips: Vec<Vec<u8>>,
    /// `TINT_LAYERS` layers of `TINT_SIZE`², RGBA8, indexed by world x and z.
    pub tint_map: Vec<u8>,
}

#[derive(Resource, Deref)]
struct StaticGeometry(Arc<Geometry>);

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct DrawArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    min_section_y: i32,
    reserved: [i32; 3],
}

pub struct TerrainPlugin(pub Arc<Geometry>);

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "cull.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "terrain.wgsl");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(StaticGeometry(self.0.clone()))
            .add_systems(RenderStartup, init_terrain)
            .add_systems(
                Render,
                (
                    prepare_pipelines.in_set(RenderSystems::Prepare),
                    prepare_view_bind_group.in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(Core3d, cull_terrain.in_set(Core3dSystems::Prepass))
            .add_systems(
                Core3d,
                draw_terrain
                    .in_set(Core3dSystems::MainPass)
                    .after(main_opaque_pass_3d),
            );
    }
}

#[derive(Resource)]
struct Terrain {
    args: Buffer,
    params: Buffer,
    view_layout: BindGroupLayoutDescriptor,
    cull_bind_group: BindGroup,
    draw_bind_group: BindGroup,
    cull_pipeline: CachedComputePipelineId,
    draw_layout: BindGroupLayoutDescriptor,
    terrain_shader: Handle<Shader>,
    /// `[simple opaque, complex opaque, simple blended, complex blended]`, queued once the first
    /// view reveals the colour format the pipelines have to match.
    pipelines: Option<[CachedRenderPipelineId; 4]>,
    streams: [StreamSpan; STREAMS],
    /// Workgroups to dispatch per stream, one per culling group.
    group_counts: [u32; STREAMS],
}

#[derive(Resource)]
struct ViewBindGroup(BindGroup);

fn init_terrain(
    mut commands: Commands,
    geometry: Res<StaticGeometry>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let quads = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain quads"),
        contents: bytemuck::cast_slice(&geometry.simple),
        usage: BufferUsages::STORAGE,
    });
    let vertices = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain vertices"),
        contents: bytemuck::cast_slice(&geometry.complex),
        usage: BufferUsages::STORAGE,
    });
    let groups = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain groups"),
        contents: bytemuck::cast_slice(&geometry.groups),
        usage: BufferUsages::STORAGE,
    });

    // Worst case every quad survives culling, so the visible list is sized for the whole arena and
    // each stream owns a fixed slice of it. No allocation can ever be needed at draw time.
    let mut visible_base = 0u32;
    let mut params = vec![Params::default(); STREAMS * (PARAMS_STRIDE as usize / PARAMS_SIZE as usize)];
    let mut group_counts = [0u32; STREAMS];
    for stream in 0..STREAMS {
        let span = geometry.streams[stream];
        params[stream * (PARAMS_STRIDE as usize / PARAMS_SIZE as usize)] = Params {
            group_base: span.first_group,
            group_count: span.group_count,
            visible_base,
            args_index: stream as u32,
            min_section_y: geometry.min_section_y,
            reserved: [0; 3],
        };
        group_counts[stream] = span.group_count;
        visible_base += span.quad_count;
    }
    let visible = device.create_buffer(&BufferDescriptor {
        label: Some("terrain visible list"),
        size: (visible_base.max(1) as u64) * 4,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let params = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain stream params"),
        contents: bytemuck::cast_slice(&params),
        usage: BufferUsages::UNIFORM,
    });

    let args_init = [DrawArgs {
        vertex_count: 6,
        instance_count: 0,
        first_vertex: 0,
        first_instance: 0,
    }; STREAMS];
    let args = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain draw args"),
        contents: bytemuck::cast_slice(&args_init),
        usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
    });

    let (atlas, atlas_sampler) = upload_atlas(&geometry, &device, &queue);
    let (tints, tint_sampler) = upload_tints(&geometry, &device, &queue);

    let view_layout = BindGroupLayoutDescriptor::new(
        "terrain view",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            (
                uniform_buffer_sized(true, Some(ViewUniform::min_size())),
                uniform_buffer_sized(true, NonZeroU64::new(PARAMS_SIZE)),
            ),
        ),
    );
    let cull_layout = BindGroupLayoutDescriptor::new(
        "terrain cull data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
            ),
        ),
    );
    let draw_layout = BindGroupLayoutDescriptor::new(
        "terrain draw data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );

    let cull_shader = asset_server.load("embedded://anvil_region_viewer/cull.wgsl");
    let terrain_shader: Handle<Shader> =
        asset_server.load("embedded://anvil_region_viewer/terrain.wgsl");

    let cull_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("terrain cull".into()),
        layout: vec![view_layout.clone(), cull_layout.clone()],
        shader: cull_shader,
        entry_point: Some("cull".into()),
        ..default()
    });

    let cull_bind_group = device.create_bind_group(
        "terrain cull",
        &pipeline_cache.get_bind_group_layout(&cull_layout),
        &BindGroupEntries::sequential((
            groups.as_entire_buffer_binding(),
            visible.as_entire_buffer_binding(),
            args.as_entire_buffer_binding(),
        )),
    );
    let draw_bind_group = device.create_bind_group(
        "terrain draw",
        &pipeline_cache.get_bind_group_layout(&draw_layout),
        &BindGroupEntries::sequential((
            quads.as_entire_buffer_binding(),
            vertices.as_entire_buffer_binding(),
            visible.as_entire_buffer_binding(),
            &atlas,
            &atlas_sampler,
            &tints,
            &tint_sampler,
        )),
    );

    commands.insert_resource(Terrain {
        args,
        params,
        view_layout,
        cull_bind_group,
        draw_bind_group,
        cull_pipeline,
        draw_layout,
        terrain_shader,
        pipelines: None,
        streams: geometry.streams,
        group_counts,
    });
}

fn upload_atlas(
    geometry: &Geometry,
    device: &RenderDevice,
    queue: &RenderQueue,
) -> (TextureView, Sampler) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("terrain atlas"),
        size: Extent3d {
            width: geometry.atlas_size,
            height: geometry.atlas_size,
            depth_or_array_layers: geometry.atlas_layers.max(1),
        },
        mip_level_count: geometry.atlas_mips.len() as u32,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, data) in geometry.atlas_mips.iter().enumerate() {
        let size = (geometry.atlas_size >> level).max(1);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: geometry.atlas_layers.max(1),
            },
        );
    }
    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    // Nearest within a level keeps the pixel art crisp; linear between levels stops distant terrain
    // from boiling. `Repeat` is what lets a greedy quad tile its sprite `w × h` times for free.
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("terrain atlas"),
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: MipmapFilterMode::Linear,
        ..default()
    });
    (view, sampler)
}

fn upload_tints(
    geometry: &Geometry,
    device: &RenderDevice,
    queue: &RenderQueue,
) -> (TextureView, Sampler) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("terrain tints"),
        size: Extent3d {
            width: TINT_SIZE,
            height: TINT_SIZE,
            depth_or_array_layers: TINT_LAYERS,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &geometry.tint_map,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(TINT_SIZE * 4),
            rows_per_image: Some(TINT_SIZE),
        },
        Extent3d {
            width: TINT_SIZE,
            height: TINT_SIZE,
            depth_or_array_layers: TINT_LAYERS,
        },
    );
    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    // Linear sampling across the biome map is what makes the transition between two biomes' grass
    // colours a gradient instead of a hard seam down the middle of a chunk.
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("terrain tints"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    (view, sampler)
}

/// The colour format a pipeline must declare is the view's, and no view exists yet when
/// `RenderStartup` runs — so the four raster pipelines are queued the first frame a view appears.
/// There is exactly one target here, so this happens once and never specialises again.
fn prepare_pipelines(
    mut terrain: Option<ResMut<Terrain>>,
    views: Query<&ExtractedView>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    if terrain.pipelines.is_some() {
        return;
    }
    let Some(view) = views.iter().next() else {
        return;
    };

    let mut pipelines = [CachedRenderPipelineId::INVALID; 4];
    for (index, (entry, blend)) in [
        ("vertex_simple", false),
        ("vertex_complex", false),
        ("vertex_simple", true),
        ("vertex_complex", true),
    ]
    .into_iter()
    .enumerate()
    {
        pipelines[index] = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("terrain".into()),
            layout: vec![terrain.view_layout.clone(), terrain.draw_layout.clone()],
            vertex: VertexState {
                shader: terrain.terrain_shader.clone(),
                entry_point: Some(entry.into()),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: terrain.terrain_shader.clone(),
                entry_point: Some(
                    if blend { "fragment_blend" } else { "fragment_opaque" }.into(),
                ),
                targets: vec![Some(ColorTargetState {
                    format: view.target_format,
                    blend: blend.then_some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                // The translucent pass must not occlude what is behind it.
                depth_write_enabled: Some(!blend),
                // Bevy renders with a reversed depth buffer, so nearer fragments compare greater.
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: default(),
            }),
            multisample: MultisampleState {
                count: 1,
                ..default()
            },
            ..default()
        });
    }
    terrain.pipelines = Some(pipelines);
}

fn prepare_view_bind_group(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    commands.insert_resource(ViewBindGroup(device.create_bind_group(
        "terrain view",
        &pipeline_cache.get_bind_group_layout(&terrain.view_layout),
        &BindGroupEntries::sequential((
            view_binding,
            BufferBinding {
                buffer: &terrain.params,
                offset: 0,
                size: NonZeroU64::new(PARAMS_SIZE),
            },
        )),
    )));
}

fn cull_terrain(
    view: ViewQuery<&ViewUniformOffset>,
    terrain: Option<Res<Terrain>>,
    view_bind_group: Option<Res<ViewBindGroup>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(view_bind_group)) = (terrain, view_bind_group) else {
        return;
    };
    let Some(pipeline) = pipeline_cache.get_compute_pipeline(terrain.cull_pipeline) else {
        return;
    };
    let view_offset = view.into_inner().offset;

    // Reset only the instance counters; the rest of each draw command is constant for the run of
    // the program, so there is nothing else to upload per frame.
    for stream in 0..STREAMS {
        ctx.command_encoder()
            .clear_buffer(&terrain.args, stream as u64 * 16 + 4, NonZeroU64::new(4).map(|n| n.get()));
    }

    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("terrain cull"),
            timestamp_writes: None,
        });
    let span = diagnostics.pass_span(&mut pass, "terrain_cull");
    pass.set_pipeline(pipeline);
    pass.set_bind_group(1, &terrain.cull_bind_group, &[]);
    for stream in 0..STREAMS {
        let workgroups = terrain.group_counts[stream];
        if workgroups == 0 {
            continue;
        }
        pass.set_bind_group(
            0,
            &view_bind_group.0,
            &[view_offset, stream as u32 * PARAMS_STRIDE],
        );
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
    span.end(&mut pass);
}

fn draw_terrain(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &ViewUniformOffset)>,
    terrain: Option<Res<Terrain>>,
    view_bind_group: Option<Res<ViewBindGroup>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(view_bind_group)) = (terrain, view_bind_group) else {
        return;
    };
    let Some(pipelines) = terrain.pipelines else {
        return;
    };
    let (target, depth, view_offset) = view.into_inner();
    let color_attachments = [Some(target.get_color_attachment())];
    let depth_attachment = Some(depth.get_attachment(StoreOp::Store));
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("terrain"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "terrain_draw");
    pass.set_bind_group(1, &terrain.draw_bind_group, &[]);

    for stream in 0..STREAMS {
        if terrain.streams[stream].quad_count == 0 {
            continue;
        }
        // Solid and cutout share a pipeline (the alpha test never fires on a solid sprite); only
        // the translucent pass differs, and it must come last because it does not write depth.
        let pipeline_index = (stream / 4) * 2 + stream % 2;
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipelines[pipeline_index]) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(
            0,
            &view_bind_group.0,
            &[view_offset.offset, stream as u32 * PARAMS_STRIDE],
        );
        pass.draw_indirect(&terrain.args, stream as u64 * 16);
    }
    span.end(&mut pass);
}
