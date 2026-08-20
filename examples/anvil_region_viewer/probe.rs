//! What each pass costs the GPU, taken at the boundaries of the pass itself.
//!
//! Apple's GPUs sample counters only where a pass begins and ends, never at a draw or a dispatch
//! inside one. Timestamps written from inside an encoder are therefore dropped, which is what
//! bevy's own diagnostics do, and every GPU figure it reports comes out zero. Asking for the same
//! two samples through `timestamp_writes` on the pass descriptor is the one route Metal honours.

use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use wgpu::{
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePassTimestampWrites, QuerySet,
    QuerySetDescriptor, QueryType, RenderPassTimestampWrites, QUERY_RESOLVE_BUFFER_ALIGNMENT,
};

/// The passes that are timed, in the order their pair of timestamps sits in the query set.
pub const CULL: usize = 0;
pub const TERRAIN: usize = 1;
pub const NAMES: [&str; 2] = ["cull", "terrain"];
pub const SLOTS: usize = NAMES.len();

/// Frames held per pass. Wide enough that a second of a fast frame rate fits, which is the span
/// the readout covers.
const WINDOW: usize = 256;

/// A pass that reads longer than this did not take that long: it is a slot the GPU never wrote.
const IMPLAUSIBLE_NS: u64 = 1_000_000_000;

/// Frames the query set holds at once, so a resolve only ever reads one the GPU has long finished.
///
/// A render pass is not one thing on the GPU's timeline: on a tile-based one its opening sample is
/// taken as the vertex stage starts and its closing sample as the fragment stage ends, with other
/// work running in between. A resolve recorded either side of the pass therefore still lands in the
/// middle of it, and the span reads backwards. Reading a frame two behind cannot.
const RING: u32 = 3;

const IDLE: u8 = 0;
const COPIED: u8 = 1;
const MAPPING: u8 = 2;

/// Milliseconds each pass took, shared with the main world through an `Arc` rather than extracted:
/// the figures only exist once the GPU has run, which is the wrong side of the extract boundary.
#[derive(Resource, Clone, Default)]
pub struct GpuTimings(Arc<Shared>);

impl GpuTimings {
    /// The median of the frames held for one pass, or nothing while none have landed.
    ///
    /// A median rather than a mean because a single frame that stalls on an upload would otherwise
    /// move a figure that is meant to describe the steady state.
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

    /// First timestamp index of the ring entry this frame's passes write into.
    fn writing(&self) -> u32 {
        self.0.frame.load(Ordering::Relaxed) % RING * SLOTS as u32 * 2
    }

    /// The entry a resolve may read: the oldest, written `RING - 1` frames ago and long finished.
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
    /// Which entry of the ring the passes write this frame.
    frame: AtomicU32,
    samples: Mutex<Samples>,
    /// A resolve is recorded only once the previous result has landed, so the staging buffer is
    /// never both mapped and the destination of a copy, which the backend rejects.
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

/// The query set the passes write into and the pair of buffers that carry it back.
#[derive(Resource)]
pub struct Queries {
    set: QuerySet,
    resolve: Buffer,
    readback: Buffer,
    /// Nanoseconds one tick of the GPU clock stands for.
    period_ns: f32,
}

impl Queries {
    /// Where a render pass writes the two samples that bracket it.
    pub fn render(&self, slot: usize, timings: &GpuTimings) -> RenderPassTimestampWrites<'_> {
        let first = timings.writing() + slot as u32 * 2;
        RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }

    /// The same for a compute pass, which wgpu keeps as its own type.
    pub fn compute(&self, slot: usize, timings: &GpuTimings) -> ComputePassTimestampWrites<'_> {
        let first = timings.writing() + slot as u32 * 2;
        ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }
}

/// One timestamp is eight bytes, and a resolve has to land on an offset the backend rounds to.
const TIMESTAMP_BYTES: u64 = 8;
const RESOLVE_BYTES: u64 = QUERY_RESOLVE_BUFFER_ALIGNMENT;

pub fn init(mut commands: Commands, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
    // A pass carries a fixed number of counter slots, and Metal's own performance overlay wants
    // them for the same passes when it is asked to time encoders. Whichever asks first gets them,
    // and with these two taken the overlay can only reach the blits, which is not the breakdown
    // anyone turned it on for. So this stands aside when it is running, and `ANVIL_PROBE=0` says
    // the same by hand.
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
    // Vulkan's resolve waits for every query in its range to become available, and one no pass has
    // ever written never becomes available: the queue blocks until the driver's watchdog takes the
    // device down. Metal returns its error value instead, which is why only one of the two backends
    // needs this. An empty pass writes a pair without running anything, so one per pair leaves the
    // whole set readable before the first resolve.
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

/// Empties the query set into a buffer the CPU can reach, after every pass that writes into it has
/// been recorded.
pub fn resolve(queries: Option<Res<Queries>>, timings: Res<GpuTimings>, mut ctx: RenderContext) {
    let Some(queries) = queries else {
        return;
    };
    // Advanced whether or not a result is carried off this frame, so that what the passes write
    // and what a later resolve reads stay the agreed number of frames apart.
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

/// Maps what the resolve left behind. The figures are a frame or two late, which cannot show in a
/// readout redrawn once a second, and the map never blocks: the callback lands whenever the device
/// is next polled and the state only returns to idle then.
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

/// A pass that did not run leaves its two slots at whatever they last held, or at the error value
/// the backend fills them with. A span that runs backwards or absurdly long is one of those and
/// has no answer, which is not the same as an answer of no time.
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
