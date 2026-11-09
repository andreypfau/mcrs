//! The processor class of a channel expression (`docs/molang.md`, section 10.1).

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]

use mcrs_molang::{
    Arguments, ChannelClass, CompileOptions, InputRange, Milestone, PureInput, QueryDeclaration,
    QueryRegistry, QuerySlot, Rate, StateReason, UseContext, ValueType, VariableLayout,
    compile_text,
};

/// `q.time` is a GPU input of the range `[0, 1e9]`, `q.walk` one with no known range, `q.load`
/// a value of the pack load, and `q.name` a string that is not a GPU input.
fn classify(text: &str) -> ChannelClass {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for (name, result, rate) in [
        ("time", ValueType::Number, Rate::AnimationFrame),
        ("walk", ValueType::Number, Rate::EntityFrame),
        ("load", ValueType::Number, Rate::Load),
        ("name", ValueType::String, Rate::EntityTick),
        ("cpu", ValueType::Number, Rate::EntityTick),
    ] {
        registry.declare(QueryDeclaration::entity(
            name,
            result,
            Arguments::None,
            rate,
        ));
    }
    let options = CompileOptions {
        milestone: Milestone::Later,
        ..CompileOptions::default()
    };
    let compiled = compile_text(text, &mut VariableLayout::new(), &mut registry, &options).unwrap();
    compiled.expression.classify(|slot| {
        let (_, key) = registry.slots().find(|(known, _)| *known == slot)?;
        match &*registry.declaration(key.declaration)?.name {
            "time" => Some(InputRange::new(0.0, 1e9)),
            "walk" => Some(InputRange::FINITE),
            _ => None,
        }
    })
}

fn reason(text: &str) -> StateReason {
    match classify(text) {
        ChannelClass::State(reason) => reason,
        other => panic!("{text}: {other:?}"),
    }
}

#[test]
fn a_channel_over_gpu_inputs_is_pure() {
    let ChannelClass::Pure(program) = classify("math.cos(q.walk * 38.17) * 80 * q.time") else {
        panic!("not pure");
    };
    assert_eq!(
        program.inputs(),
        [
            PureInput::Query(QuerySlot(0)),
            PureInput::Query(QuerySlot(1))
        ]
    );
    for text in [
        "v.speed * math.sin(q.time * 90)",
        "this + (q.walk > 0 ? q.walk : -q.walk)",
        "t.a = q.walk; (t.a > 1) ? { return t.a * 2; }; return t.a;",
        "math.sqrt(q.time)",
        "math.ln(q.time + 1)",
        "math.ln(math.abs(q.walk) + 0.25)",
        "math.asin(math.clamp(q.walk, -1, 1))",
        "math.pow(math.clamp(q.walk, 0.5, 4), math.clamp(q.walk, -8, 8))",
        "math.exp(math.min(q.walk, 80))",
        "math.ease_in_circ(0, 1, math.clamp(q.walk, 0, 1))",
        "math.ease_in_out_expo(0, 1, math.clamp(q.walk, 0, 1))",
        "math.ease_in_elastic(0, 1, math.clamp(q.walk, 0, 1))",
    ] {
        assert!(
            matches!(classify(text), ChannelClass::Pure(_)),
            "{text} is pure"
        );
    }
}

#[test]
fn a_channel_that_needs_the_cpu_states_why() {
    assert_eq!(reason("q.name == 'a'"), StateReason::String);
    assert_eq!(reason("v.x = 2; return q.walk * v.x;"), StateReason::Write);
    assert_eq!(reason("v.x ?? 1"), StateReason::MissingValue);
    assert_eq!(reason("q.cpu * 2"), StateReason::NotGpuInput(QuerySlot(0)));
    assert_eq!(
        reason("q.walk + math.ln(0)"),
        StateReason::NonFiniteConstant
    );
    for text in [
        "math.sqrt(q.walk)",
        "math.ln(q.time)",
        "math.exp(q.time)",
        "math.asin(q.walk)",
        "math.acos(q.walk * 0.5)",
        "math.pow(q.walk, 2)",
        "math.pow(math.clamp(q.walk, 2, 4), math.clamp(q.walk, 0, 200))",
        "math.ease_in_circ(0, 1, q.walk)",
        "math.ease_in_out_circ(0, 1, q.walk)",
        "math.ease_in_expo(0, 1, q.walk)",
        "math.ease_out_elastic(0, 1, q.walk)",
    ] {
        assert!(
            matches!(reason(text), StateReason::DomainRisk(_)),
            "{text} has a domain risk"
        );
    }
}

#[test]
fn a_channel_of_the_load_is_constant() {
    for text in ["1 + 2", "q.load * 3", "math.cos(q.load)"] {
        assert_eq!(classify(text), ChannelClass::Constant, "{text}");
    }
}
