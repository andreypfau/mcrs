use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use wgpu::{
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePassTimestampWrites,
    QUERY_RESOLVE_BUFFER_ALIGNMENT, QuerySet, QuerySetDescriptor, QueryType,
    RenderPassTimestampWrites,
};

use crate::readback::{self, Gate, Reader};

pub const CULL: usize = 0;
pub const WORLD: usize = 1;
pub const HEAT: usize = 2;
pub const HIZ: usize = 3;
pub const CULL_SECOND: usize = 4;
pub const WORLD_SECOND: usize = 5;
pub const NAMES: [&str; 6] = [
    "cull",
    "world",
    "heat",
    "hiz",
    "cull second",
    "world second",
];
pub const SLOTS: usize = NAMES.len();

const WINDOW: usize = 256;

const IMPLAUSIBLE_NS: u64 = 1_000_000_000;

const RING: u32 = 3;

#[derive(Resource, Clone, Default)]
pub struct GpuTimings(Arc<Shared>);

impl GpuTimings {
    pub fn median(&self, slot: usize) -> Option<f32> {
        self.0.samples.lock().ok()?.median(slot)
    }

    fn writing(&self) -> u32 {
        self.0.frame.load(Ordering::Relaxed) % RING * SLOTS as u32 * 2
    }

    fn resolving(&self) -> u32 {
        (self.0.frame.load(Ordering::Relaxed) + 1) % RING * SLOTS as u32 * 2
    }

    #[cfg(test)]
    fn push(&self, ms: [f32; SLOTS]) {
        self.0.push(ms.map(Some));
    }
}

#[derive(Default)]
struct Shared {
    frame: AtomicU32,
    samples: Mutex<Samples<SLOTS, WINDOW>>,
    gate: Gate,
    period_ns: AtomicU32,
}

impl Shared {
    /// Metal writes no timestamp around a render pass, so a slot can stay blank forever while
    /// its neighbour resolves every frame; one silent pass must not blind the others.
    fn push(&self, ms: [Option<f32>; SLOTS]) {
        let Ok(mut samples) = self.samples.lock() else {
            return;
        };
        for (pass, value) in ms.iter().enumerate() {
            if let Some(value) = value {
                samples.push(pass, *value);
            }
        }
    }
}

impl Reader for Shared {
    fn gate(&self) -> &Gate {
        &self.gate
    }

    fn read(&self, bytes: &[u8]) {
        let period = f32::from_bits(self.period_ns.load(Ordering::Relaxed));
        let ticks: &[u64] = bytemuck::cast_slice(&bytes[..SLOTS * 2 * TIMESTAMP_BYTES as usize]);
        let ms: [Option<f32>; SLOTS] =
            std::array::from_fn(|slot| elapsed_ms(ticks[slot * 2], ticks[slot * 2 + 1], period));
        self.push(ms);
    }
}

struct Samples<const N: usize, const W: usize> {
    ms: Vec<[f32; W]>,
    written: [usize; N],
}

impl<const N: usize, const W: usize> Default for Samples<N, W> {
    fn default() -> Self {
        Self {
            ms: vec![[0.0; W]; N],
            written: [0; N],
        }
    }
}

impl<const N: usize, const W: usize> Samples<N, W> {
    fn push(&mut self, slot: usize, ms: f32) {
        let at = self.written[slot] % W;
        self.ms[slot][at] = ms;
        self.written[slot] += 1;
    }

    fn last(&self, slot: usize) -> f32 {
        match self.written[slot] {
            0 => 0.0,
            n => self.ms[slot][(n - 1) % W],
        }
    }

    fn median(&self, slot: usize) -> Option<f32> {
        self.percentiles(slot, W, &[0.5]).map(|p| p[0])
    }

    /// Quantiles over the newest `last` samples, each clamped to the last sample held.
    fn percentiles<const Q: usize>(
        &self,
        slot: usize,
        last: usize,
        quantiles: &[f32; Q],
    ) -> Option<[f32; Q]> {
        let held = self.held(slot).min(last);
        if held == 0 {
            return None;
        }
        let end = self.written[slot];
        let mut scratch: Vec<f32> = (end - held..end).map(|i| self.ms[slot][i % W]).collect();
        Some(quantiles.map(|q| {
            let at = ((held as f32 * q) as usize).min(held - 1);
            *scratch.select_nth_unstable_by(at, f32::total_cmp).1
        }))
    }

    fn held(&self, slot: usize) -> usize {
        self.written[slot].min(W)
    }
}

pub const MAIN: usize = 0;
pub const EXTRACT: usize = 1;
pub const PREPARE: usize = 2;
pub const RENDER: usize = 3;
pub const CLEANUP: usize = 4;
pub const ACQUIRE: usize = 5;
/// The frame's own work: every stage added up, the acquire wait taken back out.
pub const ENGINE: usize = 6;
pub const CPU_NAMES: [&str; 7] = [
    "main", "extract", "prepare", "render", "cleanup", "acquire", "engine",
];
pub const CPU_SLOTS: usize = CPU_NAMES.len();

/// Room for a second of frames at the rate being aimed for.
pub const CPU_WINDOW: usize = 4096;
/// A figure stands on the frames of the last second, which is how vanilla counts its fps.
pub const WINDOW_SECS: f32 = 1.0;

const FRAME_START: usize = 0;
const MAIN_END: usize = 1;
const RENDER_LAST: usize = 2;
const ACQUIRE_START: usize = 3;
const MARKS: usize = 4;

/// Wall time of each stage of the frame, main world and render world alike, as medians over the
/// same window the GPU slots use. Without pipelined rendering the stages run back to back, so
/// they and the frame period add up; with it the extract slot also holds the wait for the render
/// thread and the stages overlap instead of summing.
#[derive(Resource, Clone, Default)]
pub struct CpuTimings(Arc<CpuShared>);

#[derive(Default)]
struct CpuShared {
    marks: Mutex<[Option<Instant>; MARKS]>,
    samples: Mutex<Samples<CPU_SLOTS, CPU_WINDOW>>,
    starts: Mutex<FrameStarts>,
}

struct FrameStarts {
    at: Vec<Instant>,
    written: usize,
}

impl Default for FrameStarts {
    fn default() -> Self {
        Self {
            at: vec![Instant::now(); CPU_WINDOW],
            written: 0,
        }
    }
}

impl FrameStarts {
    fn push(&mut self, at: Instant) {
        self.at[self.written % CPU_WINDOW] = at;
        self.written += 1;
    }

    /// How many of the newest frames began within the window.
    fn recent(&self, now: Instant) -> usize {
        let held = self.written.min(CPU_WINDOW);
        (1..=held)
            .take_while(|&back| {
                let at = self.at[(self.written - back) % CPU_WINDOW];
                now.saturating_duration_since(at).as_secs_f32() <= WINDOW_SECS
            })
            .count()
    }
}

pub struct Spread {
    pub median: f32,
    pub p99: f32,
    pub max: f32,
    pub frames: usize,
}

impl CpuTimings {
    pub fn median(&self, slot: usize) -> Option<f32> {
        let recent = self.0.starts.lock().ok()?.recent(Instant::now());
        self.0
            .samples
            .lock()
            .ok()?
            .percentiles(slot, recent, &[0.5])
            .map(|p| p[0])
    }

    pub fn spread(&self, slot: usize) -> Option<Spread> {
        let recent = self.0.starts.lock().ok()?.recent(Instant::now());
        let samples = self.0.samples.lock().ok()?;
        let [median, p99, max] = samples.percentiles(slot, recent, &[0.5, 0.99, 1.0])?;
        Some(Spread {
            median,
            p99,
            max,
            frames: samples.held(slot).min(recent),
        })
    }

    fn lap(&self, from: usize, to: usize, slot: usize) {
        let now = Instant::now();
        let Ok(mut marks) = self.0.marks.lock() else {
            return;
        };
        if let Some(from) = marks[from]
            && let Ok(mut samples) = self.0.samples.lock()
        {
            samples.push(
                slot,
                now.saturating_duration_since(from).as_secs_f32() * 1e3,
            );
        }
        marks[to] = Some(now);
    }
}

pub fn frame_started(time: Res<Time<Real>>, cpu: Res<CpuTimings>) {
    if let Ok(mut marks) = cpu.0.marks.lock() {
        marks[FRAME_START] = time.last_update();
    }
    if let Some(at) = time.last_update()
        && let Ok(mut starts) = cpu.0.starts.lock()
    {
        starts.push(at);
    }
}

pub fn main_ended(cpu: Res<CpuTimings>) {
    cpu.lap(FRAME_START, MAIN_END, MAIN);
}

pub fn extracted(cpu: Res<CpuTimings>) {
    cpu.lap(MAIN_END, RENDER_LAST, EXTRACT);
}

pub fn prepared(cpu: Res<CpuTimings>) {
    cpu.lap(RENDER_LAST, RENDER_LAST, PREPARE);
}

/// The swapchain acquire blocks until the display hands a drawable back, so on a machine whose
/// presentation is throttled this is the wait that hides inside the prepare stage.
pub fn acquiring(cpu: Res<CpuTimings>) {
    if let Ok(mut marks) = cpu.0.marks.lock() {
        marks[ACQUIRE_START] = Some(Instant::now());
    }
}

pub fn acquired(cpu: Res<CpuTimings>) {
    cpu.lap(ACQUIRE_START, ACQUIRE_START, ACQUIRE);
}

pub fn rendered(cpu: Res<CpuTimings>) {
    cpu.lap(RENDER_LAST, RENDER_LAST, RENDER);
}

pub fn cleaned(cpu: Res<CpuTimings>) {
    cpu.lap(RENDER_LAST, RENDER_LAST, CLEANUP);
    if let Ok(mut samples) = cpu.0.samples.lock() {
        let engine = [MAIN, EXTRACT, PREPARE, RENDER, CLEANUP]
            .iter()
            .map(|&slot| samples.last(slot))
            .sum::<f32>()
            - samples.last(ACQUIRE);
        samples.push(ENGINE, engine);
    }
}

/// Every system dispatched costs something whether or not it finds work, so the count per
/// schedule is a number the frame budget has to know.
pub fn log_system_counts(world: &mut World) {
    let schedules = world.resource::<Schedules>();
    let mut counts: Vec<(String, usize)> = schedules
        .iter()
        .map(|(label, schedule)| (format!("{label:?}"), schedule.systems_len()))
        .filter(|(_, count)| *count > 0)
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let total: usize = counts.iter().map(|(_, count)| count).sum();
    info!(total, ?counts, "systems per schedule");
}

#[derive(Resource)]
pub struct Queries {
    set: QuerySet,
    resolve: Buffer,
    readback: Buffer,
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

pub fn init(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    timings: Res<GpuTimings>,
) {
    let overlay_times_encoders =
        std::env::var("MTL_HUD_ENCODER_TIMING_ENABLED").is_ok_and(|on| on != "0");
    if overlay_times_encoders || std::env::var("MCRS_PROBE").is_ok_and(|on| on == "0") {
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

    timings
        .0
        .period_ns
        .store(queue.get_timestamp_period().to_bits(), Ordering::Relaxed);
    commands.insert_resource(Queries {
        set,
        resolve,
        readback,
    });
}

pub fn resolve(queries: Option<&Queries>, timings: &GpuTimings, encoder: &mut CommandEncoder) {
    let Some(queries) = queries else {
        return;
    };
    timings.0.frame.fetch_add(1, Ordering::Relaxed);
    if !timings.0.gate.claim_copy() {
        return;
    }
    let first = timings.resolving();
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
    if !timings.0.gate.claim_map() {
        return;
    }
    readback::map(queries.readback.clone(), timings.0.clone());
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
            assert_ne!(
                writing, resolving,
                "a frame must not read the entry it writes"
            );
            assert!(writing < stride * RING && resolving < stride * RING);
        }
    }

    #[test]
    fn the_median_ignores_the_one_frame_that_stalled() {
        let timings = GpuTimings::default();
        for _ in 0..8 {
            timings.push([1.0, 4.0, 0.5, 0.1, 0.2, 0.3]);
        }
        timings.push([1.0, 400.0, 0.5, 0.1, 0.2, 0.3]);
        assert_eq!(timings.median(CULL), Some(1.0));
        assert_eq!(timings.median(WORLD), Some(4.0));
    }

    #[test]
    fn a_pass_the_gpu_never_timed_leaves_the_others_readable() {
        let shared = Shared::default();
        shared.period_ns.store(1.0f32.to_bits(), Ordering::Relaxed);
        let ticks: [u64; SLOTS * 2] = [0, 2_000_000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        shared.read(bytemuck::cast_slice(&ticks));
        let timings = GpuTimings(Arc::new(shared));
        assert_eq!(timings.median(CULL), Some(2.0));
        assert_eq!(timings.median(WORLD), None);
    }

    #[test]
    fn a_pass_with_no_frames_behind_it_reports_nothing() {
        assert_eq!(GpuTimings::default().median(CULL), None);
    }

    #[test]
    fn a_resolved_frame_lands_in_the_window_as_milliseconds() {
        let shared = Shared::default();
        shared.period_ns.store(1.0f32.to_bits(), Ordering::Relaxed);
        let ticks: [u64; SLOTS * 2] = [0, 1_000_000, 0, 4_000_000, 0, 0, 0, 0, 0, 0, 0, 0];
        shared.read(bytemuck::cast_slice(&ticks));
        let timings = GpuTimings(Arc::new(shared));
        assert_eq!(timings.median(CULL), Some(1.0));
        assert_eq!(timings.median(WORLD), Some(4.0));
    }
}
