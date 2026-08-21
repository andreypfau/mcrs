use std::num::NonZeroU64;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::globals::GlobalsUniform;
use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_2d_array,
    uniform_buffer, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::ViewUniform;

use crate::mesh::{Draw, Group};
use crate::pack::{MAX_SPRITE_ARRAYS, QUAD_WORDS};

use super::params::{PARAMS_SIZE, PARAMS_STRIDE, Params};
use super::pipeline::TERRAIN_PIPELINES;
use super::sky::{self, SKY_DRAWS};
use super::stats::DrawArgs;
use super::texture::{array_view, create_tints, upload_atlas, upload_atlases};
use super::upload::Pending;
use super::{Animation, Layout, Sky, WorldLayout};

#[derive(Resource)]
pub(super) struct Terrain {
    pub layout: Arc<Layout>,
    pub quads: Buffer,
    pub vertices: Buffer,
    pub faces: Buffer,
    pub group_buffer: Buffer,
    pub animations: Buffer,
    pub args: Buffer,
    pub args_readback: Buffer,
    pub params: Buffer,
    pub sky: Buffer,
    pub cave: Buffer,
    pub visible: Buffer,
    pub tints: Texture,
    pub atlas_sampler: Sampler,
    pub tint_sampler: Sampler,
    pub view_layout: BindGroupLayoutDescriptor,
    pub cull_bind_group: BindGroup,
    pub draw_bind_group: BindGroup,
    pub cull_pipeline: CachedComputePipelineId,
    pub cull_stable_pipeline: CachedComputePipelineId,
    pub draw_layout: BindGroupLayoutDescriptor,
    pub sky_layout: BindGroupLayoutDescriptor,
    pub sky_bind_group: BindGroup,
    pub terrain_shader: Handle<Shader>,
    pub sky_shader: Handle<Shader>,
    #[expect(dead_code, reason = "held to keep the imported shader module loaded")]
    pub layout_shader: Handle<Shader>,
    pub pipelines: Option<[CachedRenderPipelineId; TERRAIN_PIPELINES]>,
    pub sky_pipelines: Option<[CachedRenderPipelineId; SKY_DRAWS.len()]>,
    pub draws: Vec<Draw>,
    pub group_counts: Vec<u32>,
    pub params_cpu: Vec<Params>,
    pub params_dirty: bool,
    pub animated_from: u32,
    pub wireframe: u32,
    pub pending: Option<Pending>,
}

pub(super) fn init_terrain(
    mut commands: Commands,
    layout: Res<WorldLayout>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = layout.0.clone();
    let arena = |label, bytes: u64| {
        device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size: bytes.max(size_of::<[u32; QUAD_WORDS]>() as u64),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };
    let quads = arena(
        "terrain quads",
        (layout.quad_capacity * QUAD_WORDS * 4) as u64,
    );
    let vertices = arena(
        "terrain vertices",
        (layout.model_capacity * 4 * 3 * 4) as u64,
    );
    let faces = arena("terrain faces", (layout.face_capacity * 4) as u64);
    let group_buffer = arena(
        "terrain groups",
        (layout.group_capacity * size_of::<Group>()) as u64,
    );
    let visible = arena(
        "terrain visible list",
        ((layout.quad_capacity + layout.model_capacity) * 4) as u64,
    );
    let animations = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain animations"),
        contents: bytemuck::cast_slice(&[Animation::default()]),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });
    let cave = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain cave visibility"),
        contents: bytemuck::cast_slice(&vec![u32::MAX; layout.cave_words.max(1)]),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    let max_draws = layout.max_draws().max(1);
    let params = device.create_buffer(&BufferDescriptor {
        label: Some("terrain draw params"),
        size: max_draws as u64 * PARAMS_STRIDE as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let args_init = vec![
        DrawArgs {
            vertex_count: 4,
            instance_count: 0,
            first_vertex: 0,
            first_instance: 0,
        };
        max_draws
    ];
    let args = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain draw args"),
        contents: bytemuck::cast_slice(&args_init),
        usage: BufferUsages::STORAGE
            | BufferUsages::INDIRECT
            | BufferUsages::COPY_DST
            | BufferUsages::COPY_SRC,
    });
    let sky = device.create_buffer(&BufferDescriptor {
        label: Some("terrain sky"),
        size: size_of::<Sky>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let args_readback = device.create_buffer(&BufferDescriptor {
        label: Some("terrain draw args readback"),
        size: size_of_val(args_init.as_slice()) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let (atlases, atlas_sampler) = upload_atlases(&[], &device, &queue);
    let celestials = upload_atlas(&layout.celestials, "celestials", &device, &queue);
    let clouds = upload_atlas(&layout.clouds, "clouds", &device, &queue);
    let celestial_sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("celestials"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..default()
    });
    let (tints, tint_sampler) = create_tints(&layout, &device);

    let view_layout = BindGroupLayoutDescriptor::new(
        "terrain view",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
            (
                uniform_buffer_sized(true, Some(ViewUniform::min_size())),
                uniform_buffer_sized(true, NonZeroU64::new(PARAMS_SIZE)),
                uniform_buffer::<GlobalsUniform>(false),
                uniform_buffer_sized(false, NonZeroU64::new(size_of::<Sky>() as u64)),
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
                storage_buffer_read_only_sized(false, None),
            ),
        ),
    );
    const _: () = assert!(MAX_SPRITE_ARRAYS == 4);
    let draw_layout = BindGroupLayoutDescriptor::new(
        "terrain draw data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                storage_buffer_read_only_sized(false, None),
                storage_buffer_read_only_sized(false, None),
            ),
        ),
    );

    let sky_layout = sky::bind_group_layout();

    let layout_shader: Handle<Shader> =
        asset_server.load("embedded://anvil_region_viewer/render/shaders/layout.wgsl");
    let cull_shader = asset_server.load("embedded://anvil_region_viewer/render/shaders/cull.wgsl");
    let terrain_shader: Handle<Shader> =
        asset_server.load("embedded://anvil_region_viewer/render/shaders/terrain.wgsl");
    let sky_shader: Handle<Shader> =
        asset_server.load("embedded://anvil_region_viewer/render/shaders/sky.wgsl");

    let cull_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("terrain cull".into()),
        layout: vec![view_layout.clone(), cull_layout.clone()],
        shader: cull_shader.clone(),
        entry_point: Some("cull".into()),
        ..default()
    });
    let cull_stable_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("terrain cull stable".into()),
        layout: vec![view_layout.clone(), cull_layout.clone()],
        shader: cull_shader,
        entry_point: Some("cull_stable".into()),
        ..default()
    });

    let cull_bind_group = device.create_bind_group(
        "terrain cull",
        &pipeline_cache.get_bind_group_layout(&cull_layout),
        &BindGroupEntries::sequential((
            group_buffer.as_entire_buffer_binding(),
            visible.as_entire_buffer_binding(),
            args.as_entire_buffer_binding(),
            cave.as_entire_buffer_binding(),
        )),
    );
    let draw_bind_group = draw_bind_group(
        &device,
        &pipeline_cache,
        &draw_layout,
        &quads,
        &vertices,
        &visible,
        &atlases,
        &atlas_sampler,
        &tints,
        &tint_sampler,
        &animations,
        &faces,
    );

    let sky_bind_group = device.create_bind_group(
        "sky",
        &pipeline_cache.get_bind_group_layout(&sky_layout),
        &BindGroupEntries::sequential((&celestials, &celestial_sampler, &clouds)),
    );

    commands.insert_resource(Terrain {
        params_cpu: Vec::with_capacity(max_draws),
        params_dirty: false,
        animated_from: 0,
        wireframe: 0,
        pending: None,
        draws: Vec::new(),
        group_counts: Vec::new(),
        layout,
        quads,
        vertices,
        faces,
        group_buffer,
        animations,
        args,
        args_readback,
        params,
        sky,
        cave,
        visible,
        tints,
        atlas_sampler,
        tint_sampler,
        view_layout,
        cull_bind_group,
        draw_bind_group,
        cull_pipeline,
        cull_stable_pipeline,
        draw_layout,
        sky_layout,
        sky_bind_group,
        terrain_shader,
        sky_shader,
        layout_shader,
        pipelines: None,
        sky_pipelines: None,
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_bind_group(
    device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    layout: &BindGroupLayoutDescriptor,
    quads: &Buffer,
    vertices: &Buffer,
    visible: &Buffer,
    atlases: &[TextureView],
    atlas_sampler: &Sampler,
    tints: &Texture,
    tint_sampler: &Sampler,
    animations: &Buffer,
    faces: &Buffer,
) -> BindGroup {
    device.create_bind_group(
        "terrain draw",
        &pipeline_cache.get_bind_group_layout(layout),
        &BindGroupEntries::sequential((
            quads.as_entire_buffer_binding(),
            vertices.as_entire_buffer_binding(),
            visible.as_entire_buffer_binding(),
            &atlases[0],
            &atlases[1],
            &atlases[2],
            &atlases[3],
            atlas_sampler,
            &array_view(tints),
            tint_sampler,
            animations.as_entire_buffer_binding(),
            faces.as_entire_buffer_binding(),
        )),
    )
}
