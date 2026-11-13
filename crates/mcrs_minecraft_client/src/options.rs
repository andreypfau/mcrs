use std::path::{Path, PathBuf};

use bevy::prelude::*;
use mcrs_minecraft_client_gui::menu_options_systems::MenuOptionsSystems;
use mcrs_minecraft_client_gui::scene::GuiConfig;
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_mesh::smooth_light::SmoothLight;
use mcrs_minecraft_render::Brightness;

use crate::camera::FovOptions;
use crate::sky::Clouds;
use crate::stream::LightOptions;

/// Keeps the client options as the one resource that the field of view, the terrain light, the
/// clouds, the brightness and the GUI scale follow, and writes them to `file` as JSON each time
/// they change after start-up. The web has no file, so there `file` is ignored.
pub struct ClientOptionsPlugin {
    pub options: ClientOptions,
    pub file: Option<PathBuf>,
    /// The GUI scale the client shows in place of the one of the options; it is never saved.
    pub gui_scale: Option<u32>,
}

#[derive(Resource)]
struct GuiScaleOverride(Option<u32>);

impl Plugin for ClientOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.options)
            .insert_resource(GuiScaleOverride(self.gui_scale))
            .add_systems(
                PreUpdate,
                project_options
                    .after(MenuOptionsSystems)
                    .run_if(resource_changed::<ClientOptions>),
            );
        #[cfg(not(target_family = "wasm"))]
        if let Some(path) = &self.file {
            crate::options_file::save_options_to(app, path.clone());
        }
    }
}

/// Reads and validates the options file at `path`.
pub(crate) fn read(path: &Path) -> Result<ClientOptions, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let options: ClientOptions = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    options.validate().map_err(|error| error.to_string())?;
    Ok(options)
}

fn project_options(
    options: Res<ClientOptions>,
    gui_scale: Res<GuiScaleOverride>,
    fov: Option<ResMut<FovOptions>>,
    light: Option<ResMut<LightOptions>>,
    clouds: Option<ResMut<Clouds>>,
    brightness: Option<ResMut<Brightness>>,
    gui: Option<ResMut<GuiConfig>>,
) {
    if let Some(mut fov) = fov {
        fov.set_if_neq(FovOptions {
            fov: options.fov,
            effect_scale: options.fov_effect_scale,
        });
    }
    if let Some(mut light) = light {
        light.set_if_neq(LightOptions {
            smooth_light: if options.smooth_light {
                SmoothLight::On
            } else {
                SmoothLight::Off
            },
        });
    }
    if let Some(mut clouds) = clouds {
        clouds.set_if_neq(options.clouds.into());
    }
    if let Some(brightness) = brightness {
        brightness
            .map_unchanged(|brightness| &mut brightness.0)
            .set_if_neq(options.brightness);
    }
    if let Some(gui) = gui {
        gui.map_unchanged(|config| &mut config.scale)
            .set_if_neq(gui_scale.0.unwrap_or(options.gui_scale));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(options: ClientOptions, gui_scale: Option<u32>) -> App {
        let mut app = App::new();
        app.init_resource::<FovOptions>()
            .init_resource::<LightOptions>()
            .init_resource::<Clouds>()
            .init_resource::<Brightness>()
            .insert_resource(GuiConfig {
                scale: 0,
                cursor: None,
                frozen_ticks: None,
                advanced_tooltips: false,
            })
            .add_plugins(ClientOptionsPlugin {
                options,
                file: None,
                gui_scale,
            });
        app.update();
        app
    }

    #[test]
    fn the_start_up_options_and_a_later_change_reach_each_projection() {
        let mut options = ClientOptions::default();
        options.fov = 80;
        let mut app = app_with(options, None);
        assert_eq!(app.world().resource::<FovOptions>().fov, 80);

        let cases: [(fn(&mut ClientOptions), fn(&World) -> bool); 5] = [
            (
                |options| options.fov = 90,
                |world| world.resource::<FovOptions>().fov == 90,
            ),
            (
                |options| options.brightness = 1.0,
                |world| world.resource::<Brightness>().0 == 1.0,
            ),
            (
                |options| options.gui_scale = 3,
                |world| world.resource::<GuiConfig>().scale == 3,
            ),
            (
                |options| options.smooth_light = false,
                |world| world.resource::<LightOptions>().smooth_light == SmoothLight::Off,
            ),
            (
                |options| options.clouds = false,
                |world| *world.resource::<Clouds>() == Clouds::Off,
            ),
        ];
        for (index, (change, projected)) in cases.into_iter().enumerate() {
            change(&mut app.world_mut().resource_mut::<ClientOptions>());
            app.update();
            assert!(projected(app.world()), "case {index}");
        }
    }

    #[test]
    fn a_gui_scale_of_the_plugin_wins_over_the_options_and_stays_out_of_them() {
        let mut app = app_with(ClientOptions::default(), Some(2));
        app.world_mut().resource_mut::<ClientOptions>().gui_scale = 3;
        app.update();
        assert_eq!(app.world().resource::<GuiConfig>().scale, 2);
        assert_eq!(app.world().resource::<ClientOptions>().gui_scale, 3);
    }
}
