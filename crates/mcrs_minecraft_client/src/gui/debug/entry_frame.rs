use bevy::prelude::*;
use mcrs_minecraft_core::resource_location::ResourceLocation;

use super::{DebugEntryGroup, DebugScreenDisplayer};
use crate::probe::{self, CpuTimings, GpuTimings};
use crate::render::FrameCounts;

pub const GROUP: DebugEntryGroup = ResourceLocation::new_static("mcrs:frame");

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    cpu: Res<CpuTimings>,
    gpu: Res<GpuTimings>,
    counts: Res<FrameCounts>,
) {
    let stage = |slot: usize| cpu.median(slot).unwrap_or(0.0);
    let tail = |slot: usize| match cpu.spread(slot) {
        Some(spread) => format!("{:.2}/{:.2}", spread.p99, spread.max),
        None => "-".to_owned(),
    };
    let (terrain_draws, sky_draws) = counts.draws();
    let engine = match cpu.spread(probe::ENGINE) {
        Some(spread) => format!(
            "Engine: {:.3} ms median, {:.3} p99, {:.2} max over {} frames in the last second",
            spread.median, spread.p99, spread.max, spread.frames
        ),
        None => "Engine: no frames yet".to_owned(),
    };
    let mut lines = vec![
        engine,
        format!(
            "CPU: main {:.3} ms, extract {:.3}, prepare {:.3} (acquire {:.3}), render {:.3}, \
             cleanup {:.3}",
            stage(probe::MAIN),
            stage(probe::EXTRACT),
            stage(probe::PREPARE),
            stage(probe::ACQUIRE),
            stage(probe::RENDER),
            stage(probe::CLEANUP),
        ),
        format!(
            "CPU p99/max: main {}, extract {}, prepare {}, acquire {}, render {}, cleanup {}",
            tail(probe::MAIN),
            tail(probe::EXTRACT),
            tail(probe::PREPARE),
            tail(probe::ACQUIRE),
            tail(probe::RENDER),
            tail(probe::CLEANUP),
        ),
        format!("Draws: {terrain_draws} terrain, {sky_draws} sky"),
        format!(
            "Upload: {} KB of {} KB",
            counts.upload_bytes() >> 10,
            crate::config::upload_budget() >> 10
        ),
    ];
    for (slot, name) in probe::NAMES.iter().enumerate() {
        if let Some(ms) = gpu.median(slot) {
            lines.push(format!("GPU {name}: {ms:.3} ms"));
        }
    }
    displayer.add_to_group(GROUP, lines);
}
