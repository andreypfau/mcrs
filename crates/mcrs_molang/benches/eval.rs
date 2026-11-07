#![expect(clippy::panic, reason = "a benchmark with a broken setup stops")]
#![expect(clippy::unwrap_used, reason = "a benchmark with a broken setup stops")]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mcrs_molang::{
    Arguments, BatchQueries, BatchSubjects, BatchVariables, BatchVm, CompileOptions, Expression,
    Milestone, Program, QueryDeclaration, QueryRegistry, QueryScope, QueryValues, Rate, ScalarVm,
    Subject, UseContext, ValueType, VariableLayout, Variables, compile_text,
};

const LANES: usize = 256;

const WALK: &str = "math.cos(q.modified_distance_moved * 38.17) * 80.0 * q.modified_move_speed";

/// About 30 syntax nodes: a clamp, a conditional, trigonometry, variables and short-circuit logic.
const MIXED: &str = "math.clamp(q.life_time * 2.5 - v.offset, 0, 1) * (q.is_baby ? 0.5 : 1.0) \
    + math.sin(q.anim_time * 90 + v.phase) * 15 \
    - ((v.a > 0.5 && q.is_on_ground) ? v.a * 3 : 0)";

const QUERIES: [&str; 6] = [
    "modified_distance_moved",
    "modified_move_speed",
    "life_time",
    "is_baby",
    "anim_time",
    "is_on_ground",
];

struct Setup {
    program: Program,
    layout: VariableLayout,
    registry: QueryRegistry,
}

fn setup(text: &str) -> Setup {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for name in QUERIES {
        registry.declare(QueryDeclaration {
            name: name.into(),
            arguments: Arguments::None,
            result: ValueType::Number,
            rate: Rate::EntityFrame,
            scope: QueryScope::Entity,
            milestone: Milestone::One,
        });
    }
    let mut layout = VariableLayout::new();
    let options = CompileOptions {
        milestone: Milestone::Two,
        ..CompileOptions::default()
    };
    let compiled = compile_text(text, &mut layout, &mut registry, &options).unwrap();
    let Expression::Program(program) = compiled.expression else {
        panic!("{text} folds to a constant");
    };
    Setup {
        program,
        layout,
        registry,
    }
}

fn lane_value(lane: usize, salt: usize) -> f32 {
    ((lane * 7 + salt * 13) % 17) as f32 * 0.37
}

fn bench_expression(c: &mut Criterion, name: &str, text: &str) {
    let setup = setup(text);

    let mut variables = Variables::new(&setup.layout);
    for (index, (_, slot, _)) in setup.layout.iter().enumerate() {
        variables.set(slot, Some(u64::from(lane_value(0, index).to_bits())));
    }
    let mut queries = QueryValues::new(setup.registry.slot_count());
    for (slot, _) in setup.registry.slots() {
        queries.set_number(slot, lane_value(1, usize::from(slot.0)));
    }
    let mut vm = ScalarVm::new();
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Elements(1));
    group.bench_function("scalar", |b| {
        b.iter(|| {
            let mut subject = Subject {
                variables: &mut variables,
                queries: &queries,
                this: 0.0,
            };
            black_box(vm.run(black_box(&setup.program), &mut subject))
        });
    });

    let mut batch_variables = BatchVariables::<LANES>::new(&setup.layout);
    let mut batch_queries = BatchQueries::<LANES>::new(setup.registry.slot_count());
    for lane in 0..LANES {
        for (index, (_, slot, _)) in setup.layout.iter().enumerate() {
            batch_variables.set(
                lane,
                slot,
                Some(u64::from(lane_value(lane, index).to_bits())),
            );
        }
        for (slot, _) in setup.registry.slots() {
            batch_queries.set_number(lane, slot, lane_value(lane, usize::from(slot.0) + 5));
        }
    }
    let this = [0.0f32; LANES];
    let mut batch = BatchVm::<LANES>::new();
    group.throughput(Throughput::Elements(LANES as u64));
    group.bench_function("batch_256", |b| {
        b.iter(|| {
            let mut subjects = BatchSubjects {
                variables: &mut batch_variables,
                queries: &batch_queries,
                this: &this,
                lanes: LANES,
            };
            let output = batch.run(black_box(&setup.program), &mut subjects);
            black_box(matches!(output, mcrs_molang::BatchOutput::Numbers(_)));
        });
    });
    group.finish();
}

fn walk(c: &mut Criterion) {
    bench_expression(c, "walk", WALK);
}

fn mixed(c: &mut Criterion) {
    bench_expression(c, "mixed_30_nodes", MIXED);
}

criterion_group!(benches, walk, mixed);
criterion_main!(benches);
