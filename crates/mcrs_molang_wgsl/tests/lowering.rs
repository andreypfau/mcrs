#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]

use mcrs_molang::{
    Arguments, ChannelClass, CompileOptions, InputRange, Milestone, PureProgram, QueryDeclaration,
    QueryRegistry, QueryScope, Rate, UseContext, ValueType, VariableLayout, compile_text,
};

fn pure(text: &str) -> PureProgram {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for name in ["qa", "qb"] {
        registry.declare(QueryDeclaration {
            name: name.into(),
            arguments: Arguments::None,
            result: ValueType::Number,
            rate: Rate::EntityFrame,
            scope: QueryScope::Entity,
            milestone: Milestone::One,
        });
    }
    let options = CompileOptions {
        milestone: Milestone::Later,
        ..CompileOptions::default()
    };
    let compiled = compile_text(text, &mut VariableLayout::new(), &mut registry, &options).unwrap();
    match compiled.expression.classify(|_| Some(InputRange::FINITE)) {
        ChannelClass::Pure(program) => program,
        other => panic!("{text}: {other:?}"),
    }
}

/// Every operator, function and control form, so that naga sees each WGSL call that the lowering
/// can write.
const EVERY_FORM: [&str; 12] = [
    "-q.qa + !q.qb - (q.qa * q.qb) / v.a",
    "(q.qa < q.qb) + (q.qa <= q.qb) + (q.qa > q.qb) + (q.qa >= q.qb) + (q.qa == q.qb) + (q.qa != q.qb)",
    "(q.qa && q.qb) + (q.qa || q.qb) + q.approx_eq(q.qa, q.qb) + q.in_range(q.qa, 0, 1)",
    "math.abs(q.qa) + math.ceil(q.qa) + math.floor(q.qa) + math.trunc(q.qa) + math.round(q.qa)",
    "math.sqrt(math.abs(q.qa)) + math.exp(math.clamp(q.qa, -5, 5)) + math.ln(math.abs(q.qa) + 1)",
    "math.sin(q.qa) + math.cos(q.qa) + math.asin(math.clamp(q.qa, -1, 1)) + math.acos(math.clamp(q.qa, -1, 1))",
    "math.atan(q.qa) + math.atan2(q.qa, q.qb) + math.sign(q.qa) + math.min_angle(q.qa) + math.hermite_blend(q.qa)",
    "math.min(q.qa, q.qb) + math.max(q.qa, q.qb) + math.mod(q.qa, q.qb) + math.copy_sign(q.qa, q.qb)",
    "math.pow(math.clamp(q.qa, 1, 2), math.clamp(q.qb, -3, 3)) + math.lerp(q.qa, q.qb, this)",
    "math.inverse_lerp(q.qa, q.qb, this) + math.lerprotate(q.qa, q.qb, this) + math.ease_in_out_elastic(q.qa, q.qb, math.clamp(this, 0, 1))",
    "math.ease_out_expo(0, 1, math.clamp(q.qa, 0, 1)) + math.ease_in_circ(0, 1, math.clamp(q.qa, 0, 1)) + math.ease_in_out_bounce(q.qa, q.qb, this)",
    "t.x = q.qa > 0 ? math.sin(q.qb) : math.cos(q.qb); (q.qb > 1) ? { return t.x * 2; }; return q.qa ? t.x : 3;",
];

#[test]
fn the_library_and_every_lowered_form_validate_with_naga() {
    let mut source = String::from(mcrs_molang_wgsl::LIBRARY);
    for (index, text) in EVERY_FORM.iter().enumerate() {
        source.push_str(&mcrs_molang_wgsl::lower(
            &pure(text),
            &format!("channel_{index}"),
        ));
    }
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
}
