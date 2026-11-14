use std::path::{Path, PathBuf};

use bevy::prelude::*;
use mcrs_minecraft_client_gui::menu_options_systems::MenuOptionsSystems;
use mcrs_minecraft_client_gui::scene::GuiConfig;
use mcrs_minecraft_client_options::{ClientOptions, VIEW_DISTANCE_DEFAULT};
use mcrs_minecraft_client_world::player_view::RequestedViewDistance;
use mcrs_minecraft_mesh::smooth_light::SmoothLight;
use mcrs_minecraft_render::{Brightness, ImprovedTransparency};

use crate::camera::FovOptions;
use crate::sky::Clouds;
use crate::stream::LightOptions;

/// Keeps the client options as the one resource that the field of view, the terrain light, the
/// clouds, the brightness, Improved Transparency, the GUI scale, the requested view distance and, on a native client,
/// the window mode, the present mode and the frame limit follow, and writes them to `file` as
/// JSON each time they change after start-up. The web has no file, so there `file` is ignored.
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
            .insert_resource(requested_view_distance(&self.options))
            .insert_resource(GuiScaleOverride(self.gui_scale))
            .add_systems(
                PreUpdate,
                project_options
                    .after(MenuOptionsSystems)
                    .run_if(resource_changed::<ClientOptions>),
            );
        #[cfg(not(target_family = "wasm"))]
        {
            use bevy::window::{PrimaryWindow, WindowResized};

            use crate::window_options::{
                FullscreenWatch, WindowFullscreen, project_window_options, read_window_fullscreen,
                reconcile_fullscreen,
            };

            let watched = || not(resource_equals(FullscreenWatch::Idle));
            app.insert_resource(WindowFullscreen(self.options.fullscreen))
                .init_resource::<FullscreenWatch>()
                .add_systems(
                    PreUpdate,
                    (
                        read_window_fullscreen.run_if(watched()),
                        reconcile_fullscreen.run_if(
                            any_with_component::<PrimaryWindow>.and_then(
                                on_message::<WindowResized>
                                    .or_else(resource_changed::<ClientOptions>)
                                    .or_else(resource_changed::<WindowFullscreen>)
                                    .or_else(watched()),
                            ),
                        ),
                        project_window_options.run_if(resource_changed::<ClientOptions>),
                    )
                        .chain()
                        .after(MenuOptionsSystems),
                );
        }
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

#[must_use]
fn requested_view_distance(options: &ClientOptions) -> RequestedViewDistance {
    RequestedViewDistance(options.view_distance.unwrap_or(VIEW_DISTANCE_DEFAULT))
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and writer is its own parameter"
)]
fn project_options(
    options: Res<ClientOptions>,
    gui_scale: Res<GuiScaleOverride>,
    mut view_distance: ResMut<RequestedViewDistance>,
    fov: Option<ResMut<FovOptions>>,
    light: Option<ResMut<LightOptions>>,
    clouds: Option<ResMut<Clouds>>,
    brightness: Option<ResMut<Brightness>>,
    improved_transparency: Option<ResMut<ImprovedTransparency>>,
    gui: Option<ResMut<GuiConfig>>,
) {
    view_distance.set_if_neq(requested_view_distance(&options));
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
    if let Some(mut improved_transparency) = improved_transparency {
        improved_transparency.set_if_neq(ImprovedTransparency(options.improved_transparency));
    }
    if let Some(gui) = gui {
        gui.map_unchanged(|config| &mut config.scale)
            .set_if_neq(gui_scale.0.unwrap_or(options.gui_scale));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode, WindowResized};
    use bevy::winit::{UpdateMode, WinitSettings};

    use super::*;
    use crate::window_options::{FULLSCREEN_GRACE, FullscreenWatch, WindowFullscreen, window_mode};

    const FRAME: Duration = Duration::from_millis(100);

    fn primary_window(world: &World) -> &Window {
        world
            .try_query_filtered::<&Window, With<PrimaryWindow>>()
            .and_then(|mut windows| windows.single(world).ok())
            .unwrap()
    }

    fn frame_limit(world: &World) -> UpdateMode {
        let settings = world.resource::<WinitSettings>();
        assert_eq!(settings.focused_mode, settings.unfocused_mode);
        settings.focused_mode
    }

    fn app_with(options: ClientOptions, gui_scale: Option<u32>) -> App {
        let mut app = App::new();
        app.init_resource::<FovOptions>()
            .init_resource::<LightOptions>()
            .init_resource::<Clouds>()
            .init_resource::<Brightness>()
            .init_resource::<ImprovedTransparency>()
            .insert_resource(GuiConfig {
                scale: 0,
                cursor: None,
                frozen_ticks: None,
                advanced_tooltips: false,
            })
            .init_resource::<WinitSettings>()
            .init_resource::<Time<Real>>()
            .add_message::<WindowResized>()
            .add_plugins(ClientOptionsPlugin {
                options,
                file: None,
                gui_scale,
            });
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.update();
        app
    }

    fn grace_frames() -> u32 {
        u32::try_from(FULLSCREEN_GRACE.as_millis() / FRAME.as_millis()).unwrap()
    }

    fn run_frames(app: &mut App, frames: u32) {
        for _ in 0..frames {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(FRAME);
            app.update();
        }
    }

    fn option_fullscreen(app: &App) -> bool {
        app.world().resource::<ClientOptions>().fullscreen
    }

    fn set_window_fullscreen(app: &mut App, fullscreen: bool) {
        app.world_mut().resource_mut::<WindowFullscreen>().0 = fullscreen;
    }

    /// The option and the window mode stay at `fullscreen`, nothing writes the options again, and
    /// the fullscreen systems go idle.
    fn assert_rests_at(app: &mut App, fullscreen: bool) {
        let changed = app.world().resource_ref::<ClientOptions>().last_changed();
        run_frames(app, 2 * grace_frames());
        assert_eq!(option_fullscreen(app), fullscreen);
        assert_eq!(primary_window(app.world()).mode, window_mode(fullscreen));
        assert_eq!(
            app.world().resource_ref::<ClientOptions>().last_changed(),
            changed,
            "the options change no more"
        );
        assert_eq!(
            *app.world().resource::<FullscreenWatch>(),
            FullscreenWatch::Idle
        );
    }

    fn idle_app(fullscreen: bool) -> App {
        let mut options = ClientOptions::default();
        options.fullscreen = fullscreen;
        let mut app = app_with(options, None);
        run_frames(&mut app, grace_frames());
        app
    }

    #[test]
    fn a_change_of_the_window_by_the_system_reaches_the_option_in_that_frame() {
        for system_fullscreen in [true, false] {
            let mut app = idle_app(!system_fullscreen);
            set_window_fullscreen(&mut app, system_fullscreen);
            run_frames(&mut app, 1);
            assert_eq!(option_fullscreen(&app), system_fullscreen);
            assert_rests_at(&mut app, system_fullscreen);
        }
    }

    #[test]
    fn a_switch_that_the_window_follows_within_the_grace_period_changes_nothing_more() {
        for delay in [1, grace_frames() - 1] {
            let mut app = idle_app(false);
            app.world_mut().resource_mut::<ClientOptions>().fullscreen = true;
            run_frames(&mut app, delay);
            assert!(option_fullscreen(&app), "delay {delay}");
            set_window_fullscreen(&mut app, true);
            assert_rests_at(&mut app, true);
        }
    }

    #[test]
    fn a_switch_that_the_window_never_follows_gives_the_option_the_window_state_once() {
        let mut app = idle_app(false);
        app.world_mut().resource_mut::<ClientOptions>().fullscreen = true;
        run_frames(&mut app, grace_frames());
        assert!(option_fullscreen(&app), "the switch is still in flight");
        run_frames(&mut app, 1);
        assert!(!option_fullscreen(&app));
        assert_rests_at(&mut app, false);
    }

    #[test]
    fn the_start_up_options_and_a_later_change_reach_each_projection() {
        let mut options = ClientOptions::default();
        options.fov = 80;
        let mut app = app_with(options, None);
        assert_eq!(app.world().resource::<FovOptions>().fov, 80);
        assert_eq!(
            primary_window(app.world()).present_mode,
            PresentMode::AutoNoVsync
        );

        let cases: [(fn(&mut ClientOptions), fn(&World) -> bool); 10] = [
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
            (
                |options| options.improved_transparency = true,
                |world| world.resource::<ImprovedTransparency>().0,
            ),
            (
                |options| options.vsync = true,
                |world| primary_window(world).present_mode == PresentMode::Fifo,
            ),
            (
                |options| options.fullscreen = true,
                |world| {
                    primary_window(world).mode
                        == WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
                },
            ),
            (
                |options| options.max_framerate = 60,
                |world| {
                    frame_limit(world)
                        == UpdateMode::Reactive {
                            wait: Duration::from_secs(1) / 60,
                            react_to_device_events: false,
                            react_to_user_events: false,
                            react_to_window_events: false,
                        }
                },
            ),
            (
                |options| options.max_framerate = 260,
                |world| frame_limit(world) == UpdateMode::Continuous,
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
