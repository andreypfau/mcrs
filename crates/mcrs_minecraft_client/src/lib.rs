// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

use std::path::{Path, PathBuf};

use bevy::app::PluginGroupBuilder;
use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, RenderApp};

pub mod anim;
#[cfg(target_os = "macos")]
pub mod app_nap;
pub mod atlas;
pub mod bake;
pub mod blocks;
pub mod camera;
#[cfg(target_os = "macos")]
pub mod capture;
pub mod cave;
pub mod chunk_guard;
pub mod columns;
pub mod config;
pub mod game_mode;
pub mod gui;
pub mod input;
pub mod inventory;
pub mod item_model;
pub mod light_guard;
pub mod light_volume;
pub mod local_player;
pub mod model;
pub mod player;
pub mod render;
#[cfg(not(target_family = "wasm"))]
pub mod screenshot;
pub mod sky;
pub mod sky_state;
pub mod stream;
pub mod vanilla;
#[cfg(target_family = "wasm")]
pub mod web;

pub fn asset_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .join("assets")
}

pub struct ClientPlugins;

impl PluginGroup for ClientPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(vanilla::VanillaAssetsPlugin)
            .add(mcrs_minecraft_assets::MinecraftCorePlugin)
            .add(mcrs_minecraft_world::MinecraftWorldPlugin)
            .add(mcrs_minecraft_light_color::plugin::LightColorPlugin)
            .add(player::PlayerPlugin)
            .add(input::ClientInputPlugin)
            .add(local_player::LocalPlayerPlugin)
            .add(camera::CameraPlugin)
            .add(gui::debug::DebugScreenPlugin)
            .add(gui::chunk_map::ChunkMapPlugin)
            .add(gui::light_levels::LightLevelsPlugin)
            .add(sky::SkyPlugin)
    }
}

pub struct ClientTerrainPlugin(pub config::TerrainLimits);

impl Plugin for ClientTerrainPlugin {
    fn build(&self, app: &mut App) {
        let (budget, uploads, cave) = config::terrain(self.0);
        app.add_plugins(mcrs_minecraft_render::TerrainPlugin {
            budget: budget.clone(),
            uploads: uploads.clone(),
            heat: config::gpu_hot(),
            timestamps: config::pass_timestamps(),
        })
        .add_plugins(mcrs_minecraft_render_deferred::DeferredPlugin)
        .add_plugins(render::GuiItemsPlugin)
        .add_plugins(render::RasterPlugin)
        .insert_resource(config::occlusion())
        .insert_resource(mcrs_minecraft_render::Brightness(config::brightness()))
        .insert_resource(config::drawn_streams())
        .insert_resource(config::raster_fraction())
        .insert_resource(mcrs_minecraft_render_deferred::VolumeSettings {
            radius: config::color_radius(),
            view_distance: config::view_distance(),
            sections_per_frame: config::color_sections(),
        })
        .add_plugins(stream::StreamPlugin::new(budget, uploads))
        .add_plugins(light_volume::LightVolumePlugin)
        .insert_resource(cave)
        .add_systems(
            Update,
            (
                cave::toggle,
                toggle_culls,
                step_debug_view,
                #[cfg(target_os = "macos")]
                capture::gputrace,
            ),
        )
        .add_systems(
            PostUpdate,
            cave::cave_cull.after(VisibilitySystems::UpdateFrusta),
        );
        if let Some(tick) = config::frozen_time() {
            app.insert_resource(mcrs_minecraft_render::PinnedTick(tick));
        }
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, cave::extract_cave_visibility);
        }
    }
}

fn step_debug_view(
    keys: Res<ButtonInput<KeyCode>>,
    views: Res<mcrs_minecraft_render::DebugViews>,
    mut selected: ResMut<mcrs_minecraft_render::SelectedView>,
) {
    if keys.just_pressed(KeyCode::F10) {
        let back = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        selected.0 = views.step(selected.0, back);
    }
}

/// `O` turns the depth pyramid's occlusion test off and on, and `F8` the per-quad tests, so a
/// hole in the terrain can be traced to the cull stage that makes it.
fn toggle_culls(
    keys: Res<ButtonInput<KeyCode>>,
    mut occlusion: ResMut<mcrs_minecraft_render::Occlusion>,
    mut quad_cull: ResMut<mcrs_minecraft_render::QuadCull>,
) {
    if keys.just_pressed(KeyCode::KeyO) {
        occlusion.0 = !occlusion.0;
        info!(on = occlusion.0, "occlusion culling");
    }
    if keys.just_pressed(KeyCode::F8) {
        quad_cull.0 = !quad_cull.0;
        info!(on = quad_cull.0, "per-quad culling");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_render::{DebugViews, SelectedView};

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    fn release(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    #[test]
    fn f10_steps_forward_and_shift_f10_steps_back() {
        let mut views = DebugViews::default();
        let first = views.register("first");
        let second = views.register("second");
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SelectedView>()
            .insert_resource(views)
            .add_systems(Update, step_debug_view);
        let selected = |app: &App| app.world().resource::<SelectedView>().0;

        press(&mut app, KeyCode::F10);
        assert_eq!(selected(&app), Some(first));
        release(&mut app, KeyCode::F10);
        press(&mut app, KeyCode::F10);
        assert_eq!(selected(&app), Some(second));
        release(&mut app, KeyCode::F10);
        press(&mut app, KeyCode::F10);
        assert_eq!(selected(&app), None);
        release(&mut app, KeyCode::F10);

        press(&mut app, KeyCode::ShiftLeft);
        press(&mut app, KeyCode::F10);
        assert_eq!(selected(&app), Some(second));
    }
}
