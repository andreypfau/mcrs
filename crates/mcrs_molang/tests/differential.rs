//! Generated programs from the grammar, checked against independent evaluations: the batch VM
//! against the scalar VM, the optimised program against the direct lowering, and the scalar VM
//! against molangjs.

#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]
#![expect(
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "the case count comes from the environment of proptest"
)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use generator::{NUMBER_QUERIES, Profile, VARIABLES, program, registry};
use mcrs_molang::{
    BatchOutput, BatchQueries, BatchSubjects, BatchVariables, BatchVm, CompileOptions, Expression,
    Milestone, QueryRegistry, QuerySlot, QueryValues, ScalarVm, Source, Subject, Value, ValueType,
    VariableLayout, Variables, compile, string_hash,
};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestCaseError, TestRunner};
use serde::{Deserialize, Serialize};

mod generator;

const LANES: usize = 64;
/// A small deterministic generator for the inputs of the lanes.
struct Inputs(u64);

impl Inputs {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn pick<T: Copy>(&mut self, pool: &[T]) -> T {
        pool[(self.next() % pool.len() as u64) as usize]
    }

    fn number(&mut self, profile: Profile) -> Option<f32> {
        let full = [
            Some(0.0),
            Some(-0.0),
            Some(1.0),
            Some(-1.0),
            Some(0.5),
            Some(2.0),
            Some(90.0),
            Some(-180.0),
            Some(1e30),
            Some(f32::INFINITY),
            Some(f32::NEG_INFINITY),
            Some(f32::NAN),
            Some(3.3),
            Some(1e-40),
            None,
        ];
        let js = [
            Some(0.0),
            Some(1.0),
            Some(-1.0),
            Some(2.0),
            Some(0.5),
            Some(3.0),
            None,
        ];
        match profile {
            Profile::Full | Profile::Gpu => self.pick(&full),
            Profile::MolangJs => self.pick(&js),
        }
    }
}

/// The inputs of one lane.
#[derive(Clone, Debug)]
struct Lane {
    variables: Vec<(&'static str, Option<f32>)>,
    queries: Vec<(&'static str, Option<f32>)>,
    string_query: Option<&'static str>,
    this: f32,
}

impl Lane {
    fn generate(inputs: &mut Inputs, profile: Profile) -> Self {
        Self {
            variables: VARIABLES
                .iter()
                .map(|name| (*name, inputs.number(profile)))
                .collect(),
            queries: NUMBER_QUERIES
                .iter()
                .map(|name| (*name, inputs.number(profile)))
                .collect(),
            string_query: inputs.pick(&[Some("x"), Some("Y"), None]),
            this: inputs.number(profile).unwrap_or(0.0),
        }
    }

    fn variables(&self, layout: &VariableLayout) -> Variables {
        let mut variables = Variables::new(layout);
        for (name, value) in &self.variables {
            if let Some(slot) = layout.slot(name) {
                variables.set(slot, value.map(|v| u64::from(v.to_bits())));
            }
        }
        variables
    }

    fn queries(&self, registry: &QueryRegistry) -> QueryValues {
        let mut values = QueryValues::new(registry.slot_count());
        for (slot, _) in registry.slots() {
            values.set(slot, query_raw(slot, registry, self));
        }
        values
    }
}

fn same_raw(a: Value, b: Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
        }
        _ => a == b,
    }
}

fn same_number_bits(x: f32, y: f32) -> bool {
    x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
}

struct Compiled {
    layout: VariableLayout,
    registry: QueryRegistry,
    reference: Expression,
    optimized: Expression,
}

fn compile_both(text: &str) -> Result<Compiled, TestCaseError> {
    let mut layout = VariableLayout::new();
    let mut registry = registry();
    let mut build = |optimize| {
        let options = CompileOptions {
            milestone: Milestone::Later,
            optimize,
            resources: None,
        };
        compile(&Source::Text(text), &mut layout, &mut registry, &options)
            .map(|compiled| compiled.expression)
            .map_err(|errors| TestCaseError::fail(format!("{text}: {errors:?}")))
    };
    let reference = build(false)?;
    let optimized = build(true)?;
    Ok(Compiled {
        layout,
        registry,
        reference,
        optimized,
    })
}

fn scalar(expression: &Expression, compiled: &Compiled, lane: &Lane) -> (Value, Variables) {
    let mut variables = lane.variables(&compiled.layout);
    let queries = lane.queries(&compiled.registry);
    let mut subject = Subject {
        variables: &mut variables,
        queries: &queries,
        this: lane.this,
    };
    let value = expression.evaluate(&mut ScalarVm::new(), &mut subject);
    (value, variables)
}

/// `active` marks the lanes that run; the other lanes must keep their variables.
fn check_batch_and_passes(text: &str, seed: u64, active: u64) -> Result<(), TestCaseError> {
    let compiled = compile_both(text)?;
    let mut inputs = Inputs(seed);
    let lane_inputs: Vec<Lane> = (0..LANES)
        .map(|_| Lane::generate(&mut inputs, Profile::Full))
        .collect();

    let mut variables = BatchVariables::<LANES>::new(&compiled.layout);
    let mut queries = BatchQueries::<LANES>::new(compiled.registry.slot_count());
    let mut this = [0.0f32; LANES];
    for (index, lane) in lane_inputs.iter().enumerate() {
        let scalar_variables = lane.variables(&compiled.layout);
        for (_, slot, _) in compiled.layout.iter() {
            variables.set(index, slot, scalar_variables.get(slot));
        }
        for (slot, _) in compiled.registry.slots() {
            queries.set(index, slot, query_raw(slot, &compiled.registry, lane));
        }
        this[index] = lane.this;
    }

    let initial_variables = variables.clone();
    let mut lanes = [false; LANES];
    for (index, lane) in lanes.iter_mut().enumerate() {
        *lane = active >> index & 1 == 1;
    }
    let batch_results: Vec<Value> = match &compiled.optimized {
        Expression::Constant(value) => vec![*value; LANES],
        Expression::Program(program) => {
            let mut vm = BatchVm::<LANES>::new();
            let mut subjects = BatchSubjects {
                variables: &mut variables,
                queries: &queries,
                this: &this,
                lanes: 0,
            };
            match vm.run_masked(program, &mut subjects, &lanes) {
                BatchOutput::Numbers(numbers) => {
                    numbers.iter().map(|n| Value::Number(*n)).collect()
                }
                BatchOutput::Bits(bits, value_type) => bits
                    .iter()
                    .map(|raw| match value_type {
                        ValueType::String => Value::String(*raw),
                        _ => Value::Number(0.0),
                    })
                    .collect(),
            }
        }
    };

    for (index, lane) in lane_inputs.iter().enumerate() {
        if !lanes[index] {
            for (name, slot, _) in compiled.layout.iter() {
                prop_assert_eq!(
                    variables.get(index, slot),
                    initial_variables.get(index, slot),
                    "the inactive lane {} changed v.{}",
                    index,
                    name
                );
            }
            continue;
        }
        let (reference, reference_variables) = scalar(&compiled.reference, &compiled, lane);
        let (optimized, optimized_variables) = scalar(&compiled.optimized, &compiled, lane);
        prop_assert!(
            same_raw(reference, optimized),
            "passes change the result of lane {index}: {reference:?} / {optimized:?}\n{lane:?}"
        );
        prop_assert!(
            same_raw(reference, batch_results[index]),
            "batch differs in lane {index}: {reference:?} / {:?}\n{lane:?}",
            batch_results[index]
        );
        for (name, slot, _) in compiled.layout.iter() {
            let want = reference_variables.get(slot);
            for (source, got) in [
                ("passes", optimized_variables.get(slot)),
                ("batch", variables.get(index, slot)),
            ] {
                let equal = match (want, got) {
                    (Some(a), Some(b)) => {
                        same_number_bits(f32::from_bits(a as u32), f32::from_bits(b as u32))
                    }
                    (None, None) => true,
                    _ => false,
                };
                prop_assert!(
                    equal,
                    "{source}: v.{name} of lane {index}: {want:?} / {got:?}\n{lane:?}"
                );
            }
        }
    }
    Ok(())
}

fn query_raw(slot: QuerySlot, registry: &QueryRegistry, lane: &Lane) -> Option<u64> {
    let (_, key) = registry.slots().find(|(candidate, _)| *candidate == slot)?;
    let declaration = registry.declaration(key.declaration)?;
    if &*declaration.name == "sq" {
        return lane.string_query.map(string_hash);
    }
    lane.queries
        .iter()
        .find(|(name, _)| *name == &*declaration.name)
        .and_then(|(_, value)| value.map(|v| u64::from(v.to_bits())))
}

fn cases(default: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(default)
}

#[test]
fn batch_equals_scalar_and_passes_keep_every_bit() {
    let mut runner = TestRunner::new(Config {
        cases: cases(2000),
        ..Config::default()
    });
    let strategy = (program(Profile::Full), any::<u64>(), any::<u64>());
    let result = runner.run(&strategy, |(text, seed, active)| {
        check_batch_and_passes(&text, seed, active)
    });
    if let Err(error) = result {
        panic!("{error}");
    }
}

struct Node {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Node {
    fn start() -> Option<Self> {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize().ok()?;
        let runner = crate_dir.join("../../tools/molang-diff/runner.mjs");
        let mut child = Command::new("node")
            .arg(runner)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;
        let input = child.stdin.take()?;
        let output = BufReader::new(child.stdout.take()?);
        Some(Self {
            child,
            input,
            output,
        })
    }

    fn evaluate(&mut self, case: &Case<'_>) -> Answer {
        let line = serde_json::to_string(case).unwrap();
        writeln!(self.input, "{line}").unwrap();
        self.input.flush().unwrap();
        let mut answer = String::new();
        self.output.read_line(&mut answer).unwrap();
        serde_json::from_str(&answer).unwrap()
    }
}

/// One input line of `tools/molang-diff/runner.mjs`.
#[derive(Serialize)]
struct Case<'a> {
    expr: &'a str,
    variables: BTreeMap<String, Option<f32>>,
    queries: BTreeMap<String, f32>,
}

/// One output line of the runner.
#[derive(Debug, Deserialize)]
struct Answer {
    value: Option<JsNumber>,
    #[serde(default)]
    variables: BTreeMap<String, JsNumber>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JsNumber {
    Number(f64),
    Text(String),
}

impl JsNumber {
    fn value(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(*number),
            Self::Text(text) => match text.as_str() {
                "NaN" => Some(f64::NAN),
                "Infinity" => Some(f64::INFINITY),
                "-Infinity" => Some(f64::NEG_INFINITY),
                "-0" => Some(0.0),
                _ => None,
            },
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// molangjs gives `0` for a NaN and a `-0` result of the whole expression.
fn normalise(value: f32) -> f64 {
    if value.is_nan() || value == 0.0 {
        0.0
    } else {
        f64::from(value)
    }
}

fn close(ours: f64, theirs: f64) -> bool {
    let scale = ours.abs().max(theirs.abs()).max(1.0);
    (ours - theirs).abs() <= 2e-3 * scale
}

fn assigned_variables(text: &str) -> Vec<&'static str> {
    VARIABLES
        .iter()
        .copied()
        .filter(|name| text.contains(&format!("v.{name} =")))
        .collect()
}

fn check_molangjs(node: &RefCell<Node>, text: &str, seed: u64) -> Result<(), TestCaseError> {
    let compiled = compile_both(text)?;
    let lane = Lane::generate(&mut Inputs(seed), Profile::MolangJs);
    let (value, variables) = scalar(&compiled.optimized, &compiled, &lane);
    let Value::Number(value) = value else {
        return Err(TestCaseError::reject("not a number"));
    };
    let mut variables_in = BTreeMap::new();
    for (name, value) in &lane.variables {
        variables_in.insert(format!("v.{name}"), *value);
    }
    variables_in.insert("this".to_owned(), Some(lane.this));
    let mut queries_in = BTreeMap::new();
    for (name, value) in &lane.queries {
        if let Some(value) = value {
            queries_in.insert((*name).to_owned(), *value);
        }
    }
    let case = Case {
        expr: text,
        variables: variables_in,
        queries: queries_in,
    };
    let answer = node.borrow_mut().evaluate(&case);
    let Some(theirs) = answer.value.as_ref().and_then(JsNumber::value) else {
        return Err(TestCaseError::fail(format!("molangjs: {:?}", answer.error)));
    };
    prop_assert!(
        close(normalise(value), theirs),
        "result {value} / molangjs {theirs}\n{lane:?}"
    );
    for name in assigned_variables(text) {
        let Some(slot) = compiled.layout.slot(name) else {
            continue;
        };
        let ours = variables.get(slot).map(|raw| f32::from_bits(raw as u32));
        let theirs = answer
            .variables
            .get(&format!("variable.{name}"))
            .and_then(JsNumber::value);
        let initial = lane
            .variables
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .and_then(|(_, value)| *value);
        let theirs = theirs.or(initial.map(f64::from));
        let ours = ours.map(normalise);
        let equal = match (ours, theirs) {
            (Some(a), Some(b)) => close(a, b),
            (None, None) => true,
            (None, Some(b)) | (Some(b), None) => b == 0.0,
        };
        prop_assert!(equal, "v.{name}: {ours:?} / molangjs {theirs:?}\n{lane:?}");
    }
    Ok(())
}

#[test]
#[ignore = "needs node and `npm ci` in tools/molang-diff"]
fn scalar_agrees_with_molangjs() {
    let Some(node) = Node::start() else {
        panic!("node does not start");
    };
    let node = RefCell::new(node);
    let mut runner = TestRunner::new(Config {
        cases: cases(2000),
        ..Config::default()
    });
    let strategy = (program(Profile::MolangJs), any::<u64>());
    let result = runner.run(&strategy, |(text, seed)| check_molangjs(&node, &text, seed));
    if let Err(error) = result {
        panic!("{error}");
    }
}
