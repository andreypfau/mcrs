#![recursion_limit = "256"]

mod arenas;
mod binds;
mod cull;
mod draws;
mod frame;
mod frame_view;
mod gbuffer;
mod hiz;
mod improved_transparency;
mod layer;
pub mod name_tag;
mod panorama;
mod pass;
mod pipeline;
mod present;
mod reconstruct;
mod render_path;
#[cfg(test)]
mod shader_layout;
#[cfg(test)]
mod shader_layout_emit;
mod shaders;
#[doc(hidden)]
pub mod shared_bindings;
mod shared_types;
mod shared_wgsl;
mod show;
pub mod sky;
mod sprites;
mod stats;
mod terrain;
mod texture;
mod timestamps;
mod transparency_targets;
mod upload;
mod views;
mod words;
mod world_fog;
pub mod world_text;

use std::sync::Arc;

use bevy::core_pipeline::core_3d::{
    CORE_3D_DEPTH_FORMAT, main_opaque_pass_3d, main_transparent_pass_3d,
};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::ecs::schedule::{InternedSystemSet, ScheduleCleanupPolicy, ScheduleConfigs};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_resource::{
    BindGroup, BindGroupLayoutDescriptor, CompareFunction, TextureFormat,
};
use bevy::render::renderer::{RenderAdapter, RenderDevice};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

use render_path::full_path;

use mcrs_minecraft_mesh::DrawKind;

pub use frame::{CameraOrigin, TerrainShade, clip_from_relative};
pub use frame_view::{FrameView, SIDE_PLANES};
pub use gbuffer::{DeferredFrame, GBUFFER_FORMATS};
pub use improved_transparency::ImprovedTransparency;
pub use layer::LayerGroup;
pub use panorama::{
    CUBE_FACES, PanoramaDraw, PanoramaImages, PanoramaPass, PanoramaPlugin, PanoramaView,
};
pub use pass::draw_layer_group;
pub use pipeline::{TERRAIN_PIPELINES, terrain_slot};
pub use render_path::RenderPath;
pub use shared_types::{
    CameraUniform, DrawArgRow, DrawArgs, Params, SectionDesc, SkyUniform, SpriteEntry,
};
pub use shared_wgsl::{GBUFFER_ENCODE_WGSL, VISIBILITY_WGSL};
pub use show::{DepthDisplay, DepthSource};
pub use stats::{FrameCounts, VERTICES_PER_QUAD, args_reset};
pub use terrain::Terrain;
pub use timestamps::{PassSlot, PassTimestamps};
pub use upload::{Placement, Upload, Uploads};
pub use views::{DebugView, DebugViews, SelectedView};
pub use words::Words;
pub use world_fog::WorldFog;

pub use frame::uniform as uniform_buffer;
pub use pipeline::common as pipeline_descriptor;

pub const DEPTH_COMPARE: CompareFunction = CompareFunction::GreaterEqual;
const _: () = assert!(matches!(CORE_3D_DEPTH_FORMAT, TextureFormat::Depth32Float));

pub const QUAD_BYTES: usize = mcrs_minecraft_mesh::pack::QUAD_WORDS * 4;
pub const MODEL_BYTES: usize = 4 * 3 * 4;
pub const FACE_BYTES: usize = mcrs_minecraft_mesh::pack::FACE_WORDS * 4;
pub(crate) const VISIBLE_BYTES: usize = 8;

pub struct Budget {
    pub quads: usize,
    pub models: usize,
    pub faces: usize,
    pub groups: usize,
    pub sections: usize,
    pub upload: usize,
    pub tint_size: [u32; 2],
}

pub struct AtlasUpdate {
    pub size: u32,
    pub layers: u32,
    pub first: u32,
    pub mips: Vec<Vec<u8>>,
}

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Animation {
    pub first_layer: u32,
    pub count: u32,
    pub frametime: u32,
    pub interpolate: u32,
}

pub const STILL: u32 = u32::MAX;

pub struct SpriteUpload {
    pub atlases: Vec<AtlasUpdate>,
    pub table_from: u32,
    pub table: Vec<SpriteEntry>,
    pub animations: Vec<Animation>,
}

#[derive(Resource, Clone, Copy, ExtractResource)]
#[extract_app(RenderApp)]
pub struct DrawMask(pub u32);

impl DrawMask {
    pub(crate) const ALL: u32 = (1 << DrawKind::COUNT) - 1;

    fn drawn(&self, kind: DrawKind) -> bool {
        self.0 & (1 << kind.index()) != 0
    }
}

impl Default for DrawMask {
    fn default() -> Self {
        Self(Self::ALL)
    }
}

#[derive(Resource, Clone, Copy, ExtractResource)]
#[extract_app(RenderApp)]
pub struct Occlusion(pub bool);

impl Default for Occlusion {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Clone, Copy, ExtractResource)]
#[extract_app(RenderApp)]
pub struct QuadCull(pub bool);

impl Default for QuadCull {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, ExtractResource)]
#[extract_app(RenderApp)]
pub struct RenderDistance(pub u8);

impl Default for RenderDistance {
    fn default() -> Self {
        Self(u8::MAX)
    }
}

#[derive(Resource, Clone, Copy, ExtractResource)]
#[extract_app(RenderApp)]
pub struct Brightness(pub f32);

impl Default for Brightness {
    fn default() -> Self {
        Self(0.5)
    }
}

#[derive(Resource, Clone, Copy, ExtractResource)]
#[extract_app(RenderApp)]
pub struct PinnedTick(pub i64);

#[derive(Resource)]
pub struct LightTint {
    pub layout: BindGroupLayoutDescriptor,
    pub bind_group: Option<BindGroup>,
}

#[derive(Resource, Deref)]
struct TerrainBudget(Arc<Budget>);

fn embed_shaders(app: &mut App) {
    use bevy::shader::load_shader_library;
    load_shader_library!(app, "shaders/include/fields.wesl");
    load_shader_library!(app, "shaders/include/words.wesl");
    load_shader_library!(app, "shaders/include/section_layout.wesl");
    load_shader_library!(app, "shaders/include/section.wesl");
    load_shader_library!(app, "shaders/include/frame.wesl");
    load_shader_library!(app, "shaders/include/fog.wesl");
    load_shader_library!(app, "shaders/include/sky.wesl");
    load_shader_library!(app, "shaders/include/sky_layout.wesl");
    load_shader_library!(app, "shaders/include/quad.wesl");
    load_shader_library!(app, "shaders/include/quad_coverage.wesl");
    load_shader_library!(app, "shaders/include/lightmap.wesl");
    load_shader_library!(app, "shaders/include/sprites_layout.wesl");
    load_shader_library!(app, "shaders/include/terrain_layout.wesl");
    load_shader_library!(app, "shaders/include/terrain_bindings.wesl");
    load_shader_library!(app, "shaders/include/lighting.wesl");
    load_shader_library!(app, "shaders/include/surface.wesl");
    load_shader_library!(app, "shaders/include/finish.wesl");
    load_shader_library!(app, "shaders/include/gbuffer_encode.wesl");
    load_shader_library!(app, "shaders/include/depth.wesl");
    load_shader_library!(app, "shaders/include/visibility.wesl");
    load_shader_library!(app, "shaders/include/cull_layout.wesl");
    load_shader_library!(app, "shaders/include/deferred.wesl");
    load_shader_library!(app, "shaders/include/reconstruct.wesl");
    load_shader_library!(app, "shaders/include/gbuffer_layout.wesl");
    load_shader_library!(app, "shaders/include/gbuffer_views_layout.wesl");
    load_shader_library!(app, "shaders/include/transparency_layout.wesl");
    load_shader_library!(app, "shaders/include/transparency.wesl");
    load_shader_library!(app, "shaders/include/hiz_layout.wesl");
    load_shader_library!(app, "shaders/include/present_layout.wesl");
    load_shader_library!(app, "shaders/include/show_layout.wesl");
    load_shader_library!(app, "shaders/include/world_text_layout.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/greedy.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/model.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/cull.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/hiz.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/show.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/lighting_pass.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/gbuffer_views.wesl");
    bevy::asset::embedded_asset!(app, "shaders/core/present.wesl");
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorldPass {
    Upload,
    Cull,
    Opaque,
    Occlusion,
    OpaqueSecond,
    Entities,
    Lighting,
    Overlay,
    Forward,
}

impl WorldPass {
    pub fn order() -> ScheduleConfigs<InternedSystemSet> {
        (
            Self::Upload,
            Self::Cull,
            Self::Opaque,
            Self::Occlusion,
            Self::OpaqueSecond,
            Self::Entities,
            Self::Lighting,
            Self::Overlay,
            Self::Forward,
        )
            .chain()
            .in_set(Core3dSystems::MainPass)
            .before(main_transparent_pass_3d)
    }
}

pub struct TerrainPlugin {
    pub budget: Arc<Budget>,
    pub uploads: Uploads,
}

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        embed_shaders(app);
        app.add_plugins(world_text::WorldTextPlugin);
        app.add_plugins(name_tag::NameTagRenderPlugin);

        app.init_resource::<CameraOrigin>()
            .init_resource::<Occlusion>()
            .init_resource::<QuadCull>()
            .init_resource::<RenderDistance>()
            .init_resource::<WorldFog>()
            .init_resource::<ImprovedTransparency>()
            .init_resource::<Brightness>()
            .init_resource::<DrawMask>()
            .init_resource::<DebugViews>()
            .init_resource::<SelectedView>()
            .init_resource::<TerrainShade>()
            .add_plugins(ExtractResourcePlugin::<TerrainShade>::default())
            .add_plugins(ExtractResourcePlugin::<SelectedView>::default())
            .add_plugins(ExtractResourcePlugin::<Occlusion>::default())
            .add_plugins(ExtractResourcePlugin::<QuadCull>::default())
            .add_plugins(ExtractResourcePlugin::<RenderDistance>::default())
            .add_plugins(ExtractResourcePlugin::<WorldFog>::default())
            .add_plugins(ExtractResourcePlugin::<ImprovedTransparency>::default())
            .add_plugins(ExtractResourcePlugin::<Brightness>::default())
            .add_plugins(ExtractResourcePlugin::<DrawMask>::default())
            .add_plugins(ExtractResourcePlugin::<PinnedTick>::default())
            .add_plugins(ExtractResourcePlugin::<CameraOrigin>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(TerrainBudget(self.budget.clone()))
            .insert_resource(self.uploads.clone())
            .init_resource::<FrameCounts>()
            .init_resource::<PassTimestamps>()
            .init_resource::<pipeline::DeferredPipelines>()
            .add_systems(
                RenderStartup,
                (terrain::init_terrain, show::init_depth_display),
            )
            .add_systems(
                Render,
                (
                    pass::drop_unused_bins
                        .run_if(pass::has_unused_bins)
                        .in_set(RenderSystems::Prepare),
                    terrain::write_lightmap.in_set(RenderSystems::Prepare),
                    sprites::write_animation_frames.in_set(RenderSystems::Prepare),
                    (frame::write_frame_view, frame::write_camera)
                        .chain()
                        .in_set(RenderSystems::Prepare),
                    pipeline::prepare_deferred_pipelines.in_set(RenderSystems::Prepare),
                    (
                        gbuffer::fit_deferred_frame,
                        reconstruct::write_lighting_uniform,
                    )
                        .chain()
                        .in_set(RenderSystems::PrepareBindGroups),
                    upload::recall_staging.in_set(RenderSystems::Cleanup),
                ),
            )
            .configure_sets(Core3d, WorldPass::order())
            .add_systems(
                Core3d,
                (
                    pass::upload_frame.in_set(WorldPass::Upload),
                    pass::cull_frame
                        .in_set(WorldPass::Cull)
                        .run_if(pipeline::frame_ready)
                        .run_if(full_path),
                    pass::draw_gbuffer
                        .in_set(WorldPass::Opaque)
                        .run_if(pipeline::frame_ready),
                    pass::build_occlusion
                        .in_set(WorldPass::Occlusion)
                        .run_if(pipeline::frame_ready)
                        .run_if(full_path),
                    pass::draw_gbuffer_second
                        .in_set(WorldPass::OpaqueSecond)
                        .run_if(pipeline::frame_ready)
                        .run_if(full_path),
                    pass::draw_lighting
                        .in_set(WorldPass::Lighting)
                        .run_if(pipeline::frame_ready),
                    pass::draw_forward_deferred
                        .in_set(WorldPass::Forward)
                        .run_if(pipeline::frame_ready)
                        .run_if(pass::draws_forward),
                    present::present
                        .in_set(WorldPass::Forward)
                        .after(pass::draw_forward_deferred)
                        .run_if(pipeline::frame_ready),
                ),
            );
        if let Err(error) = render_app.remove_systems_in_set(
            Core3d,
            main_opaque_pass_3d,
            ScheduleCleanupPolicy::RemoveSystemsOnly,
        ) {
            error!("bevy's opaque pass stays in the 3d core pipeline: {error}");
        }
    }

    fn finish(&self, app: &mut App) {
        let world = app.world();
        let path = match (
            world.get_resource::<RenderDevice>(),
            world.get_resource::<RenderAdapter>(),
        ) {
            (Some(device), Some(adapter)) => RenderPath::of(device, adapter),
            _ => {
                warn!(
                    "the terrain plugin finished before the renderer: drawing on the minimal path"
                );
                RenderPath::Minimal
            }
        };
        let occlusion = world.resource::<Occlusion>().0 && path.is_full();
        app.insert_resource(path);
        let mut views = app.world_mut().resource_mut::<DebugViews>();
        let deferred = views::DeferredViews::register(&mut views, occlusion);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(path).insert_resource(deferred);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::core_pipeline::schedule::Core3dSystems;

    #[derive(Resource, Default)]
    struct Ran(Vec<u8>);

    fn record(step: u8) -> impl FnMut(ResMut<Ran>) {
        move |mut ran: ResMut<Ran>| ran.0.push(step)
    }

    #[test]
    fn a_system_in_a_world_pass_runs_in_its_stage() {
        let mut schedule = Core3d::base_schedule();
        schedule.configure_sets(WorldPass::order());
        schedule.add_systems(record(0).in_set(Core3dSystems::Prepass));
        schedule.add_systems((
            record(6)
                .after(WorldPass::Forward)
                .in_set(Core3dSystems::MainPass),
            record(5).in_set(WorldPass::Forward),
            record(4).in_set(WorldPass::Lighting),
            record(3).in_set(WorldPass::OpaqueSecond),
            record(2).in_set(WorldPass::Opaque),
            record(1).in_set(WorldPass::Upload),
        ));
        schedule.add_systems(record(7).in_set(Core3dSystems::EarlyPostProcess));

        let mut world = World::new();
        world.init_resource::<Ran>();
        schedule.run(&mut world);

        assert_eq!(world.resource::<Ran>().0, [0, 1, 2, 3, 4, 5, 6, 7]);
    }
}
