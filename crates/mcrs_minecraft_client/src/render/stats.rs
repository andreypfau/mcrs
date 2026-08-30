use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::render_resource::*;

use crate::mesh::STREAMS;
use crate::readback::{self, Gate, Reader};

use super::terrain::Terrain;

pub(super) const DRAW_ARGS_SIZE: u64 = size_of::<DrawArgs>() as u64;

const VERTICES_PER_QUAD: u32 = 4;
const TRIANGLES_PER_QUAD: u32 = 2;

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(super) struct DrawArgs {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

impl DrawArgs {
    pub(super) fn quad_strip() -> Self {
        Self {
            vertex_count: VERTICES_PER_QUAD,
            instance_count: 0,
            first_vertex: 0,
            first_instance: 0,
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct DrawnTriangles(Arc<Counted>);

impl DrawnTriangles {
    pub fn get(&self) -> u32 {
        self.0.triangles.load(Ordering::Relaxed)
    }

    /// Quads a bucket culled in but had no room for in its share of the visible list.
    pub fn dropped(&self) -> u32 {
        self.0.dropped.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub struct Counted {
    triangles: AtomicU32,
    dropped: AtomicU32,
    limits: [AtomicU32; STREAMS],
    gate: Gate,
}

impl Reader for Counted {
    fn gate(&self) -> &Gate {
        &self.gate
    }

    fn read(&self, bytes: &[u8]) {
        let args: &[DrawArgs] = bytemuck::cast_slice(bytes);
        let mut drawn = 0u32;
        let mut dropped = 0u32;
        for (arg, limit) in args.iter().zip(&self.limits) {
            let limit = limit.load(Ordering::Relaxed);
            drawn += arg.instance_count.min(limit);
            dropped += arg.instance_count.saturating_sub(limit);
        }
        self.triangles
            .store(drawn * TRIANGLES_PER_QUAD, Ordering::Relaxed);
        self.dropped.store(dropped, Ordering::Relaxed);
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
    for (stream, limit) in triangles.0.limits.iter().enumerate() {
        let held = terrain.list.limits.get(stream).copied().unwrap_or(0);
        limit.store(held, Ordering::Relaxed);
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
        for limit in &counted.limits {
            limit.store(u32::MAX, Ordering::Relaxed);
        }
        let args = [
            DrawArgs {
                instance_count: 3,
                ..DrawArgs::quad_strip()
            },
            DrawArgs {
                instance_count: 5,
                ..DrawArgs::quad_strip()
            },
        ];
        counted.read(bytemuck::cast_slice(&args));
        assert_eq!(counted.triangles.load(Ordering::Relaxed), 16);
        assert_eq!(counted.dropped.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_bucket_culling_in_more_than_its_share_reports_the_rest_dropped() {
        let counted = Counted::default();
        counted.limits[0].store(4, Ordering::Relaxed);
        counted.limits[1].store(8, Ordering::Relaxed);
        let args = [
            DrawArgs {
                instance_count: 7,
                ..DrawArgs::quad_strip()
            },
            DrawArgs {
                instance_count: 5,
                ..DrawArgs::quad_strip()
            },
        ];
        counted.read(bytemuck::cast_slice(&args));
        assert_eq!(counted.triangles.load(Ordering::Relaxed), 18);
        assert_eq!(counted.dropped.load(Ordering::Relaxed), 3);
    }
}
