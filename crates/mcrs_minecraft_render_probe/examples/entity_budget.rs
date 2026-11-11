//! Prints what the adapter offers, then times the hardware rates that the entity budget rests
//! on: a copy for memory bandwidth, FMA and sine loops for arithmetic, the upload of the slot
//! records and their CPU packing. The figures feed `docs/entity-render-budget.md`. The
//! `stress_field` example of `mcrs_minecraft_entity_render` times the entity passes themselves.
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
        let ticks: [u64; 2] = bytemuck::cast_slice(&t.read.slice(..).get_mapped_range().unwrap())
            [..2]
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
