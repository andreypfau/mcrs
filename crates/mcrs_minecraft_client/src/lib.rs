#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable,
    clippy::unwrap_used,
    reason = "an application has no caller to return an error to"
)]
// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

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
pub mod cave;
pub mod chat;
pub mod columns;
pub mod config;
#[cfg(feature = "dev")]
pub mod dev;
pub mod game_mode;
pub mod gui;
pub mod input;
pub mod item_model;
pub mod light_volume;
pub mod local_player;
pub mod model;
pub mod player;
pub mod registries;
pub mod render;
pub mod screen;
#[cfg(not(target_family = "wasm"))]
pub mod screenshot;
pub mod sky;
pub mod sky_state;
pub mod stream;
pub mod vanilla;
#[cfg(target_family = "wasm")]
pub mod web;

/// The default asset source over `roots`: the packs under the assets root layered over its
/// files and the game's data pack, with no built-in answer for a path none of them holds.
#[cfg(not(target_family = "wasm"))]
pub fn asset_source(
    roots: &mcrs_minecraft_assets::packs::PackRoots,
) -> bevy::asset::io::AssetSourceBuilder {
    use bevy::asset::io::AssetSourceBuilder;
    use mcrs_minecraft_assets::packs::{PackLayers, pack_files_reader};

    let mut files = pack_files_reader(roots);
    AssetSourceBuilder::platform_default(&roots.assets, None)
        .with_reader(move || Box::new(PackLayers::new(files())))
}

/// A plugin still has work a scripted capture must wait for.
#[derive(Message)]
pub struct Unsettled;

pub struct ClientPlugins;

impl PluginGroup for ClientPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(vanilla::VanillaAssetsPlugin)
            .add(player::PlayerPlugin)
            .add(input::ClientInputPlugin)
            .add(local_player::LocalPlayerPlugin)
            .add(camera::CameraPlugin)
            .add(gui::debug::DebugScreenPlugin)
            .add(gui::chunk_map::ChunkMapPlugin)
            .add(gui::light_levels::LightLevelsPlugin)
            .add(registries::ClientRegistriesPlugin::default())
            .add(sky::SkyPlugin)
    }
}

pub struct ClientTerrainPlugin(pub config::TerrainLimits);

impl Plugin for ClientTerrainPlugin {
    fn build(&self, app: &mut App) {
        let knobs = config::Knobs::of(app);
        let (budget, uploads, cave) = knobs.terrain(self.0);
        app.add_plugins(mcrs_minecraft_render::TerrainPlugin {
            budget: budget.clone(),
            uploads: uploads.clone(),
        })
        .add_plugins(mcrs_minecraft_render_probe::ProbePlugin {
            heat: knobs.gpu_hot(),
            timestamps: knobs.pass_timestamps(),
        })
        .add_plugins(render::GuiItemsPlugin)
        .insert_resource(mcrs_minecraft_render::Occlusion(knobs.occlusion()))
        .insert_resource(mcrs_minecraft_render::Brightness(knobs.brightness()))
        .add_plugins(stream::StreamPlugin::new(
            budget,
            uploads,
            stream::MeshPacing {
                per_frame: knobs.mesh_per_frame(),
                in_flight: knobs.mesh_in_flight(),
            },
        ))
        .insert_resource(cave)
        .add_systems(Update, (cave::toggle, toggle_culls, step_debug_view))
        .add_systems(
            PostUpdate,
            cave::cave_cull.after(VisibilitySystems::UpdateFrusta),
        );
        if let Some(tick) = knobs.frozen_time() {
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
