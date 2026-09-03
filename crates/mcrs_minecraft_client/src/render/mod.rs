mod arenas;
mod binds;
mod draws;
mod frame;
mod layer;
mod pass;
mod pipeline;
mod shaders;
mod sprites;
mod stats;
mod terrain;
mod texture;
mod upload;

use std::sync::Arc;

use bevy::core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, main_opaque_pass_3d};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_resource::{CompareFunction, TextureFormat};
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};

use crate::mesh::STREAMS;
use crate::probe::{self, CpuTimings, GpuTimings};

pub use stats::{DrawnTriangles, FrameCounts};
pub use upload::{Placement, Upload, Uploads};

pub(crate) use frame::uniform as uniform_buffer;
pub(crate) use pipeline::common as pipeline_descriptor;

/// The depth buffer runs reverse-Z, so the near plane is at one and a fragment passes when its
/// depth is the greater. Every pipeline drawing into the view depth must agree on this.
pub const DEPTH_COMPARE: CompareFunction = CompareFunction::GreaterEqual;
const _: () = assert!(matches!(CORE_3D_DEPTH_FORMAT, TextureFormat::Depth32Float));

pub const QUAD_BYTES: usize = crate::pack::QUAD_WORDS * 4;
pub const MODEL_BYTES: usize = 4 * 3 * 4;
pub const FACE_BYTES: usize = 4;
pub const SECTION_BYTES: usize = size_of::<SectionDesc>();
const VISIBLE_BYTES: usize = 8;

pub struct Budget {
    pub quads: usize,
    pub models: usize,
    pub faces: usize,
    pub groups: usize,
    pub sections: usize,
    pub tint_origin: [i32; 2],
    pub tint_size: [u32; 2],
}

/// One row of the section table: where a section sits in the world, how many blocks one of its
/// samples covers, and where its face attributes begin.
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct SectionDesc {
    pub section: [i32; 3],
    pub scale: u32,
    pub face_base: u32,
}

/// The layers one array gained since the last update: stills from `first_still` up to `stills`,
/// frame layers from `first_frame` up to `frames`, each as a full mip chain with level zero
/// first. What was sent before stays where it is on the GPU.
pub struct AtlasUpdate {
    pub size: u32,
    pub stills: u32,
    pub frames: u32,
    pub first_still: u32,
    pub first_frame: u32,
    pub still_mips: Vec<Vec<u8>>,
    pub frame_mips: Vec<Vec<u8>>,
}

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Animation {
    pub array: u32,
    pub frame_base: u32,
    pub count: u32,
    pub frametime: u32,
    pub interpolate: u32,
}

#[derive(Resource, Clone, Copy, ExtractResource)]
pub struct Streams(pub u32);

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
pub struct Raster(pub f32);

impl Default for Raster {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Resource, Clone, Copy, Default, ExtractResource)]
pub struct Wireframe(pub bool);

pub fn toggle_wireframe(keys: Res<ButtonInput<KeyCode>>, mut wireframe: ResMut<Wireframe>) {
    if keys.just_pressed(KeyCode::F10) {
        wireframe.0 = !wireframe.0;
    }
}

#[derive(Resource, Deref)]
struct TerrainBudget(Arc<Budget>);

fn embed_shaders(app: &mut App) {
    use bevy::shader::load_shader_library;
    load_shader_library!(app, "shaders/include/fields.wgsl");
    load_shader_library!(app, "shaders/include/section.wgsl");
    load_shader_library!(app, "shaders/include/frame.wgsl");
    load_shader_library!(app, "shaders/include/sky.wgsl");
    load_shader_library!(app, "shaders/include/quad.wgsl");
    load_shader_library!(app, "shaders/include/lighting.wgsl");
    load_shader_library!(app, "shaders/include/terrain_bindings.wgsl");
    load_shader_library!(app, "shaders/include/surface.wgsl");
    load_shader_library!(app, "shaders/include/finish.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/core/greedy.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/core/model.wgsl");
    bevy::asset::embedded_asset!(app, "shaders/core/cull.wgsl");
}

pub struct TerrainPlugin(pub Arc<Budget>, pub Uploads);

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        embed_shaders(app);

        let triangles = DrawnTriangles::default();
        let timings = GpuTimings::default();
        let cpu = CpuTimings::default();
        let counts = FrameCounts::default();
        app.insert_resource(crate::config::wireframe())
            .init_resource::<Streams>()
            .add_plugins(ExtractResourcePlugin::<Wireframe>::default())
            .add_plugins(ExtractResourcePlugin::<Streams>::default())
            .add_plugins(ExtractResourcePlugin::<Raster>::default())
            .insert_resource(triangles.clone())
            .insert_resource(timings.clone())
            .insert_resource(cpu.clone())
            .insert_resource(counts.clone())
            .add_systems(First, probe::frame_started.after(bevy::time::TimeSystems))
            .add_systems(Last, probe::main_ended)
            .add_systems(PostStartup, probe::log_system_counts);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(triangles)
            .insert_resource(timings)
            .insert_resource(cpu)
            .insert_resource(counts)
            .insert_resource(TerrainBudget(self.0.clone()))
            .insert_resource(self.1.clone())
            .add_systems(
                RenderStartup,
                (terrain::init_terrain, probe::init, probe::log_system_counts),
            )
            .add_systems(ExtractSchedule, pass::extract_cave_visibility)
            .add_systems(
                Render,
                (
                    probe::extracted.before(RenderSystems::ExtractCommands),
                    probe::acquiring.before(bevy::render::view::window::prepare_windows),
                    probe::acquired.after(bevy::render::view::window::prepare_windows),
                    probe::prepared
                        .after(RenderSystems::Prepare)
                        .before(RenderSystems::Render),
                    probe::rendered
                        .after(RenderSystems::Render)
                        .before(RenderSystems::Cleanup),
                    probe::cleaned.in_set(RenderSystems::PostCleanup),
                    pipeline::prepare_pipelines.in_set(RenderSystems::Prepare),
                    pass::drop_unused_bins.in_set(RenderSystems::Prepare),
                    terrain::write_lightmap.in_set(RenderSystems::Prepare),
                    sprites::write_animation_frames.in_set(RenderSystems::Prepare),
                    frame::write_camera.in_set(RenderSystems::Prepare),
                    stats::read_draw_args.in_set(RenderSystems::Cleanup),
                    probe::read.in_set(RenderSystems::Cleanup),
                    upload::recall_staging.in_set(RenderSystems::Cleanup),
                ),
            )
            .add_systems(
                Core3d,
                pass::draw_frame
                    .in_set(Core3dSystems::MainPass)
                    .after(main_opaque_pass_3d),
            );
    }
}
