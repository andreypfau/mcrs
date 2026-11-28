use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
use mcrs_minecraft_core::VERSION;

use crate::debug_displayer::DebugScreenDisplayer;
use mcrs_minecraft_render_probe::probe::{CPU_WINDOW, WINDOW_SECS};

pub const HISTORY: usize = CPU_WINDOW;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "frame time measurements carry wall-clock instants, so the one-second window compares them with the wall clock"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a debug screen refresh, at most ten times a second while the F3 overlay shows or when the stats log is due"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(3))]
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
    let at = |q: f64| {
        let index = ((sorted.len() as f64 * q) as usize).min(sorted.len().saturating_sub(1));
        sorted.get(index).copied().unwrap_or(0.0)
    };
    let vsync = matches!(
        window.present_mode,
        PresentMode::AutoVsync | PresentMode::Fifo | PresentMode::FifoRelaxed
    );
    let line = if sorted.is_empty() {
        "0 fps T: inf".to_owned()
    } else {
        format!(
            "{} fps ({:.3} ms median, {:.3} p99, {:.2} max over the last second) T: inf{}",
            sorted.len(),
            at(0.5),
            at(0.99),
            sorted.last().copied().unwrap_or(0.0),
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

const BRAND: &str = "mcrs";

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a debug screen refresh, at most ten times a second while the F3 overlay shows or when the stats log is due, and the displayer takes owned lines"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
pub fn display_version(mut displayer: ResMut<DebugScreenDisplayer>) {
    displayer.add_priority_line(format!(
        "Minecraft {} ({}/{BRAND})",
        VERSION.name,
        env!("CARGO_PKG_VERSION")
    ));
}
