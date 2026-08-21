mod params;
mod pass;
mod pipeline;
mod sky;
mod stats;
mod terrain;
mod texture;
mod upload;

use std::sync::Arc;

use bevy::core_pipeline::core_3d::main_opaque_pass_3d;
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};

use crate::mesh::STREAMS;
use crate::pack::RegionGrid;
use crate::probe::{self, GpuTimings};

pub use stats::DrawnTriangles;
pub use upload::{Placement, Upload, Uploads};

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

pub struct Atlas {
    pub size: u32,
    pub layers: u32,
    pub mips: Vec<Vec<u8>>,
}

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Animation {
    pub base_layer: u32,
    pub count: u32,
    pub frametime: u32,
    pub interpolate: u32,
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

const BLENDED_STREAM: u32 = STREAMS as u32 - 2;

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

pub fn toggle_wireframe(keys: Res<ButtonInput<KeyCode>>, mut wireframe: ResMut<Wireframe>) {
    if keys.just_pressed(KeyCode::F10) {
        wireframe.0 = !wireframe.0;
    }
}

#[derive(Resource, Deref)]
struct WorldLayout(Arc<Layout>);

pub struct TerrainPlugin(pub Arc<Layout>, pub Uploads);

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "shaders/layout.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "shaders/cull.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "shaders/terrain.wgsl");
        bevy::asset::embedded_asset!(app, "examples/anvil_region_viewer/", "shaders/sky.wgsl");

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
            .add_systems(RenderStartup, (terrain::init_terrain, probe::init))
            .add_systems(ExtractSchedule, pass::extract_cave_visibility)
            .add_systems(
                Render,
                (
                    pipeline::prepare_pipelines.in_set(RenderSystems::Prepare),
                    pass::drop_unused_bins.in_set(RenderSystems::Prepare),
                    upload::apply_uploads
                        .in_set(RenderSystems::Prepare)
                        .before(params::prepare_wireframe),
                    params::prepare_wireframe.in_set(RenderSystems::Prepare),
                    sky::prepare.in_set(RenderSystems::Prepare),
                    pass::prepare_view_bind_group.in_set(RenderSystems::PrepareBindGroups),
                    stats::read_draw_args.in_set(RenderSystems::Cleanup),
                    probe::read.in_set(RenderSystems::Cleanup),
                ),
            )
            .add_systems(
                Core3d,
                (probe::resolve, pass::cull_terrain)
                    .chain()
                    .in_set(Core3dSystems::Prepass),
            )
            .add_systems(
                Core3d,
                pass::draw_terrain
                    .in_set(Core3dSystems::MainPass)
                    .after(main_opaque_pass_3d),
            );
    }
}
