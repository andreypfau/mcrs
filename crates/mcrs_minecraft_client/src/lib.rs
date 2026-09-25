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
        .insert_resource(config::render_path())
        .insert_resource(config::occlusion())
        .insert_resource(mcrs_minecraft_render::Brightness(config::brightness()))
        .insert_resource(config::drawn_streams())
        .insert_resource(config::raster_fraction())
        .add_plugins(stream::StreamPlugin::new(budget, uploads))
        .insert_resource(cave)
        .add_systems(
            Update,
            (
                cave::toggle,
                step_debug_view,
                toggle_render_path,
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
    shown: Res<mcrs_minecraft_render::ShownPath>,
    mut selected: ResMut<mcrs_minecraft_render::SelectedView>,
) {
    if keys.just_pressed(KeyCode::F10) {
        let back = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        selected.0 = views.step(shown.get(), selected.0, back);
    }
}

fn toggle_render_path(
    keys: Res<ButtonInput<KeyCode>>,
    mut path: ResMut<mcrs_minecraft_render::RenderPath>,
    mut selected: ResMut<mcrs_minecraft_render::SelectedView>,
) {
    use mcrs_minecraft_render::{RenderPath, SelectedView};
    if keys.just_pressed(KeyCode::F7) {
        *path = match *path {
            RenderPath::Classic => RenderPath::Deferred,
            RenderPath::Deferred => RenderPath::Classic,
        };
        *selected = SelectedView(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_render::{DebugViews, RenderPath, SelectedView, ShownPath};

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
    fn f7_switches_the_requested_render_path_back_and_forth() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<RenderPath>()
            .init_resource::<SelectedView>()
            .add_systems(Update, toggle_render_path);
        let path = |app: &App| *app.world().resource::<RenderPath>();

        press(&mut app, KeyCode::F7);
        assert_eq!(path(&app), RenderPath::Deferred);
        release(&mut app, KeyCode::F7);
        assert_eq!(path(&app), RenderPath::Deferred);
        press(&mut app, KeyCode::F7);
        assert_eq!(path(&app), RenderPath::Classic);
    }

    #[test]
    fn f10_steps_forward_and_shift_f10_steps_back() {
        let mut views = DebugViews::default();
        let first = views.register(RenderPath::Classic, "first");
        let second = views.register(RenderPath::Classic, "second");
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SelectedView>()
            .init_resource::<ShownPath>()
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

    #[test]
    fn f7_returns_to_final_shading() {
        let mut views = DebugViews::default();
        let view = views.register(RenderPath::Classic, "view");
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<RenderPath>()
            .insert_resource(SelectedView(Some(view)))
            .add_systems(Update, toggle_render_path);

        press(&mut app, KeyCode::F7);
        assert_eq!(app.world().resource::<SelectedView>().0, None);
    }
}
