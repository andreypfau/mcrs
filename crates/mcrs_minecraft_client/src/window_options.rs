use std::time::Duration;

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use bevy::winit::{UpdateMode, WinitSettings};
use mcrs_minecraft_client_options::{ClientOptions, MAX_FRAMERATE_RANGE};

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
        // chisle: a failed switch or a change by the system does not reach the option
        // (MENU-DISP-003 steps 5, 6): Bevy 0.20 never writes the real mode back to `Window::mode`.
        // A Bevy that reports the real mode lifts this.
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
