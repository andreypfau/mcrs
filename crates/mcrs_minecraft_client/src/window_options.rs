use std::time::Duration;

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode, WindowResized};
use bevy::winit::{UpdateMode, WINIT_WINDOWS, WinitSettings};
use mcrs_minecraft_client_options::{ClientOptions, MAX_FRAMERATE_RANGE};

pub(crate) const FULLSCREEN_GRACE: Duration = Duration::from_secs(2);

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct WindowFullscreen(pub(crate) bool);

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum FullscreenWatch {
    #[default]
    Idle,
    Window {
        until: Duration,
    },
    Switch {
        to: bool,
        until: Duration,
    },
}

#[must_use]
pub fn window_mode(fullscreen: bool) -> WindowMode {
    if fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    } else {
        WindowMode::Windowed
    }
}

#[must_use]
pub fn present_mode(vsync: bool) -> PresentMode {
    if vsync {
        PresentMode::Fifo
    } else {
        PresentMode::AutoNoVsync
    }
}

fn update_mode(max_framerate: u16) -> UpdateMode {
    let unlimited = *MAX_FRAMERATE_RANGE.end();
    match Duration::from_secs(1).checked_div(u32::from(max_framerate)) {
        Some(wait) if max_framerate < unlimited => UpdateMode::Reactive {
            wait,
            react_to_device_events: false,
            react_to_user_events: false,
            react_to_window_events: false,
        },
        _ => UpdateMode::Continuous,
    }
}

pub(crate) fn project_window_options(
    options: Res<ClientOptions>,
    window: Option<Single<&mut Window, With<PrimaryWindow>>>,
    winit: Option<ResMut<WinitSettings>>,
) {
    if let Some(mut window) = window {
        let mode = window_mode(options.fullscreen);
        if window.mode != mode {
            window.mode = mode;
        }
        let present = present_mode(options.vsync);
        if window.present_mode != present {
            window.present_mode = present;
        }
    }
    if let Some(mut winit) = winit {
        let limit = update_mode(options.max_framerate);
        if winit.focused_mode != limit || winit.unfocused_mode != limit {
            *winit = WinitSettings {
                focused_mode: limit,
                unfocused_mode: limit,
            };
        }
    }
}

pub(crate) fn read_window_fullscreen(
    window: Single<Entity, With<PrimaryWindow>>,
    mut real: ResMut<WindowFullscreen>,
    _winit_thread: NonSendMarker,
) {
    let fullscreen = WINIT_WINDOWS.with_borrow(|windows| {
        windows
            .get_window(*window)
            .map(|window| window.fullscreen().is_some())
    });
    if let Some(fullscreen) = fullscreen {
        real.set_if_neq(WindowFullscreen(fullscreen));
    }
}

pub(crate) fn reconcile_fullscreen(
    time: Res<Time<Real>>,
    real: Res<WindowFullscreen>,
    mut resized: MessageReader<WindowResized>,
    mut options: ResMut<ClientOptions>,
    mut watch: ResMut<FullscreenWatch>,
) {
    let now = time.elapsed();
    let until = now.saturating_add(FULLSCREEN_GRACE);
    let window_changed = real.is_changed() || resized.read().count() > 0;
    let next = match *watch {
        FullscreenWatch::Switch { to, until: end } => {
            if options.fullscreen != to || window_changed {
                FullscreenWatch::Switch {
                    to: options.fullscreen,
                    until,
                }
            } else if now < end {
                return;
            } else {
                FullscreenWatch::Idle
            }
        }
        _ if options.fullscreen != real.0 && !real.is_changed() => FullscreenWatch::Switch {
            to: options.fullscreen,
            until,
        },
        _ if window_changed => FullscreenWatch::Window { until },
        FullscreenWatch::Window { until: end } if now < end => return,
        _ => FullscreenWatch::Idle,
    };
    if !matches!(next, FullscreenWatch::Switch { .. }) && options.fullscreen != real.0 {
        options.fullscreen = real.0;
    }
    watch.set_if_neq(next);
}
