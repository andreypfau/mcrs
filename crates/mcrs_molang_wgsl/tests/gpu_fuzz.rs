//! Generated pure programs on a real adapter: each GPU result must have the bits of the CPU
//! twin, and must agree with the scalar VM within the pose tolerance of
//! `docs/spec/entity-animation.md`, EANIM-GPU-003, after the clamp to `[-1e6, 1e6]`. A value at
//! a discontinuity is counted, not compared, and a subnormal value agrees with zero.
//!
//! `cargo test --release -p mcrs_molang_wgsl --test gpu_fuzz -- --ignored --nocapture`;
//! `MCRS_GPU_FUZZ_PROGRAMS` sets the number of programs (default 10,000).

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking, and the report reads top to bottom"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "the program count comes from the environment"
)]

#[path = "../../mcrs_molang/tests/generator/mod.rs"]
#[expect(
    dead_code,
    reason = "this test uses only the GPU profile of the shared generator"
)]
mod generator;
mod gpu;
mod twin;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use mcrs_molang::{
    BinaryFn, ChannelClass, CompileOptions, EaseDirection, EaseShape, Easing, Expression,
    InputRange, Milestone, PureInput, PureInstr, PureProgram, QueryRegistry, QueryValues, ScalarVm,
    Source, Subject, TernaryFn, UnaryFn, Value, VariableLayout, Variables, compile,
};
use mcrs_molang_wgsl::{LIBRARY, lower};
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;
use wgpu::{BufferDescriptor, BufferUsages, MapMode, PollType};

use generator::{Profile, program, registry};
use gpu::Gpu;
use twin::Twin;

const LANES: usize = 64;
const STRIDE: usize = 8;
const BATCH: usize = 128;
/// The scale limit of EANIM-GPU-003, the smallest of its limits.
const TOLERANCE: f32 = 1e-4;
/// `2^-20`: above about 800, one rounding step of an `f32` is larger than [`TOLERANCE`]; there
/// the comparison allows eight steps.
const RELATIVE_TOLERANCE: f32 = 1.0 / 1_048_576.0;

struct Case {
    text: String,
    program: PureProgram,
    expression: Expression,
    layout: VariableLayout,
    registry: QueryRegistry,
    inputs: Vec<[f32; STRIDE]>,
}

/// A small deterministic generator of lane inputs.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn value(&mut self) -> f32 {
        const POOL: [f32; 13] = [
            0.0, -0.0, 1.0, -1.0, 0.5, 2.0, 90.0, -180.0, 3.3, 1e6, -1e-3, 360.0, 45.0,
        ];
        match self.next() % 10 {
            0..=3 => POOL[(self.next() % POOL.len() as u64) as usize],
            4..=6 => self.unit() * 20.0 - 10.0,
            7 | 8 => self.unit() * 2000.0 - 1000.0,
            _ => {
                let magnitude = 10f32.powf(self.unit() * 18.0 - 6.0);
                if self.next() & 1 == 0 {
                    magnitude
                } else {
                    -magnitude
                }
            }
        }
    }
}

fn pose_clamp(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-1e6, 1e6)
    }
}

fn reason_name(class: &ChannelClass) -> String {
    match class {
        ChannelClass::Constant => "constant".to_owned(),
        ChannelClass::Pure(_) => "pure".to_owned(),
        ChannelClass::State(reason) => {
            let text = format!("{reason:?}");
            let name = text.split('(').next().unwrap_or_default();
            format!("state {name}")
        }
        other => format!("unclassified {other:?}"),
    }
}

fn generate(count: usize, census: &mut BTreeMap<String, usize>) -> Vec<Case> {
    let strategy = program(Profile::Gpu);
    let mut runner = TestRunner::deterministic();
    let mut random = Random(0x5eed);
    let mut cases = Vec::with_capacity(count);
    while cases.len() < count {
        let text = strategy.new_tree(&mut runner).unwrap().current();
        let mut layout = VariableLayout::new();
        let mut registry = registry();
        let options = CompileOptions {
            milestone: Milestone::Later,
            ..CompileOptions::default()
        };
        let Ok(compiled) = compile(&Source::Text(&text), &mut layout, &mut registry, &options)
        else {
            *census.entry("load error".to_owned()).or_default() += 1;
            continue;
        };
        let class = compiled.expression.classify(|_| Some(InputRange::FINITE));
        *census.entry(reason_name(&class)).or_default() += 1;
        let ChannelClass::Pure(program) = class else {
            continue;
        };
        assert!(program.inputs().len() <= STRIDE, "{text}: too many inputs");
        let inputs = (0..LANES)
            .map(|_| core::array::from_fn(|_| random.value()))
            .collect();
        cases.push(Case {
            text,
            program,
            expression: compiled.expression,
            layout,
            registry,
            inputs,
        });
    }
    cases
}

fn scalar(case: &Case, lane: &[f32; STRIDE]) -> f32 {
    let mut variables = Variables::new(&case.layout);
    let mut queries = QueryValues::new(case.registry.slot_count());
    let mut this = 0.0;
    for (input, value) in case.program.inputs().iter().zip(lane) {
        match input {
            PureInput::This => this = *value,
            PureInput::Query(slot) => queries.set(*slot, Some(u64::from(value.to_bits()))),
            PureInput::Variable(slot) => variables.set(*slot, Some(u64::from(value.to_bits()))),
            _ => {}
        }
    }
    let mut subject = Subject {
        variables: &mut variables,
        queries: &queries,
        this,
    };
    match case.expression.evaluate(&mut ScalarVm::new(), &mut subject) {
        Value::Number(value) => value,
        other => panic!("{}: {other:?}", case.text),
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCE.max(RELATIVE_TOLERANCE * a.abs().max(b.abs()))
}

/// The same bits, or a subnormal value on one side and a zero on the other: a GPU can flush a
/// subnormal value to zero, and the CPU does not.
fn same_or_flushed(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_subnormal() && b == 0.0) || (b.is_subnormal() && a == 0.0)
}

/// Whether a relative change of `2^-20` in one input moves the scalar result or the GPU result
/// past the tolerance. The input then sits at a discontinuity of one of them, for example a
/// comparison of a sine with `0`: a difference in the last bits of an inexact function moves
/// the discontinuity, and the two results can differ by any amount. The GPU side matters too:
/// `math.sin(-180)` is exactly 0 on one side only.
fn at_discontinuity(case: &Case, lane: &[f32; STRIDE], reference: f32, gpu: f32) -> bool {
    let reference = pose_clamp(reference);
    let count = case.program.inputs().len();
    let mut twin = Twin::new();
    (0..count).any(|index| {
        [1.0 + RELATIVE_TOLERANCE, 1.0 - RELATIVE_TOLERANCE]
            .iter()
            .any(|factor| {
                let mut nudged = *lane;
                nudged[index] = if nudged[index] == 0.0 {
                    1e-6
                } else {
                    nudged[index] * factor
                };
                let value = pose_clamp(scalar(case, &nudged));
                let gpu_value = twin.run(&case.program, &nudged[..count]).value;
                !close(value, reference) || !close(gpu_value, gpu)
            })
    })
}

fn exact_unary(function: UnaryFn, x: f64) -> f64 {
    let truth = |condition: bool| if condition { 1.0 } else { 0.0 };
    match function {
        UnaryFn::Negate => -x,
        UnaryFn::Not => truth(x == 0.0),
        UnaryFn::Truthy => truth(x != 0.0),
        UnaryFn::Abs => x.abs(),
        UnaryFn::Ceil => x.ceil(),
        UnaryFn::Floor => x.floor(),
        UnaryFn::Trunc => x.trunc(),
        UnaryFn::Round => x.round(),
        UnaryFn::Sqrt => x.sqrt(),
        UnaryFn::Exp => x.exp(),
        UnaryFn::Ln => x.ln(),
        UnaryFn::Sin => (x % 360.0).to_radians().sin(),
        UnaryFn::Cos => (x % 360.0).to_radians().cos(),
        UnaryFn::Asin => x.asin().to_degrees(),
        UnaryFn::Acos => x.acos().to_degrees(),
        UnaryFn::Atan => x.atan().to_degrees(),
        UnaryFn::Sign => truth(x > 0.0) * 2.0 - 1.0,
        UnaryFn::MinAngle => x - 360.0 * ((x + 180.0) / 360.0).floor(),
        UnaryFn::HermiteBlend => 3.0 * x * x - 2.0 * x * x * x,
    }
}

fn exact_binary(function: BinaryFn, a: f64, b: f64) -> f64 {
    let truth = |condition: bool| if condition { 1.0 } else { 0.0 };
    match function {
        BinaryFn::Add => a + b,
        BinaryFn::Subtract => a - b,
        BinaryFn::Multiply => a * b,
        BinaryFn::Divide => {
            if b == 0.0 {
                0.0
            } else {
                a / b
            }
        }
        BinaryFn::Less => truth(a < b),
        BinaryFn::LessEqual => truth(a <= b),
        BinaryFn::Greater => truth(a > b),
        BinaryFn::GreaterEqual => truth(a >= b),
        BinaryFn::Equal => truth(a == b),
        BinaryFn::NotEqual => truth(a != b),
        BinaryFn::BothTruthy => truth(a != 0.0 && b != 0.0),
        BinaryFn::EitherTruthy => truth(a != 0.0 || b != 0.0),
        BinaryFn::ApproxEqual => truth((a - b).abs() <= f64::from(0.000_000_1f32)),
        BinaryFn::Min => {
            if b < a {
                b
            } else {
                a
            }
        }
        BinaryFn::Max => {
            if b > a {
                b
            } else {
                a
            }
        }
        BinaryFn::Mod => {
            if b == 0.0 {
                0.0
            } else {
                a % b
            }
        }
        BinaryFn::Pow => a.powf(b),
        BinaryFn::Atan2 => a.atan2(b).to_degrees(),
        BinaryFn::CopySign => a.abs().copysign(b),
    }
}

fn exact_ease(easing: Easing, t: f64) -> f64 {
    let ease_in = |t: f64| match easing.shape {
        EaseShape::Quad => t * t,
        EaseShape::Cubic => t * t * t,
        EaseShape::Quart => t * t * t * t,
        EaseShape::Quint => t * t * t * t * t,
        EaseShape::Sine => 1.0 - (90.0 * t).to_radians().cos(),
        EaseShape::Expo => {
            if t == 0.0 {
                0.0
            } else {
                2f64.powf(10.0 * t - 10.0)
            }
        }
        EaseShape::Circ => 1.0 - (1.0 - t * t).sqrt(),
        EaseShape::Back => 2.70158 * t * t * t - 1.70158 * t * t,
        EaseShape::Elastic => {
            if t == 0.0 {
                0.0
            } else if t == 1.0 {
                1.0
            } else {
                -(2f64.powf(10.0 * t - 10.0)) * ((10.0 * t - 10.75) * 120.0).to_radians().sin()
            }
        }
        EaseShape::Bounce => {
            let (n, d) = (7.5625, 2.75);
            let u = 1.0 - t;
            let b = if u < 1.0 / d {
                n * u * u
            } else if u < 2.0 / d {
                n * (u - 1.5 / d).powi(2) + 0.75
            } else if u < 2.5 / d {
                n * (u - 2.25 / d).powi(2) + 0.9375
            } else {
                n * (u - 2.625 / d).powi(2) + 0.984375
            };
            1.0 - b
        }
    };
    match easing.direction {
        EaseDirection::In => ease_in(t),
        EaseDirection::Out => 1.0 - ease_in(1.0 - t),
        EaseDirection::InOut if t < 0.5 => ease_in(2.0 * t) / 2.0,
        EaseDirection::InOut => 1.0 - ease_in(2.0 - 2.0 * t) / 2.0,
    }
}

fn exact_ternary(function: TernaryFn, a: f64, b: f64, c: f64) -> f64 {
    let min_angle = |x: f64| x - 360.0 * ((x + 180.0) / 360.0).floor();
    match function {
        TernaryFn::Clamp => exact_binary(BinaryFn::Min, exact_binary(BinaryFn::Max, a, b), c),
        TernaryFn::Lerp => a + (b - a) * c,
        TernaryFn::InverseLerp => exact_binary(BinaryFn::Divide, c - a, b - a),
        TernaryFn::LerpRotate => min_angle(a + min_angle(b - a) * c),
        TernaryFn::InRange => {
            if b <= a && a <= c {
                1.0
            } else {
                0.0
            }
        }
        TernaryFn::Ease(easing) => a + (b - a) * exact_ease(easing, c),
    }
}

/// Whether the specification leaves the last bits of the result open.
fn inexact(instr: PureInstr) -> bool {
    match instr {
        PureInstr::Unary { function, .. } => matches!(
            function,
            UnaryFn::Exp
                | UnaryFn::Ln
                | UnaryFn::Sin
                | UnaryFn::Cos
                | UnaryFn::Asin
                | UnaryFn::Acos
                | UnaryFn::Atan
        ),
        PureInstr::Binary { function, .. } => matches!(function, BinaryFn::Pow | BinaryFn::Atan2),
        PureInstr::Ternary {
            function: TernaryFn::Ease(easing),
            ..
        } => matches!(
            easing.shape,
            EaseShape::Sine | EaseShape::Expo | EaseShape::Elastic
        ),
        _ => false,
    }
}

/// The program in `f64` with exact functions. Each result of an inexact function is multiplied
/// by `nudge`.
fn exact(program: &PureProgram, inputs: &[f32], nudge: f64) -> f64 {
    let mut registers = vec![0.0f64; program.registers()];
    let code = program.instructions();
    let mut pc = 0;
    while let Some(instr) = code.get(pc) {
        pc += 1;
        let (dst, value) = match *instr {
            PureInstr::Constant { dst, value } => (dst, f64::from(value)),
            PureInstr::Input { dst, input } => (dst, f64::from(inputs[usize::from(input)])),
            PureInstr::Unary { function, dst, a } => {
                (dst, exact_unary(function, registers[a.index()]))
            }
            PureInstr::Binary {
                function,
                dst,
                a,
                b,
            } => (
                dst,
                exact_binary(function, registers[a.index()], registers[b.index()]),
            ),
            PureInstr::Ternary {
                function,
                dst,
                a,
                b,
                c,
            } => (
                dst,
                exact_ternary(
                    function,
                    registers[a.index()],
                    registers[b.index()],
                    registers[c.index()],
                ),
            ),
            PureInstr::Select {
                dst,
                condition,
                then,
                otherwise,
            } => {
                let pick = if registers[condition.index()] != 0.0 {
                    then
                } else {
                    otherwise
                };
                (dst, registers[pick.index()])
            }
            PureInstr::Move { dst, src } => (dst, registers[src.index()]),
            PureInstr::If {
                condition,
                otherwise,
            } => {
                if registers[condition.index()] == 0.0 {
                    pc = otherwise as usize + 1;
                }
                continue;
            }
            PureInstr::Else { end } => {
                pc = end as usize + 1;
                continue;
            }
            PureInstr::Return { src } => return registers[src.index()],
            _ => continue,
        };
        registers[dst.index()] = if inexact(*instr) {
            value * nudge
        } else {
            value
        };
    }
    0.0
}

/// Whether a relative change of `2^-21` in the result of an inexact function moves the exact
/// result past the tolerance. The specification leaves the last bits of these functions open, so
/// such a lane has no single right result: `math.trunc` of an arc cosine that one side rounds to
/// `89.99999` and the other to `90` is an example. No change of an input shows it.
fn inexact_discontinuity(case: &Case, inputs: &[f32; STRIDE]) -> bool {
    let inputs = &inputs[..case.program.inputs().len()];
    let clamp = |value: f64| {
        if value.is_nan() {
            0.0
        } else {
            value.clamp(-1e6, 1e6)
        }
    };
    let reference = clamp(exact(&case.program, inputs, 1.0));
    [1.0 + 1.0 / 2_097_152.0, 1.0 - 1.0 / 2_097_152.0]
        .iter()
        .any(|nudge| {
            (clamp(exact(&case.program, inputs, *nudge)) - reference).abs() > f64::from(TOLERANCE)
        })
}

fn kernel(cases: &[Case]) -> String {
    let mut source = String::from(LIBRARY);
    for (index, case) in cases.iter().enumerate() {
        source.push_str(&lower(&case.program, &format!("channel_{index}")));
    }
    source.push_str(
        "@group(0) @binding(0) var<uniform> params: vec4<u32>;
@group(0) @binding(1) var<storage, read> inputs: array<f32>;
@group(0) @binding(2) var<storage, read_write> outputs: array<f32>;
@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let index = group.x * 64u + lane;
    let base = index * 8u;
    var value = 0.0;
    switch group.x {
",
    );
    for (index, case) in cases.iter().enumerate() {
        let mut arguments = String::from("params.x");
        for input in 0..case.program.inputs().len() {
            write!(arguments, ", inputs[base + {input}u]").unwrap();
        }
        writeln!(
            source,
            "        case {index}u: {{ value = channel_{index}({arguments}); }}"
        )
        .unwrap();
    }
    source.push_str("        default: {}\n    }\n    outputs[index] = value;\n}\n");
    source
}

fn run(gpu: &Gpu, source: &str, cases: &[Case]) -> Vec<f32> {
    let pipeline = gpu.pipeline(source);
    let inputs: Vec<f32> = cases
        .iter()
        .flat_map(|case| case.inputs.iter().flatten().copied())
        .collect();
    let lanes = cases.len() * LANES;
    let params = gpu.buffer(bytemuck::cast_slice(&[0u32; 4]), BufferUsages::UNIFORM);
    let input = gpu.buffer(bytemuck::cast_slice(&inputs), BufferUsages::STORAGE);
    let output = gpu.buffer(
        bytemuck::cast_slice(&vec![0f32; lanes]),
        BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    );
    let read = gpu.device.create_buffer(&BufferDescriptor {
        label: None,
        size: (lanes * 4) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = gpu.bind(&pipeline, &[&params, &input, &output]);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(cases.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &read, 0, (lanes * 4) as u64);
    gpu.queue.submit([encoder.finish()]);
    read.slice(..).map_async(MapMode::Read, |_| {});
    gpu.device.poll(PollType::wait_indefinitely()).unwrap();
    let values = bytemuck::cast_slice(&read.slice(..).get_mapped_range()).to_vec();
    read.unmap();
    values
}

#[derive(Default, Debug)]
struct Counts {
    lanes: usize,
    twin_not_finite: usize,
    twin_equal: usize,
    twin_flushed: usize,
    twin_differs: usize,
    scalar_bit_equal: usize,
    scalar_within: usize,
    scalar_beyond_at_discontinuity: usize,
    scalar_beyond_inexact_discontinuity: usize,
    scalar_beyond_unexplained: usize,
    scalar_not_finite: usize,
}

#[test]
#[ignore = "needs a GPU adapter; run with --release"]
fn gpu_results_have_the_bits_of_the_twin_and_agree_with_the_scalar_vm() {
    let Some(gpu) = Gpu::new() else {
        panic!("no adapter");
    };
    let count = std::env::var("MCRS_GPU_FUZZ_PROGRAMS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(10_000);
    let mut census = BTreeMap::new();
    let cases = generate(count, &mut census);
    let mut counts = Counts::default();
    let mut twin = Twin::new();
    let mut twin_findings = Vec::new();
    let mut findings = Vec::new();
    for batch in cases.chunks(BATCH) {
        let source = kernel(batch);
        gpu::validate(&source).unwrap_or_else(|error| panic!("naga: {error}"));
        let values = run(&gpu, &source, batch);
        for (case_index, case) in batch.iter().enumerate() {
            for (lane, inputs) in case.inputs.iter().enumerate() {
                counts.lanes += 1;
                let gpu_value = values[case_index * LANES + lane];
                let twin_run = twin.run(&case.program, &inputs[..case.program.inputs().len()]);
                if !twin_run.stayed_finite {
                    counts.twin_not_finite += 1;
                    continue;
                }
                if gpu_value.to_bits() == twin_run.value.to_bits() {
                    counts.twin_equal += 1;
                } else if same_or_flushed(gpu_value, twin_run.value) {
                    counts.twin_flushed += 1;
                } else {
                    counts.twin_differs += 1;
                    twin_findings.push(format!(
                        "twin: {} inputs {:?}: gpu {gpu_value:e} twin {:e}",
                        case.text,
                        &inputs[..case.program.inputs().len()],
                        twin_run.value
                    ));
                }
                let reference = scalar(case, inputs);
                if !reference.is_finite() {
                    counts.scalar_not_finite += 1;
                    continue;
                }
                let clamped = pose_clamp(reference);
                if clamped.to_bits() == gpu_value.to_bits() {
                    counts.scalar_bit_equal += 1;
                } else if same_or_flushed(clamped, gpu_value) || close(clamped, gpu_value) {
                    counts.scalar_within += 1;
                } else if at_discontinuity(case, inputs, reference, gpu_value) {
                    counts.scalar_beyond_at_discontinuity += 1;
                } else if inexact_discontinuity(case, inputs) {
                    counts.scalar_beyond_inexact_discontinuity += 1;
                } else {
                    counts.scalar_beyond_unexplained += 1;
                    findings.push(format!(
                        "scalar: {} inputs {:?}: gpu {gpu_value:e} scalar {reference:e}",
                        case.text,
                        &inputs[..case.program.inputs().len()]
                    ));
                }
            }
        }
    }
    eprintln!("adapter: {}", gpu.name);
    eprintln!(
        "programs: {} pure of {} generated",
        cases.len(),
        census.values().sum::<usize>()
    );
    eprintln!("classes: {census:?}");
    eprintln!("{counts:#?}");
    for finding in twin_findings.iter().chain(findings.iter().take(40)) {
        eprintln!("{finding}");
    }
    assert_eq!(counts.twin_differs, 0, "the GPU and the twin differ");
    assert_eq!(
        counts.scalar_beyond_unexplained, 0,
        "the GPU leaves the pose tolerance on a well-conditioned input"
    );
}
