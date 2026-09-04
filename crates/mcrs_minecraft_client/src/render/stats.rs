use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::render_resource::*;

use crate::mesh::STREAMS;
use crate::readback::{self, Gate, Reader};

use super::terrain::Terrain;

pub(super) const DRAW_ARGS_SIZE: u64 = size_of::<DrawArgs>() as u64;

/// Quads are drawn as two triangles each, one draw and no instancing: the cull adds six
/// vertices per surviving quad and the vertex shader finds its quad and corner in the
/// vertex index.
pub(super) const VERTICES_PER_QUAD: u32 = 6;
const TRIANGLES_PER_QUAD: u32 = 2;

pub(super) type DrawArgs = DrawIndirectArgs;

pub(super) fn quad_list() -> DrawArgs {
    DrawArgs {
        instance_count: 1,
        ..default()
    }
}

/// The draw args as a frame starts: a list per stream for the first pass, one per stream for
/// the second, then a counter per stream.
pub(super) fn args_reset() -> Vec<DrawArgs> {
    (0..2 * STREAMS)
        .map(|_| quad_list())
        .chain((0..STREAMS).map(|_| DrawArgs::default()))
        .collect()
}

fn drawn_quads(args: &[DrawArgs]) -> u32 {
    (0..2 * STREAMS)
        .map(|draw| args[draw].vertex_count / VERTICES_PER_QUAD)
        .sum()
}

/// Quads the last frame's depth hid and this frame's did not revive.
fn hidden_quads(args: &[DrawArgs]) -> u32 {
    let hidden: u32 = (0..STREAMS)
        .map(|stream| args[2 * STREAMS + stream].vertex_count)
        .sum();
    let revived: u32 = (0..STREAMS)
        .map(|stream| args[STREAMS + stream].vertex_count / VERTICES_PER_QUAD)
        .sum();
    hidden.saturating_sub(revived)
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
        let args: &[DrawArgs] = bytemuck::cast_slice(bytes);
        self.triangles
            .store(drawn_quads(args) * TRIANGLES_PER_QUAD, Ordering::Relaxed);
        self.hidden
            .store(hidden_quads(args) * TRIANGLES_PER_QUAD, Ordering::Relaxed);
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
        args[0].vertex_count = 3 * VERTICES_PER_QUAD;
        args[1].vertex_count = 5 * VERTICES_PER_QUAD;
        counted.read(bytemuck::cast_slice(&args));
        assert_eq!(counted.triangles.load(Ordering::Relaxed), 16);
    }
}
