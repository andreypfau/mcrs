//! Prints what the adapter offers, then times kernels shaped like the entity passes: a copy for
//! memory bandwidth, FMA and sine loops for arithmetic, and cull, pose, expand and draw kernels
//! with the record sizes of `docs/entity-render.md`. The figures feed
//! `docs/entity-render-budget.md`.
//!
//! `cargo run --release -p mcrs_minecraft_render_probe --example entity_budget`
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::too_many_lines,
    reason = "a measurement example has no caller to return an error to, and each section reads top to bottom"
)]

use std::time::Instant;

use bevy::tasks::block_on;
use wgpu::*;

const RUNS: usize = 9;
const WIDTH: u32 = 2560;
const HEIGHT: u32 = 1440;
const GBUFFER: [TextureFormat; 3] = [
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba16Float,
    TextureFormat::Rgba16Float,
];

fn main() {
    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::PRIMARY,
        ..InstanceDescriptor::new_without_display_handle()
    });
    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no adapter");
    report_adapter(&adapter);

    let features = adapter.features() & Features::TIMESTAMP_QUERY;
    let limits = adapter.limits();
    let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("entity budget"),
        required_features: features,
        required_limits: limits.clone(),
        ..Default::default()
    }))
    .expect("no device");
    let gpu = Gpu::new(device, queue);
    println!(
        "\n# timing: {}",
        if gpu.timer.is_some() {
            "timestamp queries"
        } else {
            "wall clock"
        }
    );

    bandwidth(&gpu);
    arithmetic(&gpu);
    upload(&gpu);
    cpu_records();
    entity_passes(&gpu);
    raster(&gpu);
}

fn report_adapter(adapter: &Adapter) {
    let info = adapter.get_info();
    println!("# adapter\n{info:#?}");
    println!("\n# features\n{:#?}", adapter.features());
    let down = adapter.get_downlevel_capabilities();
    println!(
        "\n# downlevel\nflags: {:#?}\nshader model: {:?}",
        down.flags, down.shader_model
    );
    let l = adapter.limits();
    println!("\n# limits: adapter / wgpu defaults (WebGPU core) / downlevel / webgl2");
    let d = Limits::defaults();
    let dl = Limits::downlevel_defaults();
    let w = Limits::downlevel_webgl2_defaults();
    macro_rules! row {
        ($($f:ident),*) => {$(
            println!("{:<48} {:>14} {:>12} {:>12} {:>12}", stringify!($f), l.$f, d.$f, dl.$f, w.$f);
        )*};
    }
    row!(
        max_storage_buffers_per_shader_stage,
        max_storage_buffer_binding_size,
        max_buffer_size,
        max_uniform_buffer_binding_size,
        max_storage_textures_per_shader_stage,
        max_sampled_textures_per_shader_stage,
        max_texture_array_layers,
        max_texture_dimension_2d,
        max_texture_dimension_3d,
        max_bind_groups,
        max_bindings_per_bind_group,
        max_dynamic_storage_buffers_per_pipeline_layout,
        max_compute_workgroup_storage_size,
        max_compute_invocations_per_workgroup,
        max_compute_workgroup_size_x,
        max_compute_workgroups_per_dimension,
        max_color_attachments,
        max_color_attachment_bytes_per_sample,
        max_inter_stage_shader_variables,
        max_vertex_buffers,
        max_vertex_attributes,
        max_immediate_size,
        min_storage_buffer_offset_alignment,
        min_uniform_buffer_offset_alignment
    );
}

struct Timer {
    set: QuerySet,
    resolve: Buffer,
    read: Buffer,
    period: f32,
}

struct Gpu {
    device: Device,
    queue: Queue,
    timer: Option<Timer>,
    heat: (ComputePipeline, BindGroup),
}

impl Gpu {
    fn new(device: Device, queue: Queue) -> Self {
        let timer = device
            .features()
            .contains(Features::TIMESTAMP_QUERY)
            .then(|| Timer {
                set: device.create_query_set(&QuerySetDescriptor {
                    label: None,
                    ty: QueryType::Timestamp,
                    count: 2,
                }),
                resolve: device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                read: device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                period: queue.get_timestamp_period(),
            });
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(FMA.into()),
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let out = device.create_buffer(&BufferDescriptor {
            label: None,
            size: 16 << 20,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[BindGroupEntry {
                binding: 0,
                resource: out.as_entire_binding(),
            }],
        });
        let gpu = Self {
            device,
            queue,
            timer,
            heat: (pipeline, group),
        };
        let start = Instant::now();
        while start.elapsed().as_secs_f64() < 2.0 {
            gpu.heat();
            gpu.device.poll(PollType::wait_indefinitely()).unwrap();
        }
        gpu
    }

    /// The governor picks the lowest clock that meets the load it sees, so a timed pass with idle
    /// gaps around it reads its cost at a low clock. A burst of arithmetic queued ahead of each
    /// timed pass keeps the GPU at full clock.
    fn heat(&self) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.heat.0);
            pass.set_bind_group(0, &self.heat.1, &[]);
            pass.dispatch_workgroups(4096, 1, 1);
        }
        self.queue.submit([encoder.finish()]);
    }

    fn buffer(&self, size: u64, usage: BufferUsages) -> Buffer {
        self.device.create_buffer(&BufferDescriptor {
            label: None,
            size,
            usage: usage | BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn filled(&self, words: u64) -> Buffer {
        let buffer = self.buffer(words * 4, BufferUsages::empty());
        let chunk: Vec<u32> = (0..(1u32 << 20))
            .map(|i| i.wrapping_mul(2_654_435_761) >> 8)
            .collect();
        let bytes: &[u8] = bytemuck::cast_slice(&chunk);
        let mut offset = 0;
        while offset < words * 4 {
            let n = (bytes.len() as u64).min(words * 4 - offset);
            self.queue
                .write_buffer(&buffer, offset, &bytes[..n as usize]);
            offset += n;
        }
        self.queue.submit([]);
        buffer
    }

    fn uniform(&self, words: &[u32]) -> Buffer {
        let buffer = self.device.create_buffer(&BufferDescriptor {
            label: None,
            size: (words.len() as u64 * 4).max(16).next_multiple_of(16),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue
            .write_buffer(&buffer, 0, bytemuck::cast_slice(words));
        buffer
    }

    fn compute(&self, source: &str, constants: &[(&str, f64)]) -> ComputePipeline {
        let module = self.device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(source.into()),
        });
        self.device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
                cache: None,
            })
    }

    fn bind(&self, layout: &BindGroupLayout, buffers: &[&Buffer]) -> BindGroup {
        let entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(i, b)| BindGroupEntry {
                binding: i as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        })
    }

    /// Median and minimum time of one repetition over `RUNS` submissions, in milliseconds. A
    /// calibration run picks the repetition count that makes one submission about 25 ms long, so
    /// the GPU stays busy, its clock stays up, and the submit overhead disappears in the mean.
    fn time(&self, record: impl Fn(&mut CommandEncoder, Option<&QuerySet>, u32)) -> (f64, f64) {
        let (med, min, _) = self.time_both(record);
        (med, min)
    }

    /// Also returns the median wall time of one repetition. The timestamps at the bounds of a
    /// render pass on a tile-based GPU do not bracket the work of the pass, so a draw reads the
    /// wall time; with about 25 ms per submission the submit overhead is a few percent.
    fn time_both(
        &self,
        record: impl Fn(&mut CommandEncoder, Option<&QuerySet>, u32),
    ) -> (f64, f64, f64) {
        let once = self.run(&record, 1).1;
        let reps = ((25.0 / once.max(1e-3)).ceil() as u32).clamp(1, 4000);
        self.run(&record, reps);
        let runs: Vec<(f64, f64)> = (0..RUNS).map(|_| self.run(&record, reps)).collect();
        let mut gpu: Vec<f64> = runs.iter().map(|r| r.0 / reps as f64).collect();
        let mut wall: Vec<f64> = runs.iter().map(|r| r.1 / reps as f64).collect();
        gpu.sort_by(f64::total_cmp);
        wall.sort_by(f64::total_cmp);
        (gpu[gpu.len() / 2], gpu[0], wall[wall.len() / 2])
    }

    fn run(
        &self,
        record: &impl Fn(&mut CommandEncoder, Option<&QuerySet>, u32),
        reps: u32,
    ) -> (f64, f64) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        record(&mut encoder, self.timer.as_ref().map(|t| &t.set), reps);
        if let Some(t) = &self.timer {
            encoder.resolve_query_set(&t.set, 0..2, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, 16);
        }
        let start = Instant::now();
        self.queue.submit([encoder.finish()]);
        self.device.poll(PollType::wait_indefinitely()).unwrap();
        let wall = start.elapsed().as_secs_f64() * 1e3;
        let Some(t) = &self.timer else {
            return (wall, wall);
        };
        t.read.slice(..).map_async(MapMode::Read, |_| {});
        self.device.poll(PollType::wait_indefinitely()).unwrap();
        let ticks: [u64; 2] = bytemuck::cast_slice(&t.read.slice(..).get_mapped_range())[..2]
            .try_into()
            .unwrap();
        t.read.unmap();
        if ticks[1] > ticks[0] {
            ((ticks[1] - ticks[0]) as f64 * t.period as f64 * 1e-6, wall)
        } else {
            (wall, wall)
        }
    }

    fn time_dispatch(
        &self,
        pipeline: &ComputePipeline,
        groups: &[BindGroup],
        workgroups: u32,
    ) -> (f64, f64) {
        self.time(|encoder, set, reps| {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: None,
                timestamp_writes: set.map(|set| ComputePassTimestampWrites {
                    query_set: set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            pass.set_pipeline(pipeline);
            for (i, g) in groups.iter().enumerate() {
                pass.set_bind_group(i as u32, g, &[]);
            }
            for _ in 0..reps {
                pass.dispatch_workgroups(workgroups.min(65535), workgroups.div_ceil(65535), 1);
            }
        })
    }
}

const COPY: &str = r#"
@group(0) @binding(0) var<storage, read> src: array<vec4<u32>>;
@group(0) @binding(1) var<storage, read_write> dst: array<vec4<u32>>;
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) g: vec3<u32>, @builtin(num_workgroups) n: vec3<u32>) {
    let stride = n.x * 256u;
    for (var i = g.x; i < arrayLength(&src); i += stride) { dst[i] = src[i]; }
}
"#;

const READ: &str = r#"
@group(0) @binding(0) var<storage, read> src: array<vec4<u32>>;
@group(0) @binding(1) var<storage, read_write> dst: array<vec4<u32>>;
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) g: vec3<u32>, @builtin(num_workgroups) n: vec3<u32>) {
    let stride = n.x * 256u;
    var acc = vec4<u32>(0u);
    for (var i = g.x; i < arrayLength(&src); i += stride) { acc ^= src[i]; }
    if (all(acc == vec4<u32>(1u, 2u, 3u, 4u))) { dst[g.x] = acc; }
}
"#;

fn bandwidth(gpu: &Gpu) {
    println!("\n# memory bandwidth (256 MiB buffers)");
    let words = 64u64 << 20;
    let src = gpu.filled(words);
    let dst = gpu.buffer(words * 4, BufferUsages::empty());
    for (name, source, factor) in [("copy (read + write)", COPY, 2.0), ("read only", READ, 1.0)] {
        let pipeline = gpu.compute(source, &[]);
        let group = gpu.bind(&pipeline.get_bind_group_layout(0), &[&src, &dst]);
        let (med, min) = gpu.time_dispatch(&pipeline, &[group], 8192);
        let bytes = (words * 4) as f64 * factor;
        println!(
            "{name:<22} {med:8.3} ms median {min:8.3} min  {:7.1} GB/s median {:7.1} GB/s best",
            bytes / med * 1e-6,
            bytes / min * 1e-6
        );
    }
}

const FMA: &str = r#"
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    var a = vec4<f32>(f32(g.x)) * 1e-7;
    var b = a + 0.1; var c = a + 0.2; var d = a + 0.3;
    let m = vec4<f32>(0.9999); let k = vec4<f32>(1e-4);
    for (var i = 0u; i < 1024u; i++) { a = fma(a, m, k); b = fma(b, m, k); c = fma(c, m, k); d = fma(d, m, k); }
    out[g.x] = a + b + c + d;
}
"#;

const MAD: &str = r#"
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    var a = f32(g.x) * 1e-7;
    var b = a + 0.1; var c = a + 0.2; var d = a + 0.3;
    var e = a + 0.4; var f = a + 0.5; var h = a + 0.6; var j = a + 0.7;
    for (var i = 0u; i < 1024u; i++) {
        a = a * 0.9999 + 1e-4; b = b * 0.9999 + 1e-4; c = c * 0.9999 + 1e-4; d = d * 0.9999 + 1e-4;
        e = e * 0.9999 + 1e-4; f = f * 0.9999 + 1e-4; h = h * 0.9999 + 1e-4; j = j * 0.9999 + 1e-4;
    }
    out[g.x] = vec4(a + b, c + d, e + f, h + j);
}
"#;

const SIN: &str = r#"
@group(0) @binding(0) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    var a = vec4<f32>(f32(g.x)) * 1e-7;
    var b = a + 0.1;
    for (var i = 0u; i < 256u; i++) { a = sin(a) + 0.5; b = cos(b) + 0.25; }
    out[g.x] = a + b;
}
"#;

fn arithmetic(gpu: &Gpu) {
    println!("\n# arithmetic (1,048,576 lanes)");
    let lanes = 1u32 << 20;
    let out = gpu.buffer(lanes as u64 * 16, BufferUsages::empty());
    for (name, source, ops, unit) in [
        ("fma f32", FMA, 1024.0 * 16.0 * 2.0, "GFLOP/s"),
        ("scalar mul-add f32", MAD, 1024.0 * 8.0 * 2.0, "GFLOP/s"),
        ("sin + cos f32", SIN, 256.0 * 8.0, "G trig/s"),
    ] {
        let pipeline = gpu.compute(source, &[]);
        let group = gpu.bind(&pipeline.get_bind_group_layout(0), &[&out]);
        let (med, min) = gpu.time_dispatch(&pipeline, &[group], lanes / 256);
        let total = lanes as f64 * ops;
        println!(
            "{name:<22} {med:8.3} ms median {min:8.3} min  {:9.1} {unit} median {:9.1} best",
            total / med * 1e-6,
            total / min * 1e-6
        );
    }
}

fn upload(gpu: &Gpu) {
    println!("\n# CPU upload: queue.write_buffer + submit + wait, wall clock");
    for mb in [1u64, 6, 13, 26] {
        let bytes = mb << 20;
        let dst = gpu.buffer(bytes, BufferUsages::empty());
        let data = vec![7u8; bytes as usize];
        let mut cpu = Vec::new();
        let mut total = Vec::new();
        for _ in 0..RUNS {
            let start = Instant::now();
            gpu.queue.write_buffer(&dst, 0, &data);
            let written = start.elapsed().as_secs_f64() * 1e3;
            gpu.queue.submit([]);
            gpu.device.poll(PollType::wait_indefinitely()).unwrap();
            cpu.push(written);
            total.push(start.elapsed().as_secs_f64() * 1e3);
        }
        cpu.sort_by(f64::total_cmp);
        total.sort_by(f64::total_cmp);
        println!(
            "{mb:>3} MiB  write_buffer {:7.3} ms  with submit and wait {:7.3} ms  ({:6.1} GB/s)",
            cpu[RUNS / 2],
            total[RUNS / 2],
            bytes as f64 / total[RUNS / 2] * 1e-6
        );
    }
}

/// One thread packs the tick record of each moving mob into the CPU mirror of its slot and marks
/// the page dirty. Slots are a random permutation of the mobs, as the arena hands them out.
fn cpu_records() {
    println!("\n# CPU tick records: pack and scatter into the slot mirror, one thread");
    #[derive(Clone, Copy)]
    struct Mob {
        position: [f64; 3],
        yaw: [f32; 2],
        pitch: f32,
        walk: [f32; 2],
        slot: u32,
    }
    for n in [10_000usize, 100_000, 300_000] {
        let mut state = 0x9e37_79b9u32;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let mut slots: Vec<u32> = (0..n as u32).collect();
        for i in (1..n).rev() {
            slots.swap(i, next() as usize % (i + 1));
        }
        let mobs: Vec<Mob> = slots
            .iter()
            .map(|&slot| Mob {
                position: [next() as f64 * 1e-4, 64.5, next() as f64 * 1e-4],
                yaw: [0.5, 0.25],
                pitch: 0.1,
                walk: [12.0, 0.4],
                slot,
            })
            .collect();
        for (bytes, label) in [(64usize, "64 B record"), (32, "32 B record")] {
            let mut mirror = vec![0u8; n * bytes];
            let mut dirty = vec![0u64; n.div_ceil(64 * 64)];
            let mut times = Vec::new();
            for _ in 0..RUNS {
                let start = Instant::now();
                for mob in &mobs {
                    let block = mob.position.map(|v| v.floor());
                    let mut record = [0u8; 64];
                    for a in 0..3 {
                        record[a * 4..a * 4 + 4].copy_from_slice(&(block[a] as i32).to_le_bytes());
                        let frac = ((mob.position[a] - block[a]) * 65536.0) as u16;
                        record[12 + a * 2..14 + a * 2].copy_from_slice(&frac.to_le_bytes());
                    }
                    record[18..20].copy_from_slice(&((mob.yaw[0] * 65536.0) as u16).to_le_bytes());
                    record[20..22].copy_from_slice(&((mob.yaw[1] * 65536.0) as u16).to_le_bytes());
                    record[22..24].copy_from_slice(&((mob.pitch * 32767.0) as i16).to_le_bytes());
                    record[24..28].copy_from_slice(&mob.walk[0].to_le_bytes());
                    record[28..32].copy_from_slice(&mob.walk[1].to_le_bytes());
                    let at = mob.slot as usize * bytes;
                    mirror[at..at + bytes].copy_from_slice(&record[..bytes]);
                    let page = mob.slot as usize / 64;
                    dirty[page / 64] |= 1 << (page % 64);
                }
                times.push(start.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box((&mirror, &dirty));
            }
            times.sort_by(f64::total_cmp);
            println!(
                "mobs {n:>7} {label}, byte packing   {:7.3} ms median  {:6.2} ns/mob",
                times[RUNS / 2],
                times[RUNS / 2] * 1e6 / n as f64
            );
            if bytes != 32 {
                continue;
            }
            #[derive(Clone, Copy, Default)]
            #[repr(C)]
            struct Tick {
                block: [i32; 3],
                fraction: [u16; 3],
                yaw: [u16; 2],
                pitch: i16,
                walk: [f32; 2],
            }
            let mut mirror = vec![Tick::default(); n];
            let mut times = Vec::new();
            for _ in 0..RUNS {
                let start = Instant::now();
                for mob in &mobs {
                    let block = mob.position.map(|v| v.floor());
                    mirror[mob.slot as usize] = Tick {
                        block: block.map(|b| b as i32),
                        fraction: [0, 1, 2]
                            .map(|a| ((mob.position[a] - block[a]) * 65536.0) as u16),
                        yaw: mob.yaw.map(|y| (y * 65536.0) as u16),
                        pitch: (mob.pitch * 32767.0) as i16,
                        walk: mob.walk,
                    };
                    let page = mob.slot as usize / 64;
                    dirty[page / 64] |= 1 << (page % 64);
                }
                times.push(start.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box((&mirror, &dirty));
            }
            times.sort_by(f64::total_cmp);
            println!(
                "mobs {n:>7} {label}, struct store   {:7.3} ms median  {:6.2} ns/mob",
                times[RUNS / 2],
                times[RUNS / 2] * 1e6 / n as f64
            );
        }
    }
}

/// Lane per slot: tick record 64 B and the first 16 B of the state record are read, the frustum
/// and a depth-pyramid word are tested, and the slot is appended with a workgroup-aggregated atomic.
const CULL: &str = r#"
struct Params { count: u32, visible_per_mille: u32, pad0: u32, pad1: u32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> ticks: array<array<vec4<u32>, 4>>;
@group(0) @binding(2) var<storage, read> states: array<array<vec4<u32>, 3>>;
@group(0) @binding(3) var<storage, read> pyramid: array<f32>;
@group(0) @binding(4) var<storage, read_write> visible: array<u32>;
@group(0) @binding(5) var<storage, read_write> counter: atomic<u32>;
var<workgroup> local_count: atomic<u32>;
var<workgroup> base: u32;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) g: vec3<u32>, @builtin(local_invocation_index) l: u32,
        @builtin(workgroup_id) w: vec3<u32>, @builtin(num_workgroups) n: vec3<u32>) {
    let slot = g.x + (w.y * n.x) * 64u;
    if (l == 0u) { atomicStore(&local_count, 0u); }
    workgroupBarrier();
    var keep = false;
    var at = 0u;
    if (slot < params.count) {
        let t = ticks[slot];
        let s = states[slot][2];
        let block = vec3<f32>(bitcast<vec3<i32>>(t[0].xyz) - vec3<i32>(1000, 64, 1000));
        let frac = vec3<f32>(f32(t[0].w & 0xffffu), f32(t[0].w >> 16u), f32(t[1].x & 0xffffu)) / 65536.0;
        let delta = bitcast<vec3<f32>>(t[1].yzw);
        let p = block + frac - delta * 0.5;
        let radius = f32(s.x & 0xffffu) * (1.0 / 256.0);
        var inside = true;
        for (var i = 0u; i < 5u; i++) {
            let plane = normalize(vec3<f32>(f32(i) - 2.0, 1.0, 0.5));
            inside = inside && dot(plane, p) > -radius - 1e9;
        }
        let texel = (slot * 2654435761u) % arrayLength(&pyramid);
        let occluded = pyramid[texel] > 2.0;
        let pick = (slot * 2246822519u) % 1000u < params.visible_per_mille;
        keep = inside && !occluded && pick;
        if (keep) { at = atomicAdd(&local_count, 1u); }
    }
    workgroupBarrier();
    if (l == 0u) { base = atomicAdd(&counter, atomicLoad(&local_count)); }
    workgroupBarrier();
    if (keep) { visible[base + at] = slot; }
}
"#;

/// Lane per visible mob. Each bone samples `CURVES` keyframe channels (a linear search over 8 keys)
/// and `TRIG` cosine channels, builds a rotation from three Euler angles, composes with its parent
/// when `HIER` is set, and writes a `mat3x4<f32>`.
const POSE: &str = r#"
override BONES: u32 = 6u;
override CURVES: u32 = 0u;
override TRIG: u32 = 1u;
override HIER: bool = false;
struct Params { count: u32, pt: f32, pad0: u32, pad1: u32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> visible: array<u32>;
@group(0) @binding(2) var<storage, read> ticks: array<array<vec4<u32>, 4>>;
@group(0) @binding(3) var<storage, read> states: array<array<vec4<u32>, 3>>;
@group(0) @binding(4) var<storage, read> curves: array<vec2<f32>>;
@group(0) @binding(5) var<storage, read_write> bones: array<mat3x4<f32>>;
fn sample(curve: u32, t: f32) -> f32 {
    let first = (curve % 64u) * 8u;
    var k = 0u;
    for (var i = 1u; i < 7u; i++) { if (curves[first + i].x <= t) { k = i; } }
    let a = curves[first + k];
    let b = curves[first + k + 1u];
    return mix(a.y, b.y, clamp((t - a.x) / max(b.x - a.x, 1e-6), 0.0, 1.0));
}
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) g: vec3<u32>, @builtin(workgroup_id) w: vec3<u32>,
        @builtin(num_workgroups) n: vec3<u32>) {
    let lane = g.x + (w.y * n.x) * 64u;
    if (lane >= params.count) { return; }
    let slot = visible[lane];
    let t = ticks[slot];
    let s = states[slot];
    let walk = bitcast<f32>(t[2].w) + params.pt;
    let speed = mix(bitcast<f32>(t[3].x), bitcast<f32>(t[3].y), params.pt);
    let yaw = f32(t[2].x & 0xffffu) * (6.2831853 / 65536.0);
    let pitch = f32(t[2].z & 0xffffu) * (1.0 / 32768.0);
    let clock = f32(bitcast<i32>(s[2].z)) + params.pt;
    var parent = mat3x4<f32>(vec4(1.0, 0.0, 0.0, 0.0), vec4(0.0, 1.0, 0.0, 0.0), vec4(0.0, 0.0, 1.0, 0.0));
    for (var b = 0u; b < BONES; b++) {
        var e = vec3<f32>(yaw * 0.1, pitch * 0.1, 0.0);
        for (var c = 0u; c < TRIG; c++) { e.x += cos(walk * 0.6662 + f32(b + c)) * 1.4 * speed; }
        for (var c = 0u; c < CURVES; c++) {
            let v = sample(b * 3u + c, fract(clock * 0.05));
            e[c % 3u] += v;
        }
        e = clamp(e, vec3(-1e6), vec3(1e6));
        let cs = cos(e);
        let sn = sin(e);
        let r0 = vec3(cs.y * cs.z, cs.y * sn.z, -sn.y);
        let r1 = vec3(sn.x * sn.y * cs.z - cs.x * sn.z, sn.x * sn.y * sn.z + cs.x * cs.z, sn.x * cs.y);
        let r2 = vec3(cs.x * sn.y * cs.z + sn.x * sn.z, cs.x * sn.y * sn.z - sn.x * cs.z, cs.x * cs.y);
        let pivot = vec3(f32(b), 12.0, -2.0);
        var m = mat3x4<f32>(vec4(r0, pivot.x), vec4(r1, pivot.y), vec4(r2, pivot.z));
        if (HIER) {
            var composed: mat3x4<f32>;
            let t = vec4(m[0].w, m[1].w, m[2].w, 1.0);
            for (var i = 0u; i < 3u; i++) {
                let row = parent[i];
                composed[i] = vec4(row.x * m[0].xyz + row.y * m[1].xyz + row.z * m[2].xyz, dot(row, t));
            }
            m = composed;
            if ((b & 3u) != 3u) { parent = m; }
        }
        bones[lane * BONES + b] = m;
    }
}
"#;

/// Lane per visible mob and cuboid: the camera goes into bone space, the faces that face it are
/// selected, and up to three 8 B face records are appended. `AGGREGATE` reserves per workgroup;
/// otherwise each lane does its own `atomicAdd`, as `docs/entity-render.md`, section 5.3, states.
const EXPAND: &str = r#"
override CUBOIDS: u32 = 7u;
override BONES: u32 = 6u;
override AGGREGATE: bool = true;
struct Params { count: u32, pad0: u32, pad1: u32, pad2: u32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> cuboids: array<array<vec4<f32>, 2>>;
@group(0) @binding(2) var<storage, read> bones: array<mat3x4<f32>>;
@group(0) @binding(3) var<storage, read_write> faces: array<vec2<u32>>;
@group(0) @binding(4) var<storage, read_write> counter: atomic<u32>;
var<workgroup> local_count: atomic<u32>;
var<workgroup> base: u32;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) g: vec3<u32>, @builtin(local_invocation_index) l: u32,
        @builtin(workgroup_id) w: vec3<u32>, @builtin(num_workgroups) n: vec3<u32>) {
    let lane = g.x + (w.y * n.x) * 64u;
    if (l == 0u) { atomicStore(&local_count, 0u); }
    workgroupBarrier();
    var mask = 0u;
    var count = 0u;
    var at = 0u;
    let mob = lane / CUBOIDS;
    let cuboid = lane % CUBOIDS;
    if (lane < params.count) {
        let c = cuboids[cuboid];
        let bone = bitcast<u32>(c[0].w) % BONES;
        let m = bones[mob * BONES + bone];
        let eye = vec3<f32>(f32(mob % 97u), 30.0, f32(mob % 89u));
        let local = eye - vec3(m[0].w, m[1].w, m[2].w);
        let e = vec3(dot(m[0].xyz, local), dot(m[1].xyz, local), dot(m[2].xyz, local));
        let lo = c[0].xyz;
        let hi = c[0].xyz + c[1].xyz;
        mask = select(0u, 1u, e.x > hi.x) | select(0u, 2u, e.x < lo.x)
             | select(0u, 4u, e.y > hi.y) | select(0u, 8u, e.y < lo.y)
             | select(0u, 16u, e.z > hi.z) | select(0u, 32u, e.z < lo.z);
        mask &= bitcast<u32>(c[1].w);
        count = countOneBits(mask);
        if (AGGREGATE) { at = atomicAdd(&local_count, count); } else { at = atomicAdd(&counter, count); }
    }
    if (AGGREGATE) {
        workgroupBarrier();
        if (l == 0u) { base = atomicAdd(&counter, atomicLoad(&local_count)); }
        workgroupBarrier();
        at += base;
    }
    for (var f = 0u; f < 6u; f++) {
        if ((mask & (1u << f)) != 0u) {
            faces[at % arrayLength(&faces)] = vec2<u32>(mob, cuboid | (f << 24u));
            at++;
        }
    }
}
"#;

fn entity_passes(gpu: &Gpu) {
    println!("\n# entity passes (synthetic data, record sizes of docs/entity-render.md)");
    let max = 300_000u32;
    let ticks = gpu.filled(max as u64 * 16);
    let states = gpu.filled(max as u64 * 12);
    let pyramid = gpu.filled(1 << 20);
    let visible = gpu.buffer(max as u64 * 4, BufferUsages::empty());
    let counter = gpu.buffer(16, BufferUsages::empty());
    let identity: Vec<u32> = (0..max).collect();
    gpu.queue
        .write_buffer(&visible, 0, bytemuck::cast_slice(&identity));
    let mut curve_words = Vec::new();
    for c in 0..64u32 {
        for k in 0..8u32 {
            curve_words.push((k as f32 * 0.15).to_bits());
            curve_words.push(((c + k) as f32 * 0.1).sin().to_bits());
        }
    }
    let curves = gpu.buffer(curve_words.len() as u64 * 4, BufferUsages::empty());
    gpu.queue
        .write_buffer(&curves, 0, bytemuck::cast_slice(&curve_words));

    let cull = gpu.compute(CULL, &[]);
    for (n, per_mille) in [
        (10_000u32, 300u32),
        (100_000, 300),
        (300_000, 300),
        (300_000, 1000),
    ] {
        let params = gpu.uniform(&[n, per_mille, 0, 0]);
        let group = gpu.bind(
            &cull.get_bind_group_layout(0),
            &[&params, &ticks, &states, &pyramid, &visible, &counter],
        );
        let (med, min) = gpu.time_dispatch(&cull, &[group], n.div_ceil(64));
        println!(
            "cull   slots {n:>7} kept {:>4}‰  {med:7.4} ms median {min:7.4} min  {:6.2} ns/slot",
            per_mille,
            med * 1e6 / n as f64
        );
    }
    gpu.queue
        .write_buffer(&visible, 0, bytemuck::cast_slice(&identity));

    let bone_bytes = max as u64 * 16 * 48;
    let bones = gpu.buffer(bone_bytes, BufferUsages::empty());
    for (name, bone_count, curves_per_bone, trig, hier) in [
        ("pig", 6u32, 0u32, 1u32, false),
        ("pig + saddle", 12, 0, 1, false),
        ("frog", 16, 3, 0, true),
    ] {
        let pose = gpu.compute(
            POSE,
            &[
                ("BONES", bone_count as f64),
                ("CURVES", curves_per_bone as f64),
                ("TRIG", trig as f64),
                ("HIER", if hier { 1.0 } else { 0.0 }),
            ],
        );
        for n in [10_000u32, 100_000, 300_000] {
            let params = gpu.uniform(&[n, 0.5f32.to_bits(), 0, 0]);
            let group = gpu.bind(
                &pose.get_bind_group_layout(0),
                &[&params, &visible, &ticks, &states, &curves, &bones],
            );
            let (med, min) = gpu.time_dispatch(&pose, &[group], n.div_ceil(64));
            let bytes = n as f64 * (64.0 + 48.0 + bone_count as f64 * 48.0);
            println!(
                "pose   {name:<13} mobs {n:>7}  {med:7.4} ms median {min:7.4} min  {:6.2} ns/mob  {:6.1} GB/s",
                med * 1e6 / n as f64,
                bytes / med * 1e-6
            );
        }
    }

    let faces = gpu.buffer(max as u64 * 16 * 3 * 8, BufferUsages::empty());
    for (name, cuboid_count, bone_count) in [("pig", 7u32, 6u32), ("frog", 16, 16)] {
        let mut words = Vec::new();
        for c in 0..cuboid_count {
            let size = if name == "frog" && c % 2 == 1 {
                [3.0f32, 0.0, 3.0]
            } else {
                [4.0, 6.0, 4.0]
            };
            words.extend([-2.0f32, 0.0, -2.0].map(f32::to_bits));
            words.push(c);
            words.extend(size.map(f32::to_bits));
            words.push(if size[1] == 0.0 { 0b000100 } else { 0b111111 });
        }
        let cuboids = gpu.buffer(words.len() as u64 * 4, BufferUsages::empty());
        gpu.queue
            .write_buffer(&cuboids, 0, bytemuck::cast_slice(&words));
        for aggregate in [true, false] {
            let expand = gpu.compute(
                EXPAND,
                &[
                    ("CUBOIDS", cuboid_count as f64),
                    ("BONES", bone_count as f64),
                    ("AGGREGATE", if aggregate { 1.0 } else { 0.0 }),
                ],
            );
            for n in [10_000u32, 100_000, 300_000] {
                let lanes = n * cuboid_count;
                let params = gpu.uniform(&[lanes, 0, 0, 0]);
                let group = gpu.bind(
                    &expand.get_bind_group_layout(0),
                    &[&params, &cuboids, &bones, &faces, &counter],
                );
                let reset = gpu.device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: BufferUsages::COPY_SRC,
                    mapped_at_creation: true,
                });
                reset.unmap();
                let (med, min) = gpu.time(|encoder, set, reps| {
                    encoder.copy_buffer_to_buffer(&reset, 0, &counter, 0, 4);
                    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: None,
                        timestamp_writes: set.map(|set| ComputePassTimestampWrites {
                            query_set: set,
                            beginning_of_pass_write_index: Some(0),
                            end_of_pass_write_index: Some(1),
                        }),
                    });
                    pass.set_pipeline(&expand);
                    pass.set_bind_group(0, &group, &[]);
                    let wg = lanes.div_ceil(64);
                    for _ in 0..reps {
                        pass.dispatch_workgroups(wg.min(65535), wg.div_ceil(65535), 1);
                    }
                });
                println!(
                    "expand {name:<5} {} mobs {n:>7} lanes {lanes:>8}  {med:7.4} ms median {min:7.4} min  {:6.2} ns/mob",
                    if aggregate {
                        "workgroup atomic"
                    } else {
                        "lane atomic     "
                    },
                    med * 1e6 / n as f64
                );
            }
        }
    }
}

/// Vertex pulling of face records into the three gbuffer targets and a depth target. Faces are
/// laid out on a grid of `side`-pixel squares; `side = 0.25` gives triangles that cover almost no
/// sample, so the time is the vertex and setup cost.
const DRAW: &str = r#"
struct Params { side: f32, columns: u32, discard_on: u32, layers: u32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> faces: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read> cuboids: array<array<vec4<f32>, 2>>;
@group(0) @binding(3) var<storage, read> bones: array<mat3x4<f32>>;
@group(0) @binding(4) var tex: texture_2d_array<f32>;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) @interpolate(flat) layer: u32 };
@vertex
fn vs(@builtin(vertex_index) v: u32) -> Out {
    let face = faces[v / 6u];
    let corner = array(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0))[v % 6u];
    let c = cuboids[face.y & 0xffu];
    let m = bones[(face.x * 6u + (face.y & 0xffu) % 6u) % arrayLength(&bones)];
    let local = c[0].xyz + vec3(corner, 0.0) * c[1].xyz;
    let world = vec3(dot(m[0], vec4(local, 1.0)), dot(m[1], vec4(local, 1.0)), dot(m[2], vec4(local, 1.0)));
    let i = v / 6u;
    let cell = vec2(f32(i % params.columns), f32((i / params.columns) % (1440u / max(u32(params.side), 1u) + 1u)));
    let pixel = (cell + corner) * params.side + world.xy * 1e-30;
    var out: Out;
    out.pos = vec4(pixel / vec2(1280.0, -720.0) + vec2(-1.0, 1.0), 0.5 + world.z * 1e-30, 1.0);
    out.uv = corner;
    out.layer = i % params.layers;
    return out;
}
struct G { @location(0) a: vec4<f32>, @location(1) n: vec4<f32>, @location(2) l: vec4<f32> };
@fragment
fn fs(in: Out) -> G {
    let texel = textureLoad(tex, vec2<u32>(in.uv * 63.0), in.layer, 0);
    if (params.discard_on != 0u && texel.a < 0.1) { discard; }
    var g: G;
    g.a = vec4(texel.rgb * 0.8, 1.0);
    g.n = vec4(0.3, 0.4, 0.0, 0.0);
    g.l = vec4(0.0, 0.5, 0.9, 0.0);
    return g;
}
"#;

fn raster(gpu: &Gpu) {
    println!("\n# gbuffer draw: vertex pulling, {WIDTH}x{HEIGHT}, 3 targets + depth32");
    let device = &gpu.device;
    let targets: Vec<TextureView> = GBUFFER
        .iter()
        .map(|&format| {
            device
                .create_texture(&TextureDescriptor {
                    label: None,
                    size: Extent3d {
                        width: WIDTH,
                        height: HEIGHT,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        })
        .collect();
    let depth = device
        .create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    let layers = 16u32;
    let texture = device.create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let pixels = vec![200u8; 64 * 64 * 4 * layers as usize];
    gpu.queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(256),
            rows_per_image: Some(64),
        },
        Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: layers,
        },
    );
    let texture_view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });

    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(DRAW.into()),
    });
    let color: Vec<Option<ColorTargetState>> = GBUFFER
        .iter()
        .map(|&format| {
            Some(ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })
        })
        .collect();
    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &color,
        }),
        multiview_mask: None,
        cache: None,
    });

    let face_count = 6_300_000u32;
    let face_words: Vec<u32> = (0..face_count).flat_map(|i| [i / 7, i % 7]).collect();
    let faces = gpu.buffer(face_words.len() as u64 * 4, BufferUsages::empty());
    gpu.queue
        .write_buffer(&faces, 0, bytemuck::cast_slice(&face_words));
    let cuboids = gpu.filled(16 * 8);
    let bones = gpu.filled(300_000 * 6 * 12);

    for (side, count, discard_on) in [
        (0.25f32, 630_000u32, 1u32),
        (0.25, 6_300_000, 1),
        (2.0, 921_600, 1),
        (4.0, 230_400, 1),
        (4.0, 921_600, 1),
        (8.0, 57_600, 1),
        (16.0, 14_400, 1),
        (16.0, 57_600, 1),
        (16.0, 57_600, 0),
        (64.0, 3_600, 1),
    ] {
        let columns = (WIDTH as f32 / side) as u32;
        let params = gpu.uniform(&[side.to_bits(), columns, discard_on, layers]);
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: faces.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: cuboids.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: bones.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&texture_view),
                },
            ],
        });
        let (med, min, wall) = gpu.time_both(|encoder, set, reps| {
            let attachments: Vec<_> = targets
                .iter()
                .map(|view| {
                    Some(RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })
                })
                .collect();
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &attachments,
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: set.map(|set| RenderPassTimestampWrites {
                    query_set: set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            for _ in 0..reps {
                pass.draw(0..count * 6, 0..1);
            }
        });
        let covered = if side < 1.0 {
            0.0
        } else {
            count as f64 * (side as f64).powi(2)
        };
        println!(
            "draw side {side:>5} px faces {count:>8} verts {:>9} covered {:5.2} screens discard {discard_on}  wall {wall:7.3} ms (timestamps {med:7.3} median {min:7.3} min)  {:6.2} ns/face  {:6.2} Gpx/s",
            count * 6,
            covered / (WIDTH * HEIGHT) as f64,
            wall * 1e6 / count as f64,
            covered / wall * 1e-6
        );
    }
}
