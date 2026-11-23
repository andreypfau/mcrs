use bevy::camera::visibility::VisibilitySystems;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, RenderApp};
use mcrs_minecraft_client_gui::debug_entry_system_specs;
use mcrs_minecraft_client_gui::debug_screen::{DebugScreenSet, collecting};

use crate::{cave, config, entities, sign_text, stream, terrain_debug_entry};

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
        .add_plugins(mcrs_minecraft_client_gui::gui_items::GuiItemsPlugin)
        .add_plugins(entities::ClientEntityRenderPlugin)
        .add_plugins(sign_text::SignTextPlugin)
        .add_plugins(crate::name_tag::NameTagPlugin)
        .insert_resource(mcrs_minecraft_render::Occlusion(knobs.occlusion()))
        .add_plugins(stream::StreamPlugin::new(
            budget,
            uploads,
            stream::MeshPacing {
                per_frame: knobs.mesh_per_frame(),
                in_flight: knobs.mesh_in_flight(),
            },
        ))
        .insert_resource(cave)
        .add_systems(
            Update,
            (
                cave::toggle.run_if(input_just_pressed(KeyCode::KeyC)),
                toggle_culls.run_if(
                    input_just_pressed(KeyCode::KeyO).or_else(input_just_pressed(KeyCode::F8)),
                ),
                step_debug_view.run_if(input_just_pressed(KeyCode::F10)),
                terrain_debug_entry::display
                    .after(debug_entry_system_specs::display)
                    .in_set(DebugScreenSet::Collect)
                    .run_if(collecting)
                    .run_if(resource_exists::<stream::Loader>),
            ),
        )
        .add_systems(
            PostUpdate,
            cave::cave_cull.after(VisibilitySystems::UpdateFrusta),
        );
        #[cfg(not(target_family = "wasm"))]
        app.add_plugins(crate::title::TitlePanoramaPlugin);
        if let Some(tick) = knobs.frozen_time() {
            app.insert_resource(mcrs_minecraft_render::PinnedTick(tick));
        }
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                ExtractSchedule,
                cave::extract_cave_visibility
                    .run_if(resource_exists::<mcrs_minecraft_render::Terrain>),
            );
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
        selected.set_if_neq(mcrs_minecraft_render::SelectedView(
            views.step(selected.0, back),
        ));
    }
}

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
            .add_systems(
                Update,
                step_debug_view.run_if(input_just_pressed(KeyCode::F10)),
            );
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
