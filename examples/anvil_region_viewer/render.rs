//! The GPU side: arenas of geometry, a compute culling pass, and one indirect draw per stream.
//!
//! Every buffer is created at its full size before anything is loaded, because the shape of the
//! world — how many render regions, how many sections, how many draws at most — is settled by the
//! window rather than by what turns out to be in it. Geometry then arrives from the loader a
//! render region at a time and is written into the arenas at offsets the loader picked. A region's
//! draws are published only once every byte of it has landed, and no more than a fixed number of
//! bytes go up in one frame, so a region coming into view costs a slice of several frames rather
//! than one long one.

use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use bevy::core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, main_opaque_pass_3d};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::globals::{GlobalsBuffer, GlobalsUniform};
use bevy::render::render_phase::TrackedRenderPass;
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
use crate::pack::{MAX_SPRITE_ARRAYS, MODEL_OVERHANG, QUAD_WORDS, RegionGrid};

/// Dynamic uniform offsets must be a multiple of the device's alignment; 256 satisfies every
/// backend we can land on.
const PARAMS_STRIDE: u32 = 256;
const PARAMS_SIZE: u64 = 64;
const _: () = assert!(size_of::<Params>() as u64 == PARAMS_SIZE);

/// How many bytes of geometry reach the GPU in one frame, from `ANVIL_UPLOAD` in megabytes.
///
/// A region file runs to a couple of hundred megabytes, and handing it over in fewer, larger
/// pieces lands as a stall on the frame a region comes into view. Measured on this world at 4K:
/// four megabytes a frame holds the loading tail at 34 ms against a settled 30, sixteen pushes it
/// to 80, and no limit at all to 63. Four also finishes sooner, because the main thread it is not
/// stalling is the one driving the loader.
static UPLOAD_BUDGET: std::sync::LazyLock<usize> = std::sync::LazyLock::new(|| {
    std::env::var("ANVIL_UPLOAD")
        .ok()
        .and_then(|megabytes| megabytes.parse::<usize>().ok())
        .unwrap_or(4)
        << 20
});

/// Buffer writes have to start and end on a four-byte boundary, so a slice of a large upload is
/// cut to one.
const COPY_ALIGN: usize = 4;
const DRAW_ARGS_SIZE: u64 = size_of::<DrawArgs>() as u64;
/// Byte offset of `DrawArgs::instance_count`, the only field a frame rewrites.
const INSTANCE_COUNT_OFFSET: u64 = 4;
const TINT_LAYERS: u32 = 3;

/// Vanilla's own additive blend for the sun, the moon and the stars: what is drawn is added to the
/// sky behind it rather than covering it, which is what makes the black square a sprite is painted
/// on disappear.
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

/// Either half of the sky: the backdrop behind the terrain, or the clouds in front of it. Which
/// half is which is the draw's own `depth` flag, so the order lives in the table rather than here.
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

/// The sky, in the order `LevelRenderer` draws it: the disc, the twilight band, the two celestial
/// bodies, then the stars over the top of them. Each is one draw of vertices the shader derives
/// from their own indices, so none of them has a buffer behind it.
struct SkyDraw {
    label: &'static str,
    vertex: &'static str,
    fragment: &'static str,
    blend: Option<BlendState>,
    vertices: u32,
    /// Whether the draw belongs in front of the terrain rather than behind it. The backdrop takes
    /// no part in the depth buffer at all; the clouds hang inside the world and have to be told
    /// what is standing in front of them.
    depth: bool,
}

const SKY_DRAWS: [SkyDraw; 5] = [
    SkyDraw {
        label: "sky disc",
        vertex: "vertex_disc",
        fragment: "fragment_disc",
        blend: None,
        // The disc above the horizon and the dark one below it, eight triangles each.
        vertices: 48,
        depth: false,
    },
    SkyDraw {
        label: "sky twilight",
        vertex: "vertex_sunrise",
        fragment: "fragment_flat",
        blend: Some(BlendState::ALPHA_BLENDING),
        // A sixteen step fan.
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
        // One triangle over the whole screen; the layer is found by marching, not by drawing it.
        vertices: 3,
        depth: true,
    },
];

/// How many stars vanilla tries to place. The shader rejects the same share of them as vanilla
/// does, so rather fewer than this actually reach the screen.
const STAR_COUNT: u32 = 1500;

/// The shape of the world, settled before anything is loaded.
///
/// None of it depends on what the region files turn out to hold: the window fixes how many render
/// regions there are and how tall the world is, and everything sized from that can be allocated
/// once. A span that grew after geometry was already on the GPU would renumber every region's
/// place in the sight-line bitset under it.
pub struct Layout {
    pub grid: RegionGrid,
    /// Section coordinates of the window's corner, signed on all three axes.
    pub min_section: [i32; 3],
    /// Greedy quads the arena holds.
    pub quad_capacity: usize,
    /// Model quads the arena holds, four vertices each.
    pub model_capacity: usize,
    pub group_capacity: usize,
    /// Words of the sight-line bitset the walk hands back every frame.
    pub cave_words: usize,
    /// The sun and the eight moon phases, one square layer each, in that order.
    pub celestials: Atlas,
    /// One texel per cloud cell.
    pub clouds: Atlas,
    /// The window the biome colour map covers: its corner in world blocks and its extent.
    pub tint_origin: [i32; 2],
    pub tint_size: [u32; 2],
}

impl Layout {
    /// One `draw_indirect` per stream per render region is the most the world can ever issue.
    pub fn max_draws(&self) -> usize {
        STREAMS * self.grid.len()
    }
}

/// Geometry already given its place in the arenas, waiting to go to the GPU.
///
/// The offsets were picked by the loader, so the render world only writes bytes. The draws are
/// published once every byte has landed, which is what stops a frame from ever drawing out of a
/// half-written arena.
pub struct Placement {
    /// `(byte offset, quads)` into the greedy quad arena.
    pub quads: (u64, Vec<[u32; QUAD_WORDS]>),
    /// `(byte offset, words)` into the model vertex arena.
    pub vertices: (u64, Vec<u32>),
    /// `(byte offset, groups)` into the culling group arena.
    pub groups: (u64, Vec<Group>),
    /// This region's draws, in stream order.
    pub draws: Vec<Draw>,
}

impl Placement {
    /// The three arenas in the order they are written, as bytes. Kept as a borrow rather than
    /// packed into one buffer: a region file runs to hundreds of megabytes and copying it once
    /// more on the way to the GPU would undo the point of spreading the write out.
    fn part(&self, index: usize) -> (Arena, u64, &[u8]) {
        match index {
            0 => (Arena::Quads, self.quads.0, bytemuck::cast_slice(&self.quads.1)),
            1 => (
                Arena::Vertices,
                self.vertices.0,
                bytemuck::cast_slice(&self.vertices.1),
            ),
            _ => (
                Arena::Groups,
                self.groups.0,
                bytemuck::cast_slice(&self.groups.1),
            ),
        }
    }
}

/// Work the loader hands the render world.
pub enum Upload {
    /// One region file's biome colours, at its corner in the tint window.
    Tints {
        origin: [u32; 2],
        size: u32,
        data: Vec<u8>,
    },
    /// The sprite set grew. Every array, the animation table and the bind group that names them
    /// are rebuilt together, because a layer number only means anything next to the array it
    /// indexes and the threshold that separates a still sprite from an animation moves with them.
    Sprites {
        atlases: Vec<Atlas>,
        animations: Vec<Animation>,
        animated_from: u32,
    },
    Geometry(Placement),
}

/// The queue the loader pushes into and the render world drains. An `Arc` rather than an extracted
/// resource: the payloads are megabytes and extraction would clone every one of them.
#[derive(Resource, Clone, Default)]
pub struct Uploads(Arc<Mutex<VecDeque<Upload>>>);

impl Uploads {
    pub fn push(&self, upload: Upload) {
        self.0.lock().unwrap().push_back(upload);
    }

    /// How much is still queued. Geometry reaches the GPU a slice at a time, so the loader going
    /// quiet is not the same thing as the world being on screen.
    pub fn waiting(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

/// One animated sprite: where its run of layers starts, how long the run is, and how fast to walk
/// it. The sequence was laid out one step to a layer, so this is all the shader needs to find the
/// step showing now.
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Animation {
    pub base_layer: u32,
    pub count: u32,
    pub frametime: u32,
    pub interpolate: u32,
}

/// One sprite resolution, ready to upload.
pub struct Atlas {
    pub size: u32,
    pub layers: u32,
    /// Mip 0 first, then each smaller level, layer-major within a level.
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
    /// The corner of this draw's render region, in blocks. Coordinates in a quad are relative to
    /// their own section, so this is what puts the geometry back where it belongs. Explicit
    /// scalars rather than a vec3: a vec3 would align to 16 and silently grow the struct.
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    /// Where this region's sections start in the sight-line bitset.
    cave_base: u32,
    wireframe: u32,
    /// How far this stream's geometry may reach outside its own section.
    overhang: f32,
    /// The lowest layer number that names an animation rather than a layer of an array.
    animated_from: u32,
    /// Where the biome colour map starts in world blocks and how far it reaches. The map covers
    /// the loaded window, which does not start at the origin, so a world position has to be
    /// shifted and scaled rather than divided by a constant.
    tint_origin_x: i32,
    tint_origin_z: i32,
    tint_span_x: f32,
    tint_span_z: f32,
    reserved1: u32,
}

/// The state of the sky the terrain is lit by, as `terrain.wgsl` reads it. One uniform for the
/// whole frame rather than a light texture: there are three terms, and evaluating them in the
/// shader costs neither an upload nor a sample.
#[derive(Resource, Clone, Copy, ExtractResource, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Sky {
    /// `rgb` the colour sky light arrives in, `a` how much of it the time of day lets through.
    pub sky_light: [f32; 4],
    /// `rgb` the tint of a torch at its dimmest, `a` the factor block light is scaled by.
    pub block_light: [f32; 4],
    /// The floor under both, which is what keeps a sealed cave from being pure black.
    pub ambient: [f32; 4],
    /// `rgb` the colour of the sky disc, `a` set when the camera has dropped below the horizon and
    /// the dark disc under it has to be drawn.
    pub disc: [f32; 4],
    /// The twilight band: `rgb` its colour, `a` how far it has come in.
    pub sunrise: [f32; 4],
    /// Sun, moon and star angles in radians, and how bright the stars are.
    pub angles: [f32; 4],
    /// Which layer of the celestial array the moon shows, and how much rain dims the two discs.
    pub moon: [f32; 4],
    /// `rgb` the haze the world fades into, which is also what the frame is cleared to, and `a`
    /// how far from the camera the sky has faded entirely into it.
    pub fog: [f32; 4],
    /// The colour the cloud layer is lit and tinted by, `a` included.
    pub cloud_color: [f32; 4],
    /// Where the underside of the cloud layer sits, how far the field has drifted in x and z, and
    /// the distance the clouds have faded out by.
    pub cloud: [f32; 4],
}

/// Whether the cloud layer is drawn. What the toggle skips is the draw itself rather than the
/// clouds inside it, so with them off nothing walks the screen at all.
#[derive(Resource, Clone, Copy, ExtractResource)]
pub struct Clouds(pub bool);

impl Default for Clouds {
    fn default() -> Self {
        Self(true)
    }
}

/// Draws only the edges of every triangle, in the colour that face's own texture has there, and
/// leaves the interiors unpainted so the geometry behind stays visible.
#[derive(Resource, Clone, Copy, Default, ExtractResource)]
pub struct Wireframe(pub bool);

/// Triangles the culling pass let through, read back from the indirect draw arguments. Shared with
/// the render world through an `Arc` rather than extracted, because the number only exists after
/// the GPU has run, which is the wrong side of the extract boundary.
#[derive(Resource, Clone, Default)]
pub struct DrawnTriangles(Arc<ArgsReadback>);

impl DrawnTriangles {
    pub fn get(&self) -> u32 {
        self.0.triangles.load(Ordering::Relaxed)
    }
}

/// A copy is recorded only once the previous result has landed, so the staging buffer is never both
/// mapped and the destination of a copy, which the backend rejects.
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
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "cull.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "terrain.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "sky.wgsl");

        let triangles = DrawnTriangles::default();
        app.init_resource::<Wireframe>()
            .init_resource::<Sky>()
            .init_resource::<Clouds>()
            .add_plugins(ExtractResourcePlugin::<Wireframe>::default())
            .add_plugins(ExtractResourcePlugin::<Sky>::default())
            .add_plugins(ExtractResourcePlugin::<Clouds>::default())
            .insert_resource(triangles.clone());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(triangles)
            .insert_resource(WorldLayout(self.0.clone()))
            .insert_resource(self.1.clone())
            .add_systems(RenderStartup, init_terrain)
            .add_systems(ExtractSchedule, extract_cave_visibility)
            .add_systems(
                Render,
                (
                    prepare_pipelines.in_set(RenderSystems::Prepare),
                    // Ahead of the two systems that write into the params table, and well ahead of
                    // the culling pass that reads what it publishes.
                    apply_uploads.in_set(RenderSystems::Prepare).before(prepare_wireframe),
                    prepare_wireframe.in_set(RenderSystems::Prepare),
                    prepare_sky.in_set(RenderSystems::Prepare),
                    prepare_view_bind_group.in_set(RenderSystems::PrepareBindGroups),
                    read_draw_args.in_set(RenderSystems::Cleanup),
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
    layout: Arc<Layout>,
    quads: Buffer,
    vertices: Buffer,
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
    draw_layout: BindGroupLayoutDescriptor,
    sky_layout: BindGroupLayoutDescriptor,
    sky_bind_group: BindGroup,
    terrain_shader: Handle<Shader>,
    sky_shader: Handle<Shader>,
    /// `[simple opaque, complex opaque, simple blended, complex blended]`, queued once the first
    /// view reveals the colour format the pipelines have to match.
    pipelines: Option<[CachedRenderPipelineId; 4]>,
    /// The sky's own four, in the order vanilla draws them.
    sky_pipelines: Option<[CachedRenderPipelineId; SKY_DRAWS.len()]>,
    /// One entry per `draw_indirect`, in draw order. Grows as regions land.
    draws: Vec<Draw>,
    /// Workgroups to dispatch per draw, one per culling group.
    group_counts: Vec<u32>,
    /// The params table as the CPU holds it, so a change to one field does not need the rest read
    /// back off the GPU. Rewritten whole whenever anything in it moves; it is seventy kilobytes.
    params_cpu: Vec<Params>,
    params_dirty: bool,
    /// The lowest layer number that names an animation. It moves as the sprite set grows, and
    /// every params entry carries it.
    animated_from: u32,
    /// Whether the table's entries ask for edges only. Carried here so a region published while
    /// the toggle is on does not come out solid among wireframe neighbours.
    wireframe: u32,
    /// Whatever of the current upload has not been handed over yet.
    pending: Option<Pending>,
}

/// One upload part way through being written.
struct Pending {
    placement: Placement,
    /// Which of the three arenas is being written.
    part: usize,
    /// Bytes of that arena's share already sent.
    done: usize,
}

#[derive(Copy, Clone)]
enum Arena {
    Quads,
    Vertices,
    Groups,
}

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
            // A zero-length storage buffer is not a legal binding, and a window with no files in
            // it would ask for one.
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
    let group_buffer = arena(
        "terrain groups",
        (layout.group_capacity * size_of::<Group>()) as u64,
    );
    // Worst case every quad in the arenas survives culling, so the visible list is sized for both
    // of them and each draw owns a slice of it. No allocation can ever be needed at draw time.
    let visible = arena(
        "terrain visible list",
        ((layout.quad_capacity + layout.model_capacity) * 4) as u64,
    );
    // A pack with no animation still binds one entry, which no quad names because the layer field
    // never reaches it.
    let animations = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain animations"),
        contents: bytemuck::cast_slice(&[Animation::default()]),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });
    let cave = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("terrain cave visibility"),
        // All ones until the first flood fill, so nothing goes missing on the earliest frames.
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
            vertex_count: 6,
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
    // Nearest, and one mip only: a thirty-two pixel sun blown across the sky has to keep its edges
    // rather than smear into a blur, and there is no distance here for a mip chain to serve.
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
    // One binding per addressable sprite array. Binding arrays would collapse these into one entry
    // but need a feature Metal does not offer, so the count is spelled out and pinned to the width
    // of the packed field.
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

    let cull_shader = asset_server.load("embedded://anvil_region_viewer/cull.wgsl");
    let terrain_shader: Handle<Shader> =
        asset_server.load("embedded://anvil_region_viewer/terrain.wgsl");
    let sky_shader: Handle<Shader> = asset_server.load("embedded://anvil_region_viewer/sky.wgsl");

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
        draw_layout,
        sky_layout,
        sky_bind_group,
        terrain_shader,
        sky_shader,
        pipelines: None,
        sky_pipelines: None,
    });
}

/// The draw bind group names every sprite array, so it has to be rebuilt whenever the set of them
/// changes. Building it is cheap; the textures behind it are what cost.
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
        )),
    )
}

/// The shader has one binding per addressable array, so a pack that uses fewer resolutions still
/// has to leave something bound. An unused slot gets a single blank layer, which no quad names.
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

/// Nearest within a level keeps the pixel art crisp; linear between levels stops distant terrain
/// from boiling. `Repeat` is what lets a greedy quad tile its sprite `w × h` times for free. Every
/// array shares it: the filtering does not depend on how large the sprites in it are.
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
    (texture, sampler)
}

/// Drains what the loader has produced, within a fixed number of bytes a frame.
///
/// A region's draws go into the table only after every byte of its geometry has landed, so the
/// culling pass never sees a group pointing into an arena that has not been written yet.
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

    loop {
        if terrain.pending.is_none() {
            let next = uploads.0.lock().unwrap().pop_front();
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
                    );
                    // The threshold that tells a still sprite from an animation moves as the set
                    // grows, and every entry of the table carries it.
                    terrain.animated_from = animated_from;
                    rebuild_params(terrain);
                    // Rebuilding every array and its mips is megabytes of texture, so it spends the
                    // frame's budget like geometry does rather than landing on top of it.
                    budget = budget.saturating_sub(spent);
                    if budget == 0 {
                        break;
                    }
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
        while budget > 0 && pending.part < 3 {
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
        if pending.part < 3 {
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

/// Adds a region's draws and rebuilds the table around them.
///
/// The table has to stay in stream order, because stream order is draw order: the translucent
/// streams write no depth, so opaque geometry from a region published later would draw straight
/// over water published earlier and leave the sea missing in region-sized rectangles. A stable
/// sort keeps regions in the order they landed within each stream.
fn publish(terrain: &mut Terrain, draws: Vec<Draw>) {
    terrain.draws.extend(draws);
    terrain.draws.sort_by_key(|draw| draw.stream);
    rebuild_params(terrain);
}

/// Rewrites every entry of the table from the draw list. At a few hundred entries this costs less
/// than tracking which of them a change touched.
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
            // Odd streams carry baked model quads, the only geometry that leaves its own section.
            // Growing the greedy streams' boxes too would only weaken a test that is exact today.
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
            reserved1: 0,
        });
        visible_base += draw.quad_count;
        terrain.group_counts.push(draw.group_count);
    }
    terrain.params_dirty = true;
}

/// The table is strided for dynamic offsets, so it is assembled here rather than written entry by
/// entry: at seventy kilobytes for a full window one write costs less than the bookkeeping would.
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

/// Maps the copy the culling pass left behind. The number is a frame late, which cannot show in a
/// readout that is only redrawn once a second, and the map never blocks: the callback lands
/// whenever the device is next polled and the state only returns to idle then.
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
            // Every quad is two triangles, whichever stream drew it.
            let count = args.iter().map(|arg| arg.instance_count).sum::<u32>() * 2;
            readback.triangles.store(count, Ordering::Relaxed);
            drop(view);
            buffer.unmap();
        }
        readback.state.store(ArgsReadback::IDLE, Ordering::Relaxed);
    });
}

/// The flag is pushed into the table only on the frame the key is pressed rather than re-uploaded
/// every frame.
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

/// The flood fill runs in `PostUpdate` against this frame's frustum, so the bitset the compute pass
/// reads describes exactly the view it is culling for. Written straight from the main world rather
/// than through an `ExtractResourcePlugin`, which would clone these 4 KiB every frame.
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
                // The grass block, and every block like it, ships a tinted overlay as a second
                // element exactly coplanar with the cube face under it. The face below goes down
                // the greedy path and is merged across neighbouring blocks, while the overlay stays
                // one quad per block, so the two rasterise the same plane from different triangles
                // and their interpolated depths disagree by a rounding step. Pulling model quads a
                // hair toward the camera settles every such tie in the overlay's favour, which is
                // the side vanilla shows.
                bias: if entry == "vertex_complex" {
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
                // A disc drawn round the camera and a star turned to face it are seen from
                // whichever side they end up on, so neither may be culled by its winding.
                cull_mode: None,
                ..default()
            },
            // The backdrop reads nothing from the depth buffer and leaves nothing in it, so the
            // terrain drawn afterwards covers whatever it stands in front of. The clouds are in
            // the world rather than behind it: they test what the terrain left, against the depth
            // their own shader works out for wherever the ray met a cell.
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(false),
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
    mut ctx: RenderContext,
) {
    let (Some(terrain), Some(view_bind_group)) = (terrain, view_bind_group) else {
        return;
    };
    let Some(pipeline) = pipeline_cache.get_compute_pipeline(terrain.cull_pipeline) else {
        return;
    };
    let view_offset = view.into_inner().offset;

    // The counts still describe the frame that is being replaced, so the copy is recorded before
    // the clear rather than after the dispatch, where it would have to wait on the pass.
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

    // Reset only the instance counters; the rest of each draw command is constant for the run of
    // the program, so there is nothing else to upload per frame.
    for index in 0..terrain.draws.len() {
        ctx.command_encoder().clear_buffer(
            &terrain.args,
            index as u64 * DRAW_ARGS_SIZE + INSTANCE_COUNT_OFFSET,
            NonZeroU64::new(4).map(|n| n.get()),
        );
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
    for (index, &workgroups) in terrain.group_counts.iter().enumerate() {
        if workgroups == 0 {
            continue;
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

fn draw_terrain(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &ViewUniformOffset)>,
    terrain: Option<Res<Terrain>>,
    view_bind_group: Option<Res<ViewBindGroup>>,
    clouds: Res<Clouds>,
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

    // The backdrop first, in vanilla's order, so the terrain covers whatever stands in front of
    // it. The params slot is bound to the first draw's entry and never read: the sky takes nothing
    // from it.
    draw_sky(&mut pass, &terrain, &view_bind_group.0, view_offset.offset, &pipeline_cache, false);

    pass.set_bind_group(1, &terrain.draw_bind_group, &[]);

    for (index, draw) in terrain.draws.iter().enumerate() {
        if draw.quad_count == 0 {
            continue;
        }
        // Solid and cutout share a pipeline (the alpha test never fires on a solid sprite); only
        // the translucent pass differs, and it must come last because it does not write depth.
        let stream = draw.stream as usize;
        let pipeline_index = (stream / 4) * 2 + stream % 2;
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

    // The clouds hang in the world, so they come after what can stand in front of them, and they
    // are the only half of the sky that can be turned off.
    if clouds.0 {
        draw_sky(&mut pass, &terrain, &view_bind_group.0, view_offset.offset, &pipeline_cache, true);
    }
    span.end(&mut pass);
}
