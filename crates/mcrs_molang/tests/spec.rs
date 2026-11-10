#![expect(clippy::panic, reason = "a test reports a failure by panicking")]
#![expect(
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

use mcrs_molang::{
    Arguments, CompileOptions, ErrorKind, Milestone, QueryDeclaration, QueryRegistry, QuerySlot,
    QueryValues, Rate, ScalarVm, Source, Subject, UseContext, Value, ValueType, VariableLayout,
    Variables, WarningKind, compile, compile_text, string_hash,
};

#[expect(dead_code, reason = "this test reads the table, not the registry")]
mod entity_queries;

use entity_queries::{ENTITY_QUERIES, PoseInput};

struct Run {
    value: Value,
    layout: VariableLayout,
    variables: Variables,
}

impl Run {
    fn variable(&self, name: &str) -> Option<f32> {
        let slot = self.layout.slot(name)?;
        self.variables
            .get(slot)
            .map(|raw| f32::from_bits(raw as u32))
    }
}

fn options(milestone: Milestone) -> CompileOptions<'static> {
    CompileOptions {
        milestone,
        ..CompileOptions::default()
    }
}

fn run_with(source: &Source<'_>, optimize: bool) -> Run {
    let mut layout = VariableLayout::new();
    let mut host = QueryRegistry::new(UseContext::Entity);
    let options = CompileOptions {
        optimize,
        ..options(Milestone::Later)
    };
    let compiled = compile(source, &mut layout, &mut host, &options)
        .unwrap_or_else(|errors| panic!("{source:?}: {errors:?}"));
    let mut variables = Variables::new(&layout);
    let queries = QueryValues::new(host.slot_count());
    let mut subject = Subject {
        variables: &mut variables,
        queries: &queries,
        this: 0.0,
    };
    let value = compiled
        .expression
        .evaluate(&mut ScalarVm::new(), &mut subject);
    Run {
        value,
        layout,
        variables,
    }
}

/// Runs the direct lowering and the optimised program, which must agree bit for bit.
fn run_source(source: &Source<'_>) -> Run {
    let reference = run_with(source, false);
    let optimized = run_with(source, true);
    assert_eq!(
        reference.value.to_raw(),
        optimized.value.to_raw(),
        "{source:?}: the passes change the result"
    );
    assert_eq!(reference.variables, optimized.variables, "{source:?}");
    reference
}

fn run(text: &str) -> Run {
    run_source(&Source::Text(text))
}

fn number(text: &str) -> f32 {
    match run(text).value {
        Value::Number(value) => value,
        other => panic!("{text}: {other:?}"),
    }
}

fn error(text: &str, milestone: Milestone) -> ErrorKind {
    let mut host = QueryRegistry::new(UseContext::Entity);
    let errors = compile_text(
        text,
        &mut VariableLayout::new(),
        &mut host,
        &options(milestone),
    )
    .err()
    .unwrap_or_else(|| panic!("{text} compiles"));
    errors[0].kind
}

#[test]
fn precedence_follows_the_modern_table() {
    for (text, expected) in [
        ("0 && 1 || 1", 1.0),
        ("1 || 1 && 0", 1.0),
        ("1 < 2 == 3 > 2", 1.0),
        ("0 ? 1 : 1 ? 2 : 3", 2.0),
        ("1 ? 0 ? 5 : 6 : 7", 6.0),
        ("-2 * -3 + 1", 7.0),
        ("10 - 4 - 3", 3.0),
        ("8 / 4 / 2", 1.0),
        ("!0 + 1", 2.0),
    ] {
        assert_eq!(number(text), expected, "{text}");
    }
    assert_eq!(run("v.x = 1 ? 5 : 6").variable("x"), Some(5.0));
}

#[test]
fn division_by_zero_gives_zero() {
    for (text, expected) in [
        ("1 / 0", 0.0),
        ("0 / 0", 0.0),
        ("1 / -0", 0.0),
        ("5 / -1", -5.0),
    ] {
        assert_eq!(number(text).to_bits(), f32::to_bits(expected), "{text}");
    }
}

#[test]
fn nan_is_truthy_and_compares_unequal() {
    for (text, expected) in [
        ("math.sqrt(-1) ? 1 : 2", 1.0),
        ("!math.sqrt(-1)", 0.0),
        ("math.sqrt(-1) == math.sqrt(-1)", 0.0),
        ("math.sqrt(-1) != math.sqrt(-1)", 1.0),
        ("math.sqrt(-1) < 1", 0.0),
        ("math.sqrt(-1) >= 1", 0.0),
        ("math.sqrt(-1) && 1", 1.0),
        ("math.min(math.sqrt(-1), 3)", 3.0),
    ] {
        assert_eq!(number(text), expected, "{text}");
    }
    assert!(number("math.sqrt(-1) + 1").is_nan());
    assert_eq!(number("math.ln(0)"), f32::NEG_INFINITY);
    assert_eq!(number("-(0)").to_bits(), (-0.0f32).to_bits());
}

#[test]
fn trigonometry_takes_degrees_and_reduces_exactly() {
    for (text, expected) in [
        ("math.cos(180)", -1.0),
        ("math.sin(90)", 1.0),
        ("math.sin(3600090)", 1.0),
        ("math.cos(-3600180)", -1.0),
        ("math.atan2(1, 0)", 90.0),
        ("math.asin(1)", 90.0),
    ] {
        let value = number(text);
        assert!(
            (value - expected).abs() <= 1.0 / 2048.0,
            "{text} gives {value}"
        );
    }
}

#[test]
fn exact_functions_follow_their_formulas() {
    for (text, expected) in [
        ("math.round(-2.5)", -3.0),
        ("math.round(2.5)", 3.0),
        ("math.mod(-7, 3)", -1.0),
        ("math.mod(5, 0)", 0.0),
        ("math.sign(0)", -1.0),
        ("math.copy_sign(3, -0)", -3.0),
        ("math.inverse_lerp(2, 2, 3)", 0.0),
        ("math.min_angle(180)", -180.0),
        ("math.lerprotate(10, 350, 0.25)", 5.0),
        ("math.clamp(5, 10, 0)", 0.0),
        ("math.hermite_blend(0.5)", 0.5),
        ("math.lerp(2, 4, 1.5)", 5.0),
        ("math.ease_in_bounce(0, 1, 0.25)", 0.027_343_75),
    ] {
        assert_eq!(number(text), expected, "{text}");
    }
    for (text, expected) in [
        ("math.min(-0, 0)", -0.0f32),
        ("math.min(0, -0)", 0.0),
        ("math.max(-0, 0)", -0.0),
        ("math.max(0, -0)", 0.0),
    ] {
        assert_eq!(number(text).to_bits(), expected.to_bits(), "{text}");
    }
}

#[test]
fn strings_compare_with_case_and_whitespace() {
    for (text, expected) in [
        ("'a' == 'A'", 0.0),
        ("'Ab' == 'Ab'", 1.0),
        ("'a b' == 'ab'", 0.0),
        ("'text' == 1", 0.0),
        ("'text' != 1", 1.0),
        ("'' == ''", 1.0),
    ] {
        assert_eq!(number(text), expected, "{text}");
    }
    assert_eq!(run("'Pig'").value, Value::String(string_hash("Pig")));
    assert_eq!(error("'text' + 1", Milestone::Later), ErrorKind::StaticType);
    for text in [
        "q.x ? 'a' : 1",
        "q.x ? 'a'",
        "v.s = 'a'; v.s = 1;",
        "v.c ? { return 'a'; }; return 1;",
    ] {
        assert_eq!(
            error(text, Milestone::Later),
            ErrorKind::StaticType,
            "{text}"
        );
    }
    assert_eq!(run("1 ? 'a' : 'b'").value, Value::String(string_hash("a")));
}

#[test]
fn statement_lists_give_zero_without_return() {
    for (text, expected) in [
        ("v.x = 1; v.x * 2", 0.0),
        ("v.x = 1; return v.x * 2;", 2.0),
        ("1;", 0.0),
        ("1", 1.0),
        ("return 2", 2.0),
        (";;", 0.0),
        ("", 0.0),
        ("{ return 3; }; return 4;", 3.0),
        ("t.a = 2; { t.a = t.a * 3; }; return t.a;", 6.0),
    ] {
        assert_eq!(number(text), expected, "{text}");
    }
    let assignment = run("v.x = 1");
    assert_eq!(assignment.value, Value::Number(0.0));
    assert_eq!(assignment.variable("x"), Some(1.0));
}

#[test]
fn coalescing_selects_the_right_side_for_a_missing_read_at_any_depth() {
    for (text, expected) in [
        ("v.x = 0; return v.x ?? 5;", 0.0),
        ("(v.unset + 1) ?? 5", 5.0),
        ("v.unset ?? 5", 5.0),
        ("q.unknown_name(1) ?? 5", 5.0),
        ("(q.unknown_name ?? 1) ? 0 : 1", 0.0),
        ("t.a ?? 4", 4.0),
        ("t.a = 1; return t.a ?? 4;", 1.0),
        ("(1 ? 2 : v.unset) ?? 5", 2.0),
        ("(0 ? 2 : v.unset) ?? 5", 5.0),
        ("(v.unset ?? 2) + (v.other ?? 3)", 5.0),
        ("((v.unset ?? 2) + v.other) ?? 7", 7.0),
        ("v.y = v.unset; return v.y ?? 5;", 5.0),
        ("v.y = v.unset + 1; return v.y ?? 5;", 1.0),
        ("((t.x ?? 0) + 1) ?? 5", 1.0),
        ("(t.x + 1) ?? 5", 5.0),
    ] {
        assert_eq!(number(text), expected, "{text}");
    }
    let effects = run("((1 ? (v.a = 3)) + v.unset) ?? 5; return v.a;");
    assert_eq!(effects.value, Value::Number(3.0));
}

#[test]
fn branch_forms_run_only_the_taken_side() {
    let block = run("v.y = 30; (v.y > 20) ? { v.y = 0; };");
    assert_eq!(block.variable("y"), Some(0.0));
    let kept = run("v.y = 10; (v.y > 20) ? { v.y = 0; };");
    assert_eq!(kept.variable("y"), Some(10.0));
    let group = run("v.c = 1; v.c ? (v.a = 3);");
    assert_eq!(group.variable("a"), Some(3.0));
    let skipped = run("v.c = 0; v.c ? (v.a = 3) : (v.b = 4);");
    assert_eq!(
        (skipped.variable("a"), skipped.variable("b")),
        (None, Some(4.0))
    );
    let default = run("v.d ?? { v.d = 2; }; v.d ?? { v.d = 9; };");
    assert_eq!(default.variable("d"), Some(2.0));
    let lazy = run("v.n = 0; 0 && (1 ? (v.n = 1)); 1 || (1 ? (v.n = 2));");
    assert_eq!(lazy.variable("n"), Some(0.0));
    assert_eq!(error("(v.a = 1) + 2", Milestone::Later), ErrorKind::Syntax);
}

#[test]
fn script_arrays_are_one_expression() {
    let entries = ["t.a = 1;", "v.b = t.a;", "(v.b > 0) ? {", "v.c = 2;", "};"];
    let joined = run_source(&Source::ScriptArray(&entries));
    assert_eq!(joined.variable("b"), Some(1.0));
    assert_eq!(joined.variable("c"), Some(2.0));
}

#[test]
fn versioned_values_take_the_modern_rules() {
    let text = Source::Text("v.a = 4; return v.a * 2;");
    let mut host = QueryRegistry::new(UseContext::Entity);
    let modern = compile(
        &Source::Versioned {
            expression: &text,
            version: Some(12.0),
        },
        &mut VariableLayout::new(),
        &mut host,
        &CompileOptions::default(),
    );
    let modern = modern.unwrap_or_else(|errors| panic!("{errors:?}"));
    assert!(modern.warnings.is_empty());
    let old = compile(
        &Source::Versioned {
            expression: &text,
            version: None,
        },
        &mut VariableLayout::new(),
        &mut host,
        &CompileOptions::default(),
    );
    let old = old.unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(old.warnings[0].kind, WarningKind::OldMolangVersion);
    for source in [
        Source::Versioned {
            expression: &text,
            version: Some(1.5),
        },
        Source::Versioned {
            expression: &Source::Other,
            version: Some(12.0),
        },
        Source::Other,
    ] {
        let errors = compile(
            &source,
            &mut VariableLayout::new(),
            &mut host,
            &CompileOptions::default(),
        )
        .err()
        .unwrap_or_else(|| panic!("{source:?} compiles"));
        assert_eq!(errors[0].kind, ErrorKind::Type);
    }
}

#[test]
fn load_errors_have_their_kinds() {
    for (text, milestone, kind) in [
        ("1 +", Milestone::Later, ErrorKind::Syntax),
        ("math.cos(1,)", Milestone::Later, ErrorKind::Syntax),
        ("'abc", Milestone::Later, ErrorKind::Syntax),
        ("v.é", Milestone::Later, ErrorKind::Syntax),
        ("math.nope(1)", Milestone::Later, ErrorKind::UnknownFunction),
        ("math.cos", Milestone::Later, ErrorKind::Arity),
        ("math.cos(1, 2)", Milestone::Later, ErrorKind::Arity),
        ("math.pi()", Milestone::Later, ErrorKind::Arity),
        ("foo + 1", Milestone::Later, ErrorKind::Reserved),
        ("t.a.b", Milestone::Later, ErrorKind::TempStruct),
        ("q.is_baby = 1", Milestone::Later, ErrorKind::ReadOnly),
        ("c.x = 1", Milestone::Later, ErrorKind::ReadOnly),
        ("break", Milestone::Later, ErrorKind::Misplaced),
        ("geometry.default", Milestone::Later, ErrorKind::Misplaced),
        (
            "loop(2, { v.a = 1; })",
            Milestone::Later,
            ErrorKind::Unsupported,
        ),
        (
            "c.other->q.is_baby",
            Milestone::Later,
            ErrorKind::Unsupported,
        ),
        (
            "math.random(0, 1)",
            Milestone::Later,
            ErrorKind::Unsupported,
        ),
        ("math.abs(1)", Milestone::One, ErrorKind::Unsupported),
        (
            "math.ease_in_quad(0, 1, 0.5)",
            Milestone::One,
            ErrorKind::Unsupported,
        ),
        (
            "q.in_range(1, 0, 2)",
            Milestone::One,
            ErrorKind::Unsupported,
        ),
        ("q.in_range(1, 0)", Milestone::Later, ErrorKind::Arity),
    ] {
        assert_eq!(error(text, milestone), kind, "{text}");
    }
    for text in [
        "math.abs(-1)",
        "math.ease_in_out_bounce(0, 1, 0.5)",
        "q.in_range(1, 0, 2)",
    ] {
        let compiled = compile_text(
            text,
            &mut VariableLayout::new(),
            &mut QueryRegistry::new(UseContext::Entity),
            &options(Milestone::Two),
        );
        assert!(compiled.is_ok(), "{text} is a milestone 2 construct");
    }
    assert_eq!(
        number("math.cos(0) + math.clamp(5, 0, 1) + math.pi * 0"),
        2.0
    );
}

#[test]
fn pure_queries_are_functions_of_their_arguments() {
    for (text, expected) in [
        ("q.in_range(2, 1, 3)", 1.0),
        ("q.in_range(3, 1, 2)", 0.0),
        ("q.approx_eq(1, 1.00000001, 1)", 1.0),
        ("q.approx_eq(1, 1.1)", 0.0),
        ("q.all(2, 2, 2)", 1.0),
        ("q.all(2, 2, 3)", 0.0),
        ("q.any(2, 1, 2)", 1.0),
        ("q.any(2, 1, 3)", 0.0),
    ] {
        assert_eq!(number(text), expected, "{text}");
    }
}

#[test]
fn the_cpu_keeps_subnormal_results() {
    let product = number("0.0000000001 * 0.0000000001 * 0.0000000001 * 0.0000000001 * 0.01");
    assert!(
        product.is_subnormal(),
        "{product:e} must not be flushed to zero"
    );
}

#[test]
fn a_program_lists_the_slots_it_reads_and_writes_once() {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for name in ["a", "b"] {
        registry.declare(QueryDeclaration::entity(
            name,
            ValueType::Number,
            Arguments::None,
            Rate::EntityFrame,
        ));
    }
    let mut layout = VariableLayout::new();
    let compiled = compile_text(
        "v.out = v.out + q.b + v.in; return v.other ?? q.b;",
        &mut layout,
        &mut registry,
        &options(Milestone::Two),
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    let accesses = compiled.expression.accesses();
    let slot = |name: &str| layout.slot(name).unwrap_or_else(|| panic!("v.{name}"));
    let mut reads = vec![slot("in"), slot("other"), slot("out")];
    reads.sort_unstable();
    assert_eq!(accesses.variables_read, reads);
    assert_eq!(accesses.variables_written, [slot("out")]);
    assert!(accesses.reads_own_write());
    assert_eq!(
        accesses.queries_read.len(),
        1,
        "q.a is not read and q.b is listed one time: {:?}",
        accesses.queries_read
    );
}

#[test]
fn a_query_slot_reads_back_its_value_or_missing() {
    let mut values = QueryValues::new(2);
    values.set_number(QuerySlot(1), 2.5);
    assert_eq!(values.get(QuerySlot(1)), Some(u64::from(2.5f32.to_bits())));
    assert_eq!(values.get(QuerySlot(0)), None);
    assert_eq!(values.get(QuerySlot(9)), None);
}

struct SpecRow {
    name: String,
    arguments: String,
    result: String,
    rate: String,
    pose: String,
}

fn spec_rows(markdown: &str) -> Vec<SpecRow> {
    let unquote = |cell: &str| cell.trim_matches('`').to_owned();
    let mut rows = Vec::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in markdown.lines().chain(["end"]) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let is_row = line.starts_with('|') && cells.len() >= 4;
        if is_row && cells.len() == 10 && cells[1].starts_with('`') {
            rows.push(SpecRow {
                name: unquote(cells[1]),
                arguments: cells[2].to_owned(),
                result: cells[3].to_owned(),
                rate: unquote(cells[5]),
                pose: unquote(cells[8]),
            });
        } else if is_row && cells.len() == 4 {
            pairs.push((cells[1].to_owned(), unquote(cells[2])));
        } else if !pairs.is_empty() {
            let find = |key: &str| {
                pairs
                    .iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value.clone())
            };
            if let (Some(name), Some(arguments), Some(result), Some(rate)) = (
                find("name"),
                find("arguments"),
                find("result"),
                find("rate class"),
            ) {
                rows.push(SpecRow {
                    name,
                    arguments,
                    result,
                    rate,
                    pose: "—".to_owned(),
                });
            }
            pairs.clear();
        }
    }
    rows
}

#[test]
fn the_entity_query_table_equals_the_spec() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/spec/molang-queries.md"
    );
    let markdown = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let rows = spec_rows(&markdown);
    for query in &ENTITY_QUERIES {
        let Some(row) = rows.iter().find(|row| row.name == query.name) else {
            panic!("`{}` is not in the spec", query.name);
        };
        let name = query.name;
        let result = match query.result {
            ValueType::Number => "number",
            ValueType::String => "string",
            other => panic!("{name}: {other:?}"),
        };
        assert_eq!(row.result, result, "{name}: result");
        assert_eq!(row.rate, query.rate.to_string(), "{name}: rate class");
        assert_eq!(
            row.arguments == "none",
            query.arguments == 0,
            "{name}: arguments"
        );
        match query.pose {
            PoseInput::None => assert_eq!(row.pose, "—", "{name}: pose input"),
            PoseInput::Time => assert_eq!(row.pose, "time", "{name}: pose input"),
            PoseInput::Flag => assert_eq!(row.pose, "tick", "{name}: pose input"),
            PoseInput::Other => assert!(
                !["—", "time"].contains(&row.pose.as_str()),
                "{name}: pose input"
            ),
        }
    }
}
