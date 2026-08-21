use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::render_resource::*;

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
        let quads = args.iter().map(|arg| arg.instance_count).sum::<u32>();
        self.triangles
            .store(quads * TRIANGLES_PER_QUAD, Ordering::Relaxed);
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
    encoder.copy_buffer_to_buffer(&terrain.frame.args, 0, &terrain.frame.args_readback, 0, size);
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
    }
}
