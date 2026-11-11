use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bevy::log::warn;
use bevy::render::render_resource::{Buffer, MapMode};

const IDLE: u8 = 0;
const COPIED: u8 = 1;
const MAPPING: u8 = 2;

#[derive(Default)]
pub struct Gate(AtomicU8);

impl Gate {
    pub fn claim_copy(&self) -> bool {
        self.step(IDLE, COPIED)
    }

    pub fn claim_map(&self) -> bool {
        self.step(COPIED, MAPPING)
    }

    fn step(&self, from: u8, to: u8) -> bool {
        self.0
            .compare_exchange(from, to, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }

    fn release(&self) {
        self.0.store(IDLE, Ordering::Relaxed);
    }
}

pub trait Reader: Send + Sync + 'static {
    fn gate(&self) -> &Gate;

    fn read(&self, bytes: &[u8]);
}

pub fn map(buffer: Buffer, owner: Arc<impl Reader>) {
    buffer
        .clone()
        .slice(..)
        .map_async(MapMode::Read, move |result| {
            if result.is_ok() {
                match buffer.slice(..).get_mapped_range() {
                    Ok(view) => owner.read(&view),
                    Err(error) => warn!("readback of a mapped buffer failed: {error}"),
                }
                buffer.unmap();
            }
            owner.gate().release();
        });
}
