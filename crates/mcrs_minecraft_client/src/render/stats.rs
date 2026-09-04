use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::render_resource::*;

use crate::mesh::STREAMS;
use crate::readback::{self, Gate, Reader};

use super::layer::LayerGroup;
use super::terrain::Terrain;

pub(super) const DRAW_ARGS_SIZE: u64 = size_of::<DrawArgs>() as u64;

const VERTICES_PER_QUAD: u32 = 4;
const TRIANGLES_PER_QUAD: u32 = 2;

pub(super) type DrawArgs = DrawIndirectArgs;

pub(super) fn quad_strip() -> DrawArgs {
    DrawArgs {
        vertex_count: VERTICES_PER_QUAD,
        ..default()
    }
}

const NO_SLOT: u32 = u32::MAX;

/// The draw args as a frame starts: a strip per stream, then a counter per stream. An ordered
/// draw's counter starts its first slot at the top so the cull can lower it.
pub(super) fn args_reset() -> Vec<DrawArgs> {
    let ordered = |stream: usize| LayerGroup::of(stream as u32).culls_in_order();
    (0..STREAMS)
        .map(|_| quad_strip())
        .chain((0..STREAMS).map(|stream| DrawArgs {
            first_instance: if ordered(stream) { NO_SLOT } else { 0 },
            ..default()
        }))
        .collect()
}

/// An ordered draw spans holes, so its counter holds what actually survived; a packed draw's
/// instance count is that already.
fn drawn_quads(args: &[DrawArgs]) -> u32 {
    (0..STREAMS)
        .map(|stream| {
            if LayerGroup::of(stream as u32).culls_in_order() {
                args[STREAMS + stream].instance_count
            } else {
                args[stream].instance_count
            }
        })
        .sum()
}

/// What the CPU issued this frame, kept where the main world can read it a frame later.
#[derive(Resource, Clone, Default)]
pub struct FrameCounts(Arc<Counts>);

#[derive(Default)]
struct Counts {
    terrain_draws: AtomicU32,
    sky_draws: AtomicU32,
    upload_bytes: AtomicU32,
}

impl FrameCounts {
    pub fn draws(&self) -> (u32, u32) {
        (
            self.0.terrain_draws.load(Ordering::Relaxed),
            self.0.sky_draws.load(Ordering::Relaxed),
        )
    }

    pub fn upload_bytes(&self) -> u32 {
        self.0.upload_bytes.load(Ordering::Relaxed)
    }

    pub(crate) fn set_terrain_draws(&self, draws: u32) {
        self.0.terrain_draws.store(draws, Ordering::Relaxed);
    }

    pub(crate) fn set_sky_draws(&self, draws: u32) {
        self.0.sky_draws.store(draws, Ordering::Relaxed);
    }

    pub(crate) fn set_upload_bytes(&self, bytes: usize) {
        self.0
            .upload_bytes
            .store(bytes.try_into().unwrap_or(u32::MAX), Ordering::Relaxed);
    }
}

#[derive(Resource, Clone, Default)]
pub struct DrawnTriangles(Arc<Counted>);

impl DrawnTriangles {
    pub fn get(&self) -> u32 {
        self.0.triangles.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub struct Counted {
    triangles: AtomicU32,
    gate: Gate,
}

impl Reader for Counted {
    fn gate(&self) -> &Gate {
        &self.gate
    }

    fn read(&self, bytes: &[u8]) {
        let args: &[DrawArgs] = bytemuck::cast_slice(bytes);
        self.triangles
            .store(drawn_quads(args) * TRIANGLES_PER_QUAD, Ordering::Relaxed);
    }
}

pub(super) fn copy_args(
    terrain: &Terrain,
    triangles: &DrawnTriangles,
    encoder: &mut CommandEncoder,
) {
    if !triangles.0.gate.claim_copy() {
        return;
    }
    let size = terrain.frame.args_readback.size();
    encoder.copy_buffer_to_buffer(
        &terrain.frame.args,
        0,
        &terrain.frame.args_readback,
        0,
        size,
    );
}

pub(super) fn read_draw_args(terrain: Option<Res<Terrain>>, triangles: Res<DrawnTriangles>) {
    let Some(terrain) = terrain else {
        return;
    };
    if !triangles.0.gate.claim_map() {
        return;
    }
    readback::map(terrain.frame.args_readback.clone(), triangles.0.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_triangle_count_is_two_per_drawn_quad() {
        let counted = Counted::default();
        let mut args = args_reset();
        args[0].instance_count = 3;
        args[1].instance_count = 5;
        counted.read(bytemuck::cast_slice(&args));
        assert_eq!(counted.triangles.load(Ordering::Relaxed), 16);
    }

    #[test]
    fn an_ordered_draw_counts_its_survivors_and_not_the_holes_it_spans() {
        let counted = Counted::default();
        let mut args = args_reset();
        let ordered = (0..STREAMS)
            .find(|&stream| LayerGroup::of(stream as u32).culls_in_order())
            .expect("a blended stream");
        args[ordered].instance_count = 900;
        args[STREAMS + ordered].instance_count = 7;
        args[STREAMS + ordered].first_instance = 100;
        counted.read(bytemuck::cast_slice(&args));
        assert_eq!(counted.triangles.load(Ordering::Relaxed), 14);
    }

    #[test]
    fn only_an_ordered_draw_starts_its_counter_at_the_top() {
        let args = args_reset();
        assert_eq!(args.len(), STREAMS * 2);
        for stream in 0..STREAMS {
            assert_eq!(args[stream].first_instance, 0);
            let ordered = LayerGroup::of(stream as u32).culls_in_order();
            assert_eq!(args[STREAMS + stream].first_instance == NO_SLOT, ordered);
        }
    }
}
