use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use mcrs_minecraft_mesh::DrawKind;
use mcrs_minecraft_render::{DrawArgRow, DrawArgs, FrameCounts, Terrain, VERTICES_PER_QUAD};

use crate::readback::{self, Gate, Reader};

const TRIANGLES_PER_QUAD: u32 = 2;

fn row_slice(args: &[DrawArgs], row: DrawArgRow) -> &[DrawArgs] {
    let start = row.first_slot();
    args.get(start..start + DrawKind::COUNT).unwrap_or(&[])
}

fn drawn_quads(args: &[DrawArgs]) -> u32 {
    [DrawArgRow::FirstPassDraws, DrawArgRow::SecondPassDraws]
        .into_iter()
        .flat_map(|row| row_slice(args, row))
        .map(|draw| draw.vertex_count / VERTICES_PER_QUAD)
        .sum()
}

fn hidden_quads(args: &[DrawArgs]) -> u32 {
    let hidden: u32 = row_slice(args, DrawArgRow::HiddenQuads)
        .iter()
        .map(|draw| draw.vertex_count)
        .sum();
    let revived: u32 = row_slice(args, DrawArgRow::SecondPassDraws)
        .iter()
        .map(|draw| draw.vertex_count / VERTICES_PER_QUAD)
        .sum();
    hidden.saturating_sub(revived)
}

#[derive(Resource, Clone, Default)]
pub struct ReportedCounts(Arc<FrameCounts>);

impl ReportedCounts {
    pub fn terrain_draws(&self) -> u32 {
        self.0.terrain_draws.load(Ordering::Relaxed)
    }

    pub fn sky_draws(&self) -> u32 {
        self.0.sky_draws.load(Ordering::Relaxed)
    }

    pub fn upload_bytes(&self) -> u32 {
        self.0.upload_bytes.load(Ordering::Relaxed)
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "the render counters are atomics that change detection does not see, so the copy runs on every frame"
    )
)]
pub(super) fn report_counts(counts: Res<FrameCounts>, reported: Res<ReportedCounts>) {
    let copy = |to: &AtomicU32, from: &AtomicU32| {
        to.store(from.load(Ordering::Relaxed), Ordering::Relaxed);
    };
    copy(&reported.0.terrain_draws, &counts.terrain_draws);
    copy(&reported.0.sky_draws, &counts.sky_draws);
    copy(&reported.0.upload_bytes, &counts.upload_bytes);
}

#[derive(Resource, Clone, Default)]
pub struct DrawnTriangles(Arc<Counted>);

impl DrawnTriangles {
    pub fn get(&self) -> u32 {
        self.0.triangles.load(Ordering::Relaxed)
    }

    pub fn hidden(&self) -> u32 {
        self.0.hidden.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub struct Counted {
    triangles: AtomicU32,
    hidden: AtomicU32,
    gate: Gate,
}

impl Reader for Counted {
    fn gate(&self) -> &Gate {
        &self.gate
    }

    fn read(&self, bytes: &[u8]) {
        let Ok(args) = bytemuck::try_cast_slice::<u8, DrawArgs>(bytes) else {
            return;
        };
        self.triangles
            .store(drawn_quads(args) * TRIANGLES_PER_QUAD, Ordering::Relaxed);
        self.hidden
            .store(hidden_quads(args) * TRIANGLES_PER_QUAD, Ordering::Relaxed);
    }
}

#[derive(Resource)]
pub(super) struct ArgsReadback(Buffer);

pub(super) fn init_args_readback(
    mut commands: Commands,
    terrain: Option<Res<Terrain>>,
    device: Res<RenderDevice>,
) {
    let Some(args) = terrain.as_deref().and_then(Terrain::draw_args) else {
        return;
    };
    commands.insert_resource(ArgsReadback(device.create_buffer(&BufferDescriptor {
        label: Some("terrain draw args readback"),
        size: args.size(),
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })));
}

pub(super) fn copy_args(
    terrain: &Terrain,
    readback: &ArgsReadback,
    triangles: &DrawnTriangles,
    encoder: &mut CommandEncoder,
) {
    let Some(args) = terrain.draw_args() else {
        return;
    };
    if !triangles.0.gate.claim_copy() {
        return;
    }
    encoder.copy_buffer_to_buffer(args, 0, &readback.0, 0, readback.0.size());
}

pub(super) fn read_draw_args(readback: Res<ArgsReadback>, triangles: Res<DrawnTriangles>) {
    if !triangles.0.gate.claim_map() {
        return;
    }
    readback::map(readback.0.clone(), triangles.0.clone());
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_render::args_reset;

    use super::*;

    #[test]
    fn the_triangle_count_is_two_per_drawn_quad() {
        let counted = Counted::default();
        let mut args = args_reset();
        let draws = DrawKind::ALL;
        args[DrawArgRow::FirstPassDraws.slot(draws[0])].vertex_count = 3 * VERTICES_PER_QUAD;
        args[DrawArgRow::FirstPassDraws.slot(draws[1])].vertex_count = 5 * VERTICES_PER_QUAD;
        counted.read(bytemuck::cast_slice(&args));
        assert_eq!(counted.triangles.load(Ordering::Relaxed), 16);
    }
}
