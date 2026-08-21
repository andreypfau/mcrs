use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::render_resource::*;

use super::terrain::Terrain;

pub(super) const DRAW_ARGS_SIZE: u64 = size_of::<DrawArgs>() as u64;
pub(super) const INSTANCE_COUNT_OFFSET: u64 = 4;

#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(super) struct DrawArgs {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

#[derive(Resource, Clone, Default)]
pub struct DrawnTriangles(Arc<ArgsReadback>);

impl DrawnTriangles {
    pub fn get(&self) -> u32 {
        self.0.triangles.load(Ordering::Relaxed)
    }

    pub(super) fn claim(&self, from: u8, to: u8) -> bool {
        self.0
            .state
            .compare_exchange(from, to, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }
}

#[derive(Default)]
struct ArgsReadback {
    triangles: AtomicU32,
    state: AtomicU8,
}

impl ArgsReadback {
    const IDLE: u8 = 0;
    const COPIED: u8 = 1;
    const MAPPING: u8 = 2;
}

pub(super) fn copy_args(terrain: &Terrain, triangles: &DrawnTriangles, encoder: &mut CommandEncoder) {
    if !triangles.claim(ArgsReadback::IDLE, ArgsReadback::COPIED) {
        return;
    }
    let size = terrain.args_readback.size();
    encoder.copy_buffer_to_buffer(&terrain.args, 0, &terrain.args_readback, 0, size);
}

pub(super) fn read_draw_args(terrain: Option<Res<Terrain>>, triangles: Res<DrawnTriangles>) {
    let Some(terrain) = terrain else {
        return;
    };
    if !triangles.claim(ArgsReadback::COPIED, ArgsReadback::MAPPING) {
        return;
    }

    let buffer = terrain.args_readback.clone();
    let readback = triangles.0.clone();
    buffer.clone().slice(..).map_async(MapMode::Read, move |result| {
        if result.is_ok() {
            let view = buffer.slice(..).get_mapped_range();
            let args: &[DrawArgs] = bytemuck::cast_slice(&view);
            let count = args.iter().map(|arg| arg.instance_count).sum::<u32>() * 2;
            readback.triangles.store(count, Ordering::Relaxed);
            drop(view);
            buffer.unmap();
        }
        readback.state.store(ArgsReadback::IDLE, Ordering::Relaxed);
    });
}
