use bevy::prelude::*;
use mcrs_minecraft_core::resource_location::ResourceLocation;

use super::{DebugEntryGroup, DebugScreenDisplayer};
use crate::cave::CaveCull;
use crate::render::DrawnTriangles;
use crate::stream::Loader;

pub const GROUP: DebugEntryGroup = ResourceLocation::new_static("minecraft:terrain");

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    triangles: Res<DrawnTriangles>,
    cave: Res<CaveCull>,
    loader: Res<Loader>,
) {
    let status = loader.status();
    let lines = vec![
        format!(
            "Tris: {} ({} hidden behind terrain)",
            triangles.get(),
            triangles.hidden()
        ),
        format!(
            "Sections: {}/{} in {} columns, {} evicted",
            status.sections, status.sections_total, status.columns, status.evicted
        ),
        format!(
            "Mesh: {} queued, {} in flight, {} uploads waiting",
            status.queued, status.meshing, status.uploads_waiting
        ),
        format!(
            "Arena: {:.1}% quads, {:.1}% models, {:.1}% faces, {:.1}% groups",
            status.quads * 100.0,
            status.models * 100.0,
            status.faces * 100.0,
            status.groups * 100.0
        ),
        match (cave.enabled, cave.took_ms()) {
            (false, _) => "Sight lines: off".to_owned(),
            (true, None) => format!("Sight lines: {} sections", cave.reached()),
            (true, Some(ms)) => format!("Sight lines: {} sections in {ms:.3} ms", cave.reached()),
        },
    ];
    displayer.add_to_group(GROUP, lines);
}
