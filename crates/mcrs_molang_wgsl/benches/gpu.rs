//! GPU throughput of lowered channels in a pose-shaped kernel: one lane per mob reads its tick
//! records (current and previous), its state record and its input record
//! (`docs/entity-render.md`, section 4), evaluates one channel and writes one `f32`.
//!
//! The timing follows `docs/entity-render-budget.md`, section 4.14 and appendix A: timestamps at
//! the bounds of a compute pass, a warm-up that keeps the clock up, a calibration that makes one
//! submission about 25 ms long, and the median and minimum over 9 submissions. On Metal the
//! timestamps of a compute pass bracket its work; the wall time per repetition is printed next
//! to them as a check.
//!
//! `cargo bench -p mcrs_molang_wgsl --bench gpu`

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a measurement program has no caller to return an error to"
)]

#[path = "../tests/gpu/mod.rs"]
mod gpu;

use std::fmt::Write as _;
use std::time::Instant;

use mcrs_molang::{
    Arguments, ChannelClass, CompileOptions, InputRange, Milestone, PureInput, PureProgram,
    QueryDeclaration, QueryRegistry, QueryScope, Rate, UseContext, ValueType, VariableLayout,
    compile_text,
};
use wgpu::{
    BindGroup, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoder, ComputePassDescriptor,
    ComputePassTimestampWrites, ComputePipeline, ComputePipelineDescriptor, MapMode,
    PipelineLayoutDescriptor, PollType, QuerySet, QuerySetDescriptor, QueryType,
    ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

use gpu::Gpu;

const RUNS: usize = 9;
const WORKGROUP: u32 = 128;

/// The leg swing of `docs/spec/molang-queries.md`, MOLQ-WALK-001, postcondition 2.
const WALK: &str =
    "math.cos(q.modified_distance_moved * 38.1704) * 80.2141 * q.modified_move_speed";

/// A synthetic channel of about 30 syntax nodes over the clock, the walk, the head and a flag.
const THIRTY_NODES: &str = "math.sin(q.anim_time * 120 + v.phase) \
    * math.lerp(8, 20, math.clamp(q.modified_move_speed, 0, 1)) \
    + math.cos(q.life_time * 45) * 4 \
    - (q.is_baby ? 2 : 0) * math.abs(math.sin(q.life_time * 30)) \
    + math.min_angle(q.body_y_rotation - q.head_y_rotation) * 0.25 \
    + math.clamp(q.head_x_rotation, -30, 30) * 0.5";

/// Each query of the benchmark, and the WGSL expression of the kernel that gives its value.
const QUERIES: [(&str, Rate, &str); 8] = [
    (
        "modified_distance_moved",
        Rate::EntityFrame,
        "walk_distance",
    ),
    ("modified_move_speed", Rate::EntityFrame, "walk_speed"),
    (
        "anim_time",
        Rate::AnimationFrame,
        "clock - inputs[lane * 24u + 17u]",
    ),
    ("life_time", Rate::EntityFrame, "clock"),
    (
        "is_baby",
        Rate::EntityTick,
        "f32(state[lane * 12u + 5u] & 1u)",
    ),
    (
        "body_y_rotation",
        Rate::EntityFrame,
        "angle_lerp(previous.x >> 16u, current.x >> 16u)",
    ),
    (
        "head_y_rotation",
        Rate::EntityFrame,
        "angle_lerp(previous.y & 0xffffu, current.y & 0xffffu)",
    ),
    ("head_x_rotation", Rate::EntityFrame, "pitch"),
];

struct Channel {
    program: PureProgram,
    arguments: String,
}

fn channel(text: &str) -> Channel {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for (name, rate, _) in QUERIES {
        registry.declare(QueryDeclaration {
            name: name.into(),
            arguments: Arguments::None,
            result: ValueType::Number,
            rate,
            scope: QueryScope::Entity,
            milestone: Milestone::One,
        });
    }
    let mut layout = VariableLayout::new();
    let compiled = compile_text(text, &mut layout, &mut registry, &CompileOptions::default())
        .expect("the benchmark channel compiles");
    let ChannelClass::Pure(program) = compiled.expression.classify(|_| Some(InputRange::FINITE))
    else {
        panic!("{text} is not pure");
    };
    let mut arguments = String::from("params.zero");
    for input in program.inputs() {
        let value = match input {
            PureInput::Query(slot) => {
                let (_, key) = registry.slots().find(|(known, _)| known == slot).unwrap();
                let name = &registry.declaration(key.declaration).unwrap().name;
                QUERIES.iter().find(|entry| entry.0 == &**name).unwrap().2
            }
            PureInput::Variable(_) => "inputs[lane * 24u]",
            _ => "0.0",
        };
        write!(arguments, ", {value}").unwrap();
    }
    Channel { program, arguments }
}

/// A pose-shaped kernel. `body` computes `value` from the decoded records.
fn kernel(library: &str, function: &str, body: &str) -> String {
    format!(
        "{library}
{function}
struct Params {{ zero: u32, lanes: u32, tick: u32, partial: f32 }}
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> current_ticks: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> previous_ticks: array<vec4<u32>>;
@group(0) @binding(3) var<storage, read> state: array<u32>;
@group(0) @binding(4) var<storage, read> inputs: array<f32>;
@group(0) @binding(5) var<storage, read_write> outputs: array<f32>;

fn angle_lerp(previous: u32, current: u32) -> f32 {{
    let delta = f32(i32((current - previous + 32768u) & 0xffffu) - 32768);
    return (f32(previous) + params.partial * delta) * (360.0 / 65536.0);
}}

@compute @workgroup_size({WORKGROUP})
fn main(@builtin(global_invocation_id) id: vec3<u32>) {{
    let lane = id.x;
    if (lane >= params.lanes) {{ return; }}
    let current = current_ticks[lane * 2u + 1u];
    let previous = previous_ticks[lane * 2u + 1u];
    let pt = params.partial;
    let walk_position = bitcast<f32>(current.z);
    let walk_speed_now = bitcast<f32>(current.w);
    let walk_speed_old = bitcast<f32>(previous.w);
    let walk_distance = walk_position - walk_speed_now * (1.0 - pt);
    let walk_speed = min(walk_speed_old + pt * (walk_speed_now - walk_speed_old), 1.0);
    let pitch = f32(bitcast<i32>(current.y) >> 16u) * (90.0 / 32767.0);
    let clock = f32(i32(params.tick) - bitcast<i32>(state[lane * 12u + 10u])) + pt;
    var value = 0.0;
    {body}
    outputs[lane] = value;
}}
"
    )
}

/// The library with the fast-math barriers removed, to show what the bit-exact twin costs.
fn fast_library() -> String {
    mcrs_molang_wgsl::LIBRARY
        .replace(
            "fn molang_opaque(x: f32) -> f32 { return bitcast<f32>(bitcast<u32>(x) | molang_zero); }",
            "fn molang_opaque(x: f32) -> f32 { return x; }",
        )
        .replace(
            "    let bits = bitcast<u32>(x) | molang_zero;\n    return bitcast<f32>(select(bits, bits & 0x80000000u, (bits & 0x7f800000u) == 0u));",
            "    return x;",
        )
}

struct Timer {
    set: QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period: f64,
}

struct Bench {
    gpu: Gpu,
    timer: Option<Timer>,
    heat: (ComputePipeline, BindGroup),
}

const HEAT: &str = "
@group(0) @binding(0) var<storage, read_write> out: array<f32>;
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    var x = f32(g.x);
    for (var i = 0u; i < 512u; i++) { x = fma(x, 1.0001, 0.5); }
    out[g.x & 0xffffu] = x;
}
";

impl Bench {
    fn new(gpu: Gpu) -> Self {
        let timer = gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Timer {
                set: gpu.device.create_query_set(&QuerySetDescriptor {
                    label: None,
                    ty: QueryType::Timestamp,
                    count: 2,
                }),
                resolve: gpu.device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                read: gpu.device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                period: f64::from(gpu.queue.get_timestamp_period()),
            });
        let pipeline = gpu.pipeline(HEAT);
        let out = gpu.buffer(&vec![0u8; 1 << 18], BufferUsages::STORAGE);
        let group = gpu.bind(&pipeline, &[&out]);
        let bench = Self {
            gpu,
            timer,
            heat: (pipeline, group),
        };
        let start = Instant::now();
        while start.elapsed().as_secs_f64() < 2.0 {
            bench.heat();
            bench
                .gpu
                .device
                .poll(PollType::wait_indefinitely())
                .unwrap();
        }
        bench
    }

    /// The governor lowers the clock for a pass with idle gaps around it, so arithmetic runs
    /// ahead of each timed submission.
    fn heat(&self) {
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.heat.0);
            pass.set_bind_group(0, &self.heat.1, &[]);
            pass.dispatch_workgroups(4096, 1, 1);
        }
        self.gpu.queue.submit([encoder.finish()]);
    }

    fn record(
        encoder: &mut CommandEncoder,
        set: Option<&QuerySet>,
        pipeline: &ComputePipeline,
        group: &BindGroup,
        workgroups: u32,
        reps: u32,
    ) {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
            timestamp_writes: set.map(|set| ComputePassTimestampWrites {
                query_set: set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        for _ in 0..reps {
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
    }

    /// GPU time and wall time of one submission with `reps` dispatches, in milliseconds.
    fn submit(
        &self,
        pipeline: &ComputePipeline,
        group: &BindGroup,
        workgroups: u32,
        reps: u32,
    ) -> (f64, f64) {
        self.heat();
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        Self::record(
            &mut encoder,
            self.timer.as_ref().map(|timer| &timer.set),
            pipeline,
            group,
            workgroups,
            reps,
        );
        if let Some(timer) = &self.timer {
            encoder.resolve_query_set(&timer.set, 0..2, &timer.resolve, 0);
            encoder.copy_buffer_to_buffer(&timer.resolve, 0, &timer.read, 0, 16);
        }
        self.gpu.device.poll(PollType::wait_indefinitely()).unwrap();
        let start = Instant::now();
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.device.poll(PollType::wait_indefinitely()).unwrap();
        let wall = start.elapsed().as_secs_f64() * 1e3;
        let Some(timer) = &self.timer else {
            return (wall, wall);
        };
        timer.read.slice(..).map_async(MapMode::Read, |_| {});
        self.gpu.device.poll(PollType::wait_indefinitely()).unwrap();
        let ticks: Vec<u64> =
            bytemuck::cast_slice(&timer.read.slice(..).get_mapped_range()).to_vec();
        timer.read.unmap();
        let gpu = if ticks[1] > ticks[0] {
            (ticks[1] - ticks[0]) as f64 * timer.period * 1e-6
        } else {
            wall
        };
        (gpu, wall)
    }

    /// The median and minimum GPU time and the median wall time of one dispatch, in ms.
    fn time(&self, pipeline: &ComputePipeline, group: &BindGroup, workgroups: u32) -> [f64; 3] {
        let once = self.submit(pipeline, group, workgroups, 1).0;
        let reps = ((25.0 / once.max(1e-3)).ceil() as u32).clamp(1, 4000);
        self.submit(pipeline, group, workgroups, reps);
        let mut gpu = Vec::with_capacity(RUNS);
        let mut wall = Vec::with_capacity(RUNS);
        for _ in 0..RUNS {
            let (g, w) = self.submit(pipeline, group, workgroups, reps);
            gpu.push(g / f64::from(reps));
            wall.push(w / f64::from(reps));
        }
        gpu.sort_by(f64::total_cmp);
        wall.sort_by(f64::total_cmp);
        [gpu[RUNS / 2], gpu[0], wall[RUNS / 2]]
    }
}

/// A pipeline with the bindings of [`kernel`] fixed, so that a kernel that reads only some
/// records keeps the same bind group.
fn pose_pipeline(gpu: &Gpu, source: &str) -> ComputePipeline {
    let entry = |binding: u32, ty: BufferBindingType| BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let mut entries = vec![entry(0, BufferBindingType::Uniform)];
    for binding in 1..5 {
        entries.push(entry(
            binding,
            BufferBindingType::Storage { read_only: true },
        ));
    }
    entries.push(entry(5, BufferBindingType::Storage { read_only: false }));
    let group = gpu
        .device
        .create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: &entries,
        });
    let layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&group)],
            immediate_size: 0,
        });
    let module = gpu.device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(source.into()),
    });
    gpu.device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
}

fn words(count: usize, seed: u32, map: impl Fn(u32, usize) -> u32) -> Vec<u8> {
    let data: Vec<u32> = (0..count)
        .map(|index| {
            map(
                (index as u32)
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(seed)
                    >> 8,
                index,
            )
        })
        .collect();
    bytemuck::cast_slice(&data).to_vec()
}

fn main() {
    let gpu = Gpu::new().expect("no adapter");
    println!("adapter: {}", gpu.name);
    let bench = Bench::new(gpu);
    println!(
        "timing: {}",
        if bench.timer.is_some() {
            "compute pass timestamps"
        } else {
            "wall clock"
        }
    );
    let walk = channel(WALK);
    let thirty = channel(THIRTY_NODES);
    println!(
        "walk: {} instructions, {} inputs; thirty nodes: {} instructions, {} inputs",
        walk.program.instructions().len(),
        walk.program.inputs().len(),
        thirty.program.instructions().len(),
        thirty.program.inputs().len()
    );
    let fast = fast_library();
    let baseline_body = "value = walk_distance + walk_speed + clock + pitch + inputs[lane * 24u] \
        + f32(state[lane * 12u + 5u]) + angle_lerp(previous.x >> 16u, current.x >> 16u) \
        - inputs[lane * 24u + 17u];";
    let mut variants: Vec<(String, String)> =
        vec![("records only".to_owned(), kernel("", "", baseline_body))];
    for (name, channel) in [("walk", &walk), ("thirty nodes", &thirty)] {
        let function = mcrs_molang_wgsl::lower(&channel.program, "channel");
        let body = format!("value = channel({});", channel.arguments);
        variants.push((
            format!("{name}, bit-exact"),
            kernel(mcrs_molang_wgsl::LIBRARY, &function, &body),
        ));
        variants.push((
            format!("{name}, no barriers"),
            kernel(&fast, &function, &body),
        ));
    }
    for lanes in [100_000usize, 300_000] {
        let current = bench.gpu.buffer(
            &words(lanes * 8, 1, |random, index| match index % 8 {
                6 => (random as f32 * 1e-4).to_bits(),
                7 => (0.3 + (random % 1000) as f32 * 1e-3).to_bits(),
                _ => random,
            }),
            BufferUsages::STORAGE,
        );
        let previous = bench.gpu.buffer(
            &words(lanes * 8, 2, |random, index| match index % 8 {
                6 => (random as f32 * 1e-4).to_bits(),
                7 => (0.2 + (random % 1000) as f32 * 1e-3).to_bits(),
                _ => random,
            }),
            BufferUsages::STORAGE,
        );
        let state = bench.gpu.buffer(
            &words(lanes * 12, 3, |random, _| random & 0xffff),
            BufferUsages::STORAGE,
        );
        let inputs = bench.gpu.buffer(
            &words(lanes * 24, 4, |random, _| {
                ((random % 10_000) as f32 * 0.01).to_bits()
            }),
            BufferUsages::STORAGE,
        );
        let outputs = bench
            .gpu
            .buffer(&vec![0u8; lanes * 4], BufferUsages::STORAGE);
        let params = bench.gpu.buffer(
            bytemuck::cast_slice(&[0u32, lanes as u32, 100_000, 0.375f32.to_bits()]),
            BufferUsages::UNIFORM,
        );
        let workgroups = (lanes as u32).div_ceil(WORKGROUP);
        for (name, source) in &variants {
            gpu::validate(source).unwrap_or_else(|error| panic!("naga: {error}"));
            let pipeline = pose_pipeline(&bench.gpu, source);
            let group = bench.gpu.bind(
                &pipeline,
                &[&params, &current, &previous, &state, &inputs, &outputs],
            );
            let [median, min, wall] = bench.time(&pipeline, &group, workgroups);
            println!(
                "{name:<26} lanes {lanes:>7}  {median:8.4} ms median {min:8.4} min  wall {wall:8.4}  {:6.3} ns/lane",
                median * 1e6 / lanes as f64
            );
        }
    }
}
