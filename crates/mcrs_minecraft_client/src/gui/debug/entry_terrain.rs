use bevy::prelude::*;
use mcrs_minecraft_core::resource_location::ResourceLocation;

use super::{DebugEntryGroup, DebugScreenDisplayer};
use crate::cave::CaveCull;
use crate::probe::{self, GpuTimings};
use crate::render::DrawnTriangles;
use crate::stream::Loader;

pub const GROUP: DebugEntryGroup = ResourceLocation::new_static("minecraft:terrain");

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    triangles: Res<DrawnTriangles>,
    gpu: Res<GpuTimings>,
    cave: Res<CaveCull>,
    loader: Res<Loader>,
) {
    let status = loader.status();
    let mut lines = vec![
        format!("Tris: {}", triangles.get()),
        format!(
            "Sections: {}/{} in {} columns, {} evicted",
            status.sections, status.sections_total, status.columns, status.evicted
        ),
        format!(
            "Arena: {:.0}% quads, {:.0}% models",
            status.quads * 100.0,
            status.models * 100.0
        ),
        match (cave.enabled, cave.took_ms()) {
            (false, _) => "Sight lines: off".to_owned(),
            (true, None) => format!("Sight lines: {} sections", cave.reached()),
            (true, Some(ms)) => format!("Sight lines: {} sections in {ms:.3} ms", cave.reached()),
        },
    ];
    let dropped = triangles.dropped();
    if dropped > 0 {
        lines.push(format!("Visible list full: {dropped} quads dropped"));
    }
    for (slot, name) in probe::NAMES.iter().enumerate() {
        if let Some(ms) = gpu.median(slot) {
            lines.push(format!("GPU {name}: {ms:.2} ms"));
        }
    }
    displayer.add_to_group(GROUP, lines);
}
