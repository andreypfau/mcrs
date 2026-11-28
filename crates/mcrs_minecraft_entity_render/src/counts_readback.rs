use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bevy::log::warn;
use wgpu::{Buffer, BufferUsages, CommandEncoder, Device};

use crate::entity_library::{MAX_ARRAYS, MAX_DEFINITIONS};
use crate::gpu_buffers::COUNTERS;

pub(crate) const COUNTER_VISIBLE: usize = 1;
pub(crate) const COUNTER_FACES: usize = 17;
pub(crate) const COUNTER_DROPPED: usize = 25;
pub(crate) const COUNTER_IMPOSTORS: usize = 26;
const RING: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntityFrameCounts {
    pub full_rigs: u32,
    pub impostors: u32,
    pub faces: u32,
    pub dropped: u32,
}

impl EntityFrameCounts {
    #[must_use]
    pub fn of(counters: &[u32]) -> Self {
        let sum = |first: usize, count: usize| -> u32 {
            counters
                .iter()
                .skip(first)
                .take(count)
                .fold(0u32, |total, value| total.saturating_add(*value))
        };
        Self {
            full_rigs: sum(COUNTER_VISIBLE, MAX_DEFINITIONS),
            impostors: counters.get(COUNTER_IMPOSTORS).copied().unwrap_or(0),
            faces: sum(COUNTER_FACES, MAX_ARRAYS),
            dropped: counters.get(COUNTER_DROPPED).copied().unwrap_or(0),
        }
    }
}

const MAP_PENDING: u8 = 0;
const MAP_DONE: u8 = 1;
const MAP_FAILED: u8 = 2;

enum Stage {
    Free,
    Copied,
    Mapping(Arc<AtomicU8>),
}

pub(crate) struct CountsReadback {
    slots: Vec<(Buffer, Stage)>,
}

impl CountsReadback {
    pub(crate) fn new(device: &Device) -> Self {
        let slots = (0..RING)
            .map(|_| {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("entity counts readback"),
                    size: COUNTERS * 4,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                (buffer, Stage::Free)
            })
            .collect();
        Self { slots }
    }

    pub(crate) fn encode(&mut self, counters: &Buffer, encoder: &mut CommandEncoder) {
        if let Some((buffer, stage)) = self
            .slots
            .iter_mut()
            .find(|(_, stage)| matches!(stage, Stage::Free))
        {
            encoder.copy_buffer_to_buffer(counters, 0, buffer, 0, COUNTERS * 4);
            *stage = Stage::Copied;
        }
    }

    pub(crate) fn take(&mut self) -> Option<EntityFrameCounts> {
        let mut latest = None;
        for (buffer, stage) in &mut self.slots {
            match stage {
                Stage::Free => {}
                Stage::Copied => {
                    let state = Arc::new(AtomicU8::new(MAP_PENDING));
                    let reported = Arc::clone(&state);
                    buffer
                        .slice(..)
                        .map_async(wgpu::MapMode::Read, move |result| {
                            let value = if result.is_ok() { MAP_DONE } else { MAP_FAILED };
                            reported.store(value, Ordering::Release);
                        });
                    *stage = Stage::Mapping(state);
                }
                Stage::Mapping(state) => match state.load(Ordering::Acquire) {
                    MAP_PENDING => {}
                    MAP_DONE => {
                        match buffer.slice(..).get_mapped_range() {
                            Ok(view) => {
                                latest = Some(EntityFrameCounts::of(bytemuck::cast_slice(&view)));
                            }
                            Err(error) => warn!("readback of the entity counts failed: {error}"),
                        }
                        buffer.unmap();
                        *stage = Stage::Free;
                    }
                    _ => *stage = Stage::Free,
                },
            }
        }
        latest
    }
}
