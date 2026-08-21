use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use wgpu::{
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePassTimestampWrites, QuerySet,
    QuerySetDescriptor, QueryType, RenderPassTimestampWrites, QUERY_RESOLVE_BUFFER_ALIGNMENT,
};

pub const CULL: usize = 0;
pub const TERRAIN: usize = 1;
pub const NAMES: [&str; 2] = ["cull", "terrain"];
pub const SLOTS: usize = NAMES.len();

const WINDOW: usize = 256;

const IMPLAUSIBLE_NS: u64 = 1_000_000_000;

const RING: u32 = 3;

const IDLE: u8 = 0;
const COPIED: u8 = 1;
const MAPPING: u8 = 2;

#[derive(Resource, Clone, Default)]
pub struct GpuTimings(Arc<Shared>);

impl GpuTimings {
    pub fn median(&self, slot: usize) -> Option<f32> {
        let samples = self.0.samples.lock().ok()?;
        let held = samples.written.min(WINDOW);
        if held == 0 {
            return None;
        }
        let mut sorted = [0.0f32; WINDOW];
        sorted[..held].copy_from_slice(&samples.ms[slot][..held]);
        sorted[..held].sort_unstable_by(f32::total_cmp);
        Some(sorted[held / 2])
    }

    fn writing(&self) -> u32 {
        self.0.frame.load(Ordering::Relaxed) % RING * SLOTS as u32 * 2
    }

    fn resolving(&self) -> u32 {
        (self.0.frame.load(Ordering::Relaxed) + 1) % RING * SLOTS as u32 * 2
    }

    fn push(&self, ms: [f32; SLOTS]) {
        let Ok(mut samples) = self.0.samples.lock() else {
            return;
        };
        let slot = samples.written % WINDOW;
        for (pass, value) in ms.iter().enumerate() {
            samples.ms[pass][slot] = *value;
        }
        samples.written += 1;
    }
}

#[derive(Default)]
struct Shared {
    frame: AtomicU32,
    samples: Mutex<Samples>,
    state: AtomicU8,
}

struct Samples {
    ms: [[f32; WINDOW]; SLOTS],
    written: usize,
}

impl Default for Samples {
    fn default() -> Self {
        Self {
            ms: [[0.0; WINDOW]; SLOTS],
            written: 0,
        }
    }
}

#[derive(Resource)]
pub struct Queries {
    set: QuerySet,
    resolve: Buffer,
    readback: Buffer,
    period_ns: f32,
}

impl Queries {
    pub fn render(&self, slot: usize, timings: &GpuTimings) -> RenderPassTimestampWrites<'_> {
        let first = timings.writing() + slot as u32 * 2;
        RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }

    pub fn compute(&self, slot: usize, timings: &GpuTimings) -> ComputePassTimestampWrites<'_> {
        let first = timings.writing() + slot as u32 * 2;
        ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }
}

const TIMESTAMP_BYTES: u64 = 8;
const RESOLVE_BYTES: u64 = QUERY_RESOLVE_BUFFER_ALIGNMENT;

pub fn init(mut commands: Commands, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
    let overlay_times_encoders =
        std::env::var("MTL_HUD_ENCODER_TIMING_ENABLED").is_ok_and(|on| on != "0");
    if overlay_times_encoders || std::env::var("ANVIL_PROBE").is_ok_and(|on| on == "0") {
        return;
    }
    if !device.features().contains(WgpuFeatures::TIMESTAMP_QUERY) {
        warn!("this device does not time passes, so the per-pass figures stay blank");
        return;
    }
    let set = device.wgpu_device().create_query_set(&QuerySetDescriptor {
        label: Some("pass timings"),
        ty: QueryType::Timestamp,
        count: SLOTS as u32 * 2 * RING,
    });
    let resolve = device.create_buffer(&BufferDescriptor {
        label: Some("pass timings resolve"),
        size: RESOLVE_BYTES,
        usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("pass timings readback"),
        size: RESOLVE_BYTES,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut priming = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("prime pass timings"),
    });
    for pair in 0..SLOTS as u32 * RING {
        priming.begin_compute_pass(&ComputePassDescriptor {
            label: Some("prime pass timings"),
            timestamp_writes: Some(ComputePassTimestampWrites {
                query_set: &set,
                beginning_of_pass_write_index: Some(pair * 2),
                end_of_pass_write_index: Some(pair * 2 + 1),
            }),
        });
    }
    queue.submit([priming.finish()]);

    commands.insert_resource(Queries {
        set,
        resolve,
        readback,
        period_ns: queue.get_timestamp_period(),
    });
}

pub fn resolve(queries: Option<Res<Queries>>, timings: Res<GpuTimings>, mut ctx: RenderContext) {
    let Some(queries) = queries else {
        return;
    };
    timings.0.frame.fetch_add(1, Ordering::Relaxed);
    if timings
        .0
        .state
        .compare_exchange(IDLE, COPIED, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let first = timings.resolving();
    let encoder = ctx.command_encoder();
    encoder.resolve_query_set(
        &queries.set,
        first..first + SLOTS as u32 * 2,
        &queries.resolve,
        0,
    );
    encoder.copy_buffer_to_buffer(&queries.resolve, 0, &queries.readback, 0, RESOLVE_BYTES);
}

pub fn read(queries: Option<Res<Queries>>, timings: Res<GpuTimings>) {
    let Some(queries) = queries else {
        return;
    };
    if timings
        .0
        .state
        .compare_exchange(COPIED, MAPPING, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }

    let buffer = queries.readback.clone();
    let period = queries.period_ns;
    let timings = timings.clone();
    buffer.clone().slice(..).map_async(MapMode::Read, move |result| {
        if result.is_ok() {
            let view = buffer.slice(..).get_mapped_range();
            let ticks: &[u64] = bytemuck::cast_slice(&view[..(SLOTS * 2) as usize * TIMESTAMP_BYTES as usize]);
            let ms: [Option<f32>; SLOTS] =
                std::array::from_fn(|slot| elapsed_ms(ticks[slot * 2], ticks[slot * 2 + 1], period));
            if let Some(ms) = ms.iter().copied().collect::<Option<Vec<_>>>() {
                timings.push(std::array::from_fn(|slot| ms[slot]));
            }
            drop(view);
            buffer.unmap();
        }
        timings.0.state.store(IDLE, Ordering::Relaxed);
    });
}

fn elapsed_ms(begin: u64, end: u64, period_ns: f32) -> Option<f32> {
    let ticks = end.saturating_sub(begin);
    if end <= begin || (ticks as f64 * period_ns as f64) > IMPLAUSIBLE_NS as f64 {
        return None;
    }
    Some((ticks as f64 * period_ns as f64 / 1e6) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_is_the_ticks_between_its_ends_in_milliseconds() {
        assert_eq!(elapsed_ms(1_000, 2_500_000, 1.0), Some(2.499));
    }

    #[test]
    fn a_slot_the_gpu_never_wrote_has_no_answer() {
        assert_eq!(elapsed_ms(u64::MAX, u64::MAX, 1.0), None);
        assert_eq!(elapsed_ms(500, 100, 1.0), None);
        assert_eq!(elapsed_ms(0, u64::MAX, 1.0), None);
    }

    #[test]
    fn the_ring_keeps_what_is_written_and_what_is_read_apart() {
        let timings = GpuTimings::default();
        let mut seen = Vec::new();
        for _ in 0..RING * 2 {
            seen.push((timings.writing(), timings.resolving()));
            timings.0.frame.fetch_add(1, Ordering::Relaxed);
        }
        let stride = SLOTS as u32 * 2;
        for (writing, resolving) in seen {
            assert_ne!(writing, resolving, "a frame must not read the entry it writes");
            assert!(writing < stride * RING && resolving < stride * RING);
        }
    }

    #[test]
    fn the_median_ignores_the_one_frame_that_stalled() {
        let timings = GpuTimings::default();
        for _ in 0..8 {
            timings.push([1.0, 4.0]);
        }
        timings.push([1.0, 400.0]);
        assert_eq!(timings.median(CULL), Some(1.0));
        assert_eq!(timings.median(TERRAIN), Some(4.0));
    }

    #[test]
    fn a_pass_with_no_frames_behind_it_reports_nothing() {
        assert_eq!(GpuTimings::default().median(CULL), None);
    }
}
