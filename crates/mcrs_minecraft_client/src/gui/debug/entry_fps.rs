use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};

use super::DebugScreenDisplayer;

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    diagnostics: Res<DiagnosticsStore>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
        .unwrap_or(0.0);
    let vsync = matches!(
        window.present_mode,
        PresentMode::AutoVsync | PresentMode::Fifo | PresentMode::FifoRelaxed
    );
    // No framerate limiter here, which is what vanilla prints as "inf".
    displayer.add_priority_line(format!(
        "{fps:.0} fps T: inf{}",
        if vsync { " vsync" } else { "" }
    ));
}
