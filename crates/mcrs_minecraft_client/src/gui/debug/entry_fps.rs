use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};

use super::DebugScreenDisplayer;
use crate::probe::{CPU_WINDOW, WINDOW_SECS};

/// Room for a second of frames at the rate being aimed for.
pub const HISTORY: usize = CPU_WINDOW;

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    diagnostics: Res<DiagnosticsStore>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut sorted: Local<Vec<f64>>,
) {
    let frame_time = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME);
    let now = Instant::now();
    sorted.clear();
    sorted.extend(
        frame_time
            .into_iter()
            .flat_map(|d| d.measurements())
            .filter(|m| now.saturating_duration_since(m.time).as_secs_f32() <= WINDOW_SECS)
            .map(|m| m.value),
    );
    sorted.sort_unstable_by(f64::total_cmp);
    let at = |q: f64| sorted[((sorted.len() as f64 * q) as usize).min(sorted.len() - 1)];
    let vsync = matches!(
        window.present_mode,
        PresentMode::AutoVsync | PresentMode::Fifo | PresentMode::FifoRelaxed
    );
    // Frames in the last second, as vanilla counts them; no framerate limiter here, which is
    // what vanilla prints as "inf".
    let line = if sorted.is_empty() {
        "0 fps T: inf".to_owned()
    } else {
        format!(
            "{} fps ({:.3} ms median, {:.3} p99, {:.2} max over the last second) T: inf{}",
            sorted.len(),
            at(0.5),
            at(0.99),
            sorted[sorted.len() - 1],
            if vsync { " vsync" } else { "" }
        )
    };
    displayer.add_priority_line(line);
    displayer.add_priority_line(format!(
        "{}x{} physical, scale {:.2}",
        window.physical_width(),
        window.physical_height(),
        window.scale_factor()
    ));
}
