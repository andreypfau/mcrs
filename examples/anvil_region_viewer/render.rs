use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use bevy::core_pipeline::core_3d::{AlphaMask3d, CORE_3D_DEPTH_FORMAT, Opaque3d, main_opaque_pass_3d};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::globals::{GlobalsBuffer, GlobalsUniform};
use bevy::render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy::render::render_resource::binding_types::{
    sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_2d_array,
    uniform_buffer, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};

use crate::mesh::{Draw, Group, STREAMS};
use crate::probe::{self, GpuTimings, Queries};
use crate::pack::{MAX_SPRITE_ARRAYS, MODEL_OVERHANG, QUAD_WORDS, RegionGrid};

const TERRAIN_PIPELINES: usize = 6;
const UNTESTED_PIPELINE: usize = 4;

const PARAMS_STRIDE: u32 = 256;
const PARAMS_SIZE: u64 = 64;
const _: () = assert!(size_of::<Params>() as u64 == PARAMS_SIZE);

static UPLOAD_BUDGET: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| {
    std::env::var("ANVIL_UPLOAD")
        .ok()
        .and_then(|megabytes| megabytes.parse::<usize>().ok())
        .unwrap_or(4)
        << 20
});

const COPY_ALIGN: usize = 4;
const DRAW_ARGS_SIZE: u64 = size_of::<DrawArgs>() as u64;
const INSTANCE_COUNT_OFFSET: u64 = 4;
const TINT_LAYERS: u32 = 3;

const ADDITIVE: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    },
};

fn draw_sky<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    view_bind_group: &'pass BindGroup,
    view_offset: u32,
    pipeline_cache: &'pass PipelineCache,
    depth: bool,
) {
    let Some(sky) = terrain.sky_pipelines else {
        return;
    };
    pass.set_bind_group(1, &terrain.sky_bind_group, &[]);
    for (index, draw) in SKY_DRAWS.iter().enumerate() {
        if draw.depth != depth {
            continue;
        }
        let Some(pipeline) = pipeline_cache.get_render_pipeline(sky[index]) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, view_bind_group, &[view_offset, 0]);
        pass.draw(0..draw.vertices, 0..1);
    }
}

struct SkyDraw {
    label: &'static str,
    vertex: &'static str,
    fragment: &'static str,
    blend: Option<BlendState>,
    vertices: u32,
    depth: bool,
}

const SKY_DRAWS: [SkyDraw; 5] = [
    SkyDraw {
        label: "sky disc",
        vertex: "vertex_disc",
        fragment: "fragment_disc",
        blend: None,
        vertices: 48,
        depth: false,
    },
    SkyDraw {
        label: "sky twilight",
        vertex: "vertex_sunrise",
        fragment: "fragment_flat",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 48,
        depth: false,
    },
    SkyDraw {
        label: "sky celestial",
        vertex: "vertex_celestial",
        fragment: "fragment_celestial",
        blend: Some(ADDITIVE),
        vertices: 12,
        depth: false,
    },
    SkyDraw {
        label: "sky stars",
        vertex: "vertex_stars",
        fragment: "fragment_flat",
        blend: Some(ADDITIVE),
        vertices: STAR_COUNT * 6,
        depth: false,
    },
    SkyDraw {
        label: "sky clouds",
        vertex: "vertex_clouds",
        fragment: "fragment_clouds",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 3,
        depth: true,
    },
];

const STAR_COUNT: u32 = 1500;

pub struct Layout {
    pub grid: RegionGrid,
    pub min_section: [i32; 3],
    pub quad_capacity: usize,
    pub model_capacity: usize,
    pub face_capacity: usize,
    pub group_capacity: usize,
    pub cave_words: usize,
    pub celestials: Atlas,
    pub clouds: Atlas,
    pub tint_origin: [i32; 2],
    pub tint_size: [u32; 2],
}

impl Layout {
    pub fn max_draws(&self) -> usize {
        STREAMS * self.grid.len()
    }
}

pub struct Placement {
    pub quads: (u64, Vec<[u32; QUAD_WORDS]>),
    pub vertices: (u64, Vec<u32>),
    pub faces: (u64, Vec<u32>),
    pub groups: (u64, Vec<Group>),
    pub draws: Vec<Draw>,
}

impl Placement {
    fn part(&self, index: usize) -> (Arena, u64, &[u8]) {
        match index {
            0 => (Arena::Quads, self.quads.0, bytemuck::cast_slice(&self.quads.1)),
            1 => (
                Arena::Vertices,
                self.vertices.0,
                bytemuck::cast_slice(&self.vertices.1),
            ),
            2 => (
                Arena::Faces,
                self.faces.0,
                bytemuck::cast_slice(&self.faces.1),
            ),
            _ => (
                Arena::Groups,
                self.groups.0,
                bytemuck::cast_slice(&self.groups.1),
            ),
        }
    }
}

pub enum Upload {
    Tints {
        origin: [u32; 2],
        size: u32,
        data: Vec<u8>,
    },
    Sprites {
        atlases: Vec<Atlas>,
        animations: Vec<Animation>,
        animated_from: u32,
    },
    Geometry(Placement),
    Drop(u32),
}

#[derive(Resource, Clone, Default)]
pub struct Uploads(Arc<Mutex<Waiting>>);

#[derive(Default)]
pub struct Waiting {
    queue: VecDeque<Upload>,
    rebase: Option<Vec<(u32, u32)>>,
}

impl Uploads {
    pub fn push(&self, upload: Upload) {
        self.0.lock().unwrap().queue.push_back(upload);
    }

    pub fn rebase(&self, bases: Vec<(u32, u32)>) {
        self.0.lock().unwrap().rebase = Some(bases);
    }

    pub fn waiting(&self) -> usize {
        self.0.lock().unwrap().queue.len()
    }
}

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Animation {
    pub base_layer: u32,
    pub count: u32,
    pub frametime: u32,
    pub interpolate: u32,
}

pub struct Atlas {
    pub size: u32,
    pub layers: u32,
    pub mips: Vec<Vec<u8>>,
}

#[derive(Resource, Deref)]
struct WorldLayout(Arc<Layout>);

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
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    cave_base: u32,
    wireframe: u32,
    overhang: f32,
    animated_from: u32,
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    face_origin: u32,
}

#[derive(Resource, Clone, Copy, ExtractResource, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Sky {
    pub sky_light: [f32; 4],
    pub block_light: [f32; 4],
    pub ambient: [f32; 4],
    pub disc: [f32; 4],
    pub sunrise: [f32; 4],
    pub angles: [f32; 4],
    pub moon: [f32; 4],
    pub fog: [f32; 4],
    pub cloud_color: [f32; 4],
    pub cloud: [f32; 4],
}

#[derive(Resource, Clone, Copy, ExtractResource)]
pub struct Streams(pub u32);

const BLENDED_STREAM: u32 = STREAMS as u32 - 2;

impl Streams {
    pub const ALL: u32 = (1 << STREAMS) - 1;

    fn drawn(&self, stream: u32) -> bool {
        self.0 & (1 << stream) != 0
    }
}

impl Default for Streams {
    fn default() -> Self {
        Self(Self::ALL)
    }
}

#[derive(Resource, Clone, Copy, ExtractResource)]
pub struct Clouds(pub bool);

impl Default for Clouds {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Clone, Copy, ExtractResource)]
pub struct Raster(pub f32);

impl Default for Raster {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Resource, Clone, Copy, Default, ExtractResource)]
pub struct Wireframe(pub bool);

#[derive(Resource, Clone, Default)]
pub struct DrawnTriangles(Arc<ArgsReadback>);

impl DrawnTriangles {
    pub fn get(&self) -> u32 {
        self.0.triangles.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
struct ArgsReadback {
    triangles: AtomicU32,
    state: AtomicU8,
}

impl ArgsReadback {
    const IDLE: u8 = 0;
    const COPIED: u8 = 1;
    const MAPPING: u8 = 2;
}

pub struct TerrainPlugin(pub Arc<Layout>, pub Uploads);

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "layout.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "cull.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "terrain.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "sky.wgsl");

        let triangles = DrawnTriangles::default();
        let timings = GpuTimings::default();
        app.init_resource::<Wireframe>()
            .init_resource::<Sky>()
            .init_resource::<Clouds>()
            .init_resource::<Streams>()
            .add_plugins(ExtractResourcePlugin::<Wireframe>::default())
            .add_plugins(ExtractResourcePlugin::<Sky>::default())
            .add_plugins(ExtractResourcePlugin::<Clouds>::default())
            .add_plugins(ExtractResourcePlugin::<Streams>::default())
            .add_plugins(ExtractResourcePlugin::<Raster>::default())
            .insert_resource(triangles.clone())
            .insert_resource(timings.clone());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(triangles)
            .insert_resource(timings)
            .insert_resource(WorldLayout(self.0.clone()))
            .insert_resource(self.1.clone())
            .add_systems(RenderStartup, (init_terrain, probe::init))
            .add_systems(ExtractSchedule, extract_cave_visibility)
            .add_systems(
                Render,
                (
                    prepare_pipelines.in_set(RenderSystems::Prepare),
                    drop_unused_bins.in_set(RenderSystems::Prepare),
                    apply_uploads.in_set(RenderSystems::Prepare).before(prepare_wireframe),
                    prepare_wireframe.in_set(RenderSystems::Prepare),
                    prepare_sky.in_set(RenderSystems::Prepare),
                    prepare_view_bind_group.in_set(RenderSystems::PrepareBindGroups),
                    read_draw_args.in_set(RenderSystems::Cleanup),
                    probe::read.in_set(RenderSystems::Cleanup),
                ),
            )
            .add_systems(
                Core3d,
                (probe::resolve, cull_terrain).chain().in_set(Core3dSystems::Prepass),
            )
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
    layout: Arc<Layout>,
    quads: Buffer,
    vertices: Buffer,
    faces: Buffer,
    group_buffer: Buffer,
    animations: Buffer,
    args: Buffer,
    args_readback: Buffer,
    params: Buffer,
    sky: Buffer,
    cave: Buffer,
    visible: Buffer,
    tints: Texture,
    atlas_sampler: Sampler,
    tint_sampler: Sampler,
    view_layout: BindGroupLayoutDescriptor,
    cull_bind_group: BindGroup,
    draw_bind_group: BindGroup,
    cull_pipeline: CachedComputePipelineId,
    cull_stable_pipeline: CachedComputePipelineId,
    draw_layout: BindGroupLayoutDescriptor,
    sky_layout: BindGroupLayoutDescriptor,
    sky_bind_group: BindGroup,
    terrain_shader: Handle<Shader>,
    sky_shader: Handle<Shader>,
    #[expect(dead_code, reason = "held to keep the imported shader module loaded")]
    layout_shader: Handle<Shader>,
    pipelines: Option<[CachedRenderPipelineId; TERRAIN_PIPELINES]>,
    sky_pipelines: Option<[CachedRenderPipelineId; SKY_DRAWS.len()]>,
    draws: Vec<Draw>,
    group_counts: Vec<u32>,
    params_cpu: Vec<Params>,
    params_dirty: bool,
    animated_from: u32,
    wireframe: u32,
    pending: Option<Pending>,
}

struct Pending {
    placement: Placement,
    part: usize,
    done: usize,
}

#[derive(Copy, Clone)]
enum Arena {
    Quads,
    Vertices,
    Faces,
    Groups,
}

const ARENA_PARTS: usize = 4;

#[derive(Resource)]
struct ViewBindGroup(BindGroup);

fn init_terrain(
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

    let sky_layout = BindGroupLayoutDescriptor::new(
        "sky data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
            ),
        ),
    );

    let layout_shader: Handle<Shader> =
        asset_server.load("embedded://anvil_region_viewer/layout.wgsl");
    let cull_shader = asset_server.load("embedded://anvil_region_viewer/cull.wgsl");
    let terrain_shader: Handle<Shader> =
        asset_server.load("embedded://anvil_region_viewer/terrain.wgsl");
    let sky_shader: Handle<Shader> = asset_server.load("embedded://anvil_region_viewer/sky.wgsl");

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
fn draw_bind_group(
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
    let tint_view = tints.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
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
            &tint_view,
            tint_sampler,
            animations.as_entire_buffer_binding(),
            faces.as_entire_buffer_binding(),
        )),
    )
}

fn upload_atlases(
    atlases: &[Atlas],
    device: &RenderDevice,
    queue: &RenderQueue,
) -> (Vec<TextureView>, Sampler) {
    let limit = device.limits().max_texture_array_layers;
    let blank = Atlas {
        size: 1,
        layers: 1,
        mips: vec![vec![0u8; 4]],
    };
    let views = (0..MAX_SPRITE_ARRAYS)
        .map(|index| {
            let atlas = atlases.get(index).unwrap_or(&blank);
            assert!(
                atlas.layers <= limit,
                "{} sprites are {}x{}, but this device binds at most {limit} array layers",
                atlas.layers,
                atlas.size,
                atlas.size,
            );
            upload_atlas(atlas, &format!("terrain atlas {index}"), device, queue)
        })
        .collect();
    (views, atlas_sampler(device))
}

fn upload_atlas(
    atlas: &Atlas,
    label: &str,
    device: &RenderDevice,
    queue: &RenderQueue,
) -> TextureView {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: atlas.size,
            height: atlas.size,
            depth_or_array_layers: atlas.layers,
        },
        mip_level_count: atlas.mips.len() as u32,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, data) in atlas.mips.iter().enumerate() {
        let size = (atlas.size >> level).max(1);
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
                depth_or_array_layers: atlas.layers,
            },
        );
    }
    texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    })
}

fn atlas_sampler(device: &RenderDevice) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label: Some("terrain atlas"),
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: MipmapFilterMode::Linear,
        ..default()
    })
}

fn create_tints(layout: &Layout, device: &RenderDevice) -> (Texture, Sampler) {
    let [width, height] = layout.tint_size;
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("terrain tints"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: TINT_LAYERS,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("terrain tints"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    (texture, sampler)
}

fn apply_uploads(
    mut terrain: Option<ResMut<Terrain>>,
    uploads: Res<Uploads>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    let mut budget = *UPLOAD_BUDGET;

    if let Some(bases) = uploads.0.lock().unwrap().rebase.take() {
        for (region, base) in bases {
            for draw in terrain.draws.iter_mut().filter(|draw| draw.region == region) {
                draw.cave_base = base;
            }
        }
        rebuild_params(terrain);
    }

    loop {
        if terrain.pending.is_none() {
            let next = uploads.0.lock().unwrap().queue.pop_front();
            match next {
                None => break,
                Some(Upload::Tints { origin, size, data }) => {
                    write_tint_square(terrain, &queue, origin, size, &data);
                    budget = budget.saturating_sub(data.len());
                    if budget == 0 {
                        break;
                    }
                    continue;
                }
                Some(Upload::Sprites {
                    atlases,
                    animations,
                    animated_from,
                }) => {
                    let padding = [Animation::default()];
                    let spent: usize = atlases
                        .iter()
                        .flat_map(|atlas| atlas.mips.iter())
                        .map(|mip| mip.len())
                        .sum();
                    let (views, atlas_sampler) = upload_atlases(&atlases, &device, &queue);
                    terrain.atlas_sampler = atlas_sampler;
                    terrain.animations = device.create_buffer_with_data(&BufferInitDescriptor {
                        label: Some("terrain animations"),
                        contents: bytemuck::cast_slice(if animations.is_empty() {
                            &padding[..]
                        } else {
                            &animations
                        }),
                        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                    });
                    terrain.draw_bind_group = draw_bind_group(
                        &device,
                        &pipeline_cache,
                        &terrain.draw_layout,
                        &terrain.quads,
                        &terrain.vertices,
                        &terrain.visible,
                        &views,
                        &terrain.atlas_sampler,
                        &terrain.tints,
                        &terrain.tint_sampler,
                        &terrain.animations,
                        &terrain.faces,
                    );
                    terrain.animated_from = animated_from;
                    rebuild_params(terrain);
                    budget = budget.saturating_sub(spent);
                    if budget == 0 {
                        break;
                    }
                    continue;
                }
                Some(Upload::Drop(region)) => {
                    terrain.draws.retain(|draw| draw.region != region);
                    rebuild_params(terrain);
                    continue;
                }
                Some(Upload::Geometry(placement)) => {
                    terrain.pending = Some(Pending {
                        placement,
                        part: 0,
                        done: 0,
                    });
                }
            }
        }

        let mut pending = terrain.pending.take().expect("just filled");
        while budget > 0 && pending.part < ARENA_PARTS {
            let (arena, offset, data) = pending.placement.part(pending.part);
            if data.is_empty() {
                pending.part += 1;
                pending.done = 0;
                continue;
            }
            let left = data.len() - pending.done;
            let mut take = left.min(budget);
            if take < left {
                take -= take % COPY_ALIGN;
                if take == 0 {
                    break;
                }
            }
            let buffer = match arena {
                Arena::Quads => &terrain.quads,
                Arena::Vertices => &terrain.vertices,
                Arena::Faces => &terrain.faces,
                Arena::Groups => &terrain.group_buffer,
            };
            queue.write_buffer(
                buffer,
                offset + pending.done as u64,
                &data[pending.done..pending.done + take],
            );
            budget -= take;
            pending.done += take;
            if pending.done == data.len() {
                pending.part += 1;
                pending.done = 0;
            }
        }
        if pending.part < ARENA_PARTS {
            terrain.pending = Some(pending);
            break;
        }
        publish(terrain, pending.placement.draws);
        if budget == 0 {
            break;
        }
    }

    if terrain.params_dirty {
        write_params(terrain, &queue);
    }
}

fn publish(terrain: &mut Terrain, draws: Vec<Draw>) {
    terrain.draws.extend(draws);
    terrain.draws.sort_by_key(|draw| draw.stream);
    rebuild_params(terrain);
}

fn rebuild_params(terrain: &mut Terrain) {
    let animated_from = terrain.animated_from;
    let wireframe = terrain.wireframe;
    let tint_origin = terrain.layout.tint_origin;
    let tint_size = terrain.layout.tint_size;
    terrain.params_cpu.clear();
    terrain.group_counts.clear();
    let mut visible_base = 0u32;
    for (index, draw) in terrain.draws.iter().enumerate() {
        terrain.params_cpu.push(Params {
            group_base: draw.first_group,
            group_count: draw.group_count,
            visible_base,
            args_index: index as u32,
            origin_x: draw.origin[0],
            origin_y: draw.origin[1],
            origin_z: draw.origin[2],
            cave_base: draw.cave_base,
            wireframe,
            overhang: if draw.stream % 2 == 1 {
                MODEL_OVERHANG
            } else {
                0.0
            },
            animated_from,
            tint_origin_x: tint_origin[0],
            tint_origin_z: tint_origin[1],
            tint_span_x: tint_size[0] as f32,
            tint_span_z: tint_size[1] as f32,
            face_origin: draw.face_base,
        });
        visible_base += draw.quad_count;
        terrain.group_counts.push(draw.group_count);
    }
    terrain.params_dirty = true;
}

fn write_params(terrain: &mut Terrain, queue: &RenderQueue) {
    terrain.params_dirty = false;
    let stride = PARAMS_STRIDE as usize;
    let mut bytes = vec![0u8; terrain.params_cpu.len() * stride];
    for (index, entry) in terrain.params_cpu.iter().enumerate() {
        let at = index * stride;
        bytes[at..at + PARAMS_SIZE as usize].copy_from_slice(bytemuck::bytes_of(entry));
    }
    if !bytes.is_empty() {
        queue.write_buffer(&terrain.params, 0, &bytes);
    }
}

fn write_tint_square(
    terrain: &Terrain,
    queue: &RenderQueue,
    origin: [u32; 2],
    size: u32,
    data: &[u8],
) {
    let layer = (size * size * 4) as usize;
    for kind in 0..TINT_LAYERS {
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &terrain.tints,
                mip_level: 0,
                origin: Origin3d {
                    x: origin[0],
                    y: origin[1],
                    z: kind,
                },
                aspect: TextureAspect::All,
            },
            &data[kind as usize * layer..(kind as usize + 1) * layer],
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
    }
}

fn read_draw_args(terrain: Option<Res<Terrain>>, triangles: Res<DrawnTriangles>) {
    let Some(terrain) = terrain else {
        return;
    };
    if triangles
        .0
        .state
        .compare_exchange(
            ArgsReadback::COPIED,
            ArgsReadback::MAPPING,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return;
    }

    let buffer = terrain.args_readback.clone();
    let readback = triangles.0.clone();
    buffer.clone().slice(..).map_async(MapMode::Read, move |result| {
        if result.is_ok() {
            let view = buffer.slice(..).get_mapped_range();
            let args: &[DrawArgs] = bytemuck::cast_slice(&view);
            let count = args.iter().map(|arg| arg.instance_count).sum::<u32>() * 2;
            readback.triangles.store(count, Ordering::Relaxed);
            drop(view);
            buffer.unmap();
        }
        readback.state.store(ArgsReadback::IDLE, Ordering::Relaxed);
    });
}

fn prepare_wireframe(
    wireframe: Res<Wireframe>,
    mut terrain: Option<ResMut<Terrain>>,
    queue: Res<RenderQueue>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    let flag = u32::from(wireframe.0);
    if terrain.wireframe == flag {
        return;
    }
    terrain.wireframe = flag;
    rebuild_params(terrain);
    write_params(terrain, &queue);
}

fn prepare_sky(sky: Res<Sky>, terrain: Option<Res<Terrain>>, queue: Res<RenderQueue>) {
    let Some(terrain) = terrain else {
        return;
    };
    queue.write_buffer(&terrain.sky, 0, bytemuck::bytes_of(&*sky));
}

fn extract_cave_visibility(
    cave: Extract<Res<crate::cave::CaveCull>>,
    terrain: Option<Res<Terrain>>,
    queue: Res<RenderQueue>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    queue.write_buffer(&terrain.cave, 0, bytemuck::cast_slice(&cave.bits[..]));
}

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

    let mut pipelines = [CachedRenderPipelineId::INVALID; TERRAIN_PIPELINES];
    for (index, (entry, fragment, blend)) in [
        ("vertex_simple", "fragment_greedy_opaque", false),
        ("vertex_complex", "fragment_model_opaque", false),
        ("vertex_simple", "fragment_greedy_blend", true),
        ("vertex_complex", "fragment_model_blend", true),
        ("vertex_simple", "fragment_greedy_solid", false),
        ("vertex_complex", "fragment_model_solid", false),
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
                entry_point: Some(fragment.into()),
                targets: vec![Some(ColorTargetState {
                    format: view.target_format,
                    blend: blend.then_some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: (!blend).then_some(Face::Back),
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(!blend),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: if entry == "vertex_complex" && !blend {
                    DepthBiasState {
                        constant: 2,
                        slope_scale: 1.0,
                        clamp: 0.0,
                    }
                } else {
                    default()
                },
            }),
            multisample: MultisampleState {
                count: 1,
                ..default()
            },
            ..default()
        });
    }
    terrain.pipelines = Some(pipelines);

    let mut sky = [CachedRenderPipelineId::INVALID; SKY_DRAWS.len()];
    for (index, draw) in SKY_DRAWS.iter().enumerate() {
        sky[index] = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some(draw.label.into()),
            layout: vec![terrain.view_layout.clone(), terrain.sky_layout.clone()],
            vertex: VertexState {
                shader: terrain.sky_shader.clone(),
                entry_point: Some(draw.vertex.into()),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: terrain.sky_shader.clone(),
                entry_point: Some(draw.fragment.into()),
                targets: vec![Some(ColorTargetState {
                    format: view.target_format,
                    blend: draw.blend,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(draw.depth),
                depth_compare: Some(if draw.depth {
                    CompareFunction::GreaterEqual
                } else {
                    CompareFunction::Always
                }),
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
    terrain.sky_pipelines = Some(sky);
}

fn prepare_view_bind_group(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    globals: Res<GlobalsBuffer>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(globals_binding) = globals.buffer.binding() else {
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
            globals_binding,
            terrain.sky.as_entire_buffer_binding(),
        )),
    )));
}

fn cull_terrain(
    view: ViewQuery<&ViewUniformOffset>,
    terrain: Option<Res<Terrain>>,
    view_bind_group: Option<Res<ViewBindGroup>>,
    pipeline_cache: Res<PipelineCache>,
    triangles: Res<DrawnTriangles>,
    queries: Option<Res<Queries>>,
    timings: Res<GpuTimings>,
    streams: Res<Streams>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(view_bind_group)) = (terrain, view_bind_group) else {
        return;
    };
    let (Some(compacting), Some(stable)) = (
        pipeline_cache.get_compute_pipeline(terrain.cull_pipeline),
        pipeline_cache.get_compute_pipeline(terrain.cull_stable_pipeline),
    ) else {
        return;
    };
    let view_offset = view.into_inner().offset;

    if triangles
        .0
        .state
        .compare_exchange(
            ArgsReadback::IDLE,
            ArgsReadback::COPIED,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .is_ok()
    {
        let size = terrain.args_readback.size();
        ctx.command_encoder()
            .copy_buffer_to_buffer(&terrain.args, 0, &terrain.args_readback, 0, size);
    }

    for index in 0..terrain.draws.len() {
        ctx.command_encoder().clear_buffer(
            &terrain.args,
            index as u64 * DRAW_ARGS_SIZE + INSTANCE_COUNT_OFFSET,
            NonZeroU64::new(4).map(|n| n.get()),
        );
    }

    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let timestamps = queries.as_ref().map(|q| q.compute(probe::CULL, &timings));
    let mut pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("terrain cull"),
            timestamp_writes: timestamps,
        });
    let span = diagnostics.pass_span(&mut pass, "terrain_cull");
    pass.set_bind_group(1, &terrain.cull_bind_group, &[]);
    let mut blended = false;
    pass.set_pipeline(compacting);
    for (index, &workgroups) in terrain.group_counts.iter().enumerate() {
        if workgroups == 0 || !streams.drawn(terrain.draws[index].stream) {
            continue;
        }
        if !blended && terrain.draws[index].stream >= BLENDED_STREAM {
            blended = true;
            pass.set_pipeline(stable);
        }
        pass.set_bind_group(
            0,
            &view_bind_group.0,
            &[view_offset, index as u32 * PARAMS_STRIDE],
        );
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
    span.end(&mut pass);
}

fn drop_unused_bins(
    mut opaque: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    mut alpha_mask: ResMut<ViewBinnedRenderPhases<AlphaMask3d>>,
) {
    opaque.clear();
    alpha_mask.clear();
}

fn draw_terrain(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &ViewUniformOffset, &ExtractedView)>,
    terrain: Option<Res<Terrain>>,
    view_bind_group: Option<Res<ViewBindGroup>>,
    clouds: Res<Clouds>,
    streams: Res<Streams>,
    wireframe: Res<Wireframe>,
    raster: Res<Raster>,
    pipeline_cache: Res<PipelineCache>,
    queries: Option<Res<Queries>>,
    timings: Res<GpuTimings>,
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(view_bind_group)) = (terrain, view_bind_group) else {
        return;
    };
    let Some(pipelines) = terrain.pipelines else {
        return;
    };
    let (target, depth, view_offset, extracted) = view.into_inner();
    let color_attachments = [Some(target.get_color_attachment())];
    let depth_attachment = Some(depth.get_attachment(StoreOp::Store));
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let timestamps = queries.as_ref().map(|q| q.render(probe::TERRAIN, &timings));
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("terrain"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: timestamps,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "terrain_draw");

    if raster.0 < 1.0 {
        let size = extracted.viewport.zw().as_vec2() * raster.0;
        pass.set_viewport(0.0, 0.0, size.x.max(1.0), size.y.max(1.0), 0.0, 1.0);
    }

    draw_sky(&mut pass, &terrain, &view_bind_group.0, view_offset.offset, &pipeline_cache, false);

    pass.set_bind_group(1, &terrain.draw_bind_group, &[]);

    let mut clouds_drawn = false;
    for (index, draw) in terrain.draws.iter().enumerate() {
        if draw.quad_count == 0 || !streams.drawn(draw.stream) {
            continue;
        }
        if clouds.0 && !clouds_drawn && draw.stream >= BLENDED_STREAM {
            clouds_drawn = true;
            draw_sky(
                &mut pass,
                &terrain,
                &view_bind_group.0,
                view_offset.offset,
                &pipeline_cache,
                true,
            );
            pass.set_bind_group(1, &terrain.draw_bind_group, &[]);
        }
        let stream = draw.stream as usize;
        let pipeline_index = match stream {
            0 | 1 if !wireframe.0 => UNTESTED_PIPELINE + stream,
            _ => (stream / 4) * 2 + stream % 2,
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipelines[pipeline_index]) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(
            0,
            &view_bind_group.0,
            &[view_offset.offset, index as u32 * PARAMS_STRIDE],
        );
        pass.draw_indirect(&terrain.args, index as u64 * DRAW_ARGS_SIZE);
    }

    if clouds.0 && !clouds_drawn {
        draw_sky(&mut pass, &terrain, &view_bind_group.0, view_offset.offset, &pipeline_cache, true);
    }

    span.end(&mut pass);
}
