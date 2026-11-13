use std::time::Duration;

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode, WindowResized};
use bevy::winit::{UpdateMode, WINIT_WINDOWS, WinitSettings};
use mcrs_minecraft_client_options::{ClientOptions, MAX_FRAMERATE_RANGE};

// Twice the space transition of macOS, about a second, during which winit holds a new switch back.
pub(crate) const FULLSCREEN_GRACE: Duration = Duration::from_secs(2);

/// The fullscreen state of the primary window as the window system reports it. The window opens in
/// the mode of the start-up options.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct WindowFullscreen(pub(crate) bool);

/// How long the fullscreen state of the window can still change without a message.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum FullscreenWatch {
    #[default]
    Idle,
    /// The window changed: until `until`, a new state reaches the option at once.
    Window { until: Duration },
    /// The option asked for `to`; the state of the window reaches the option only at `until`.
    Switch { to: bool, until: Duration },
}

/// The mode of the window for Fullscreen: a borderless window over the primary monitor
/// (MENU-DISP-004).
#[must_use]
pub fn window_mode(fullscreen: bool) -> WindowMode {
    if fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    } else {
        WindowMode::Windowed
    }
}

/// The present mode for VSync (MENU-DISP-002): without it, Immediate, else Mailbox, else FIFO.
#[must_use]
pub fn present_mode(vsync: bool) -> PresentMode {
    if vsync {
        PresentMode::Fifo
    } else {
        PresentMode::AutoNoVsync
    }
}

/// The frame limit of Max Framerate (MENU-DISP-001): each wait ends `1 / max_framerate` seconds
/// after the end of the one before it, at once when the frame is late. Input does not end a wait,
/// and the focus does not change the limit.
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

/// Gives Fullscreen the state of the window: at once when the window changes it, and when a switch
/// does not take, once the window rested for `FULLSCREEN_GRACE` (MENU-DISP-003 steps 5 and 6).
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
