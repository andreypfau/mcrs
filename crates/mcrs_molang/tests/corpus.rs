//! The parse gate over the occurrences of the Bedrock vanilla packs (`docs/molang-corpus.md`,
//! section 12). The corpus lives outside the repository; `MCRS_MOLANG_CORPUS` names the file.

#![expect(
    clippy::disallowed_methods,
    reason = "the corpus is an artefact outside the repository, named by the environment"
)]

use std::collections::BTreeMap;

use serde::Deserialize;

use mcrs_molang::{
    Arguments, CompileOptions, Milestone, QueryDeclaration, QueryRegistry, QueryScope, Rate,
    UseContext, ValueType, VariableLayout, compile_text, parse,
};

const TEXT_FORMS: [&str; 3] = ["string", "script_array", "versioned_object"];

/// The entity queries of `docs/spec/molang-queries.md`, so that the compile census sees the
/// errors of the language rather than unknown names.
const ENTITY_QUERIES: [(&str, ValueType, u8); 35] = [
    ("life_time", ValueType::Number, 0),
    ("delta_time", ValueType::Number, 0),
    ("frame_alpha", ValueType::Number, 0),
    ("time_of_day", ValueType::Number, 0),
    ("modified_distance_moved", ValueType::Number, 0),
    ("modified_move_speed", ValueType::Number, 0),
    ("ground_speed", ValueType::Number, 0),
    ("vertical_speed", ValueType::Number, 0),
    ("is_moving", ValueType::Number, 0),
    ("is_on_ground", ValueType::Number, 0),
    ("is_in_water", ValueType::Number, 0),
    ("target_x_rotation", ValueType::Number, 0),
    ("target_y_rotation", ValueType::Number, 0),
    ("body_y_rotation", ValueType::Number, 0),
    ("head_x_rotation", ValueType::Number, 1),
    ("head_y_rotation", ValueType::Number, 1),
    ("is_baby", ValueType::Number, 0),
    ("is_alive", ValueType::Number, 0),
    ("hurt_time", ValueType::Number, 0),
    ("death_ticks", ValueType::Number, 0),
    ("variant", ValueType::Number, 0),
    ("variant_identifier", ValueType::String, 0),
    ("model_scale", ValueType::Number, 0),
    ("is_saddled", ValueType::Number, 0),
    ("pose", ValueType::String, 0),
    ("is_on_fire", ValueType::Number, 0),
    ("is_sneaking", ValueType::Number, 0),
    ("is_sprinting", ValueType::Number, 0),
    ("is_swimming", ValueType::Number, 0),
    ("is_invisible", ValueType::Number, 0),
    ("is_riding", ValueType::Number, 0),
    ("has_rider", ValueType::Number, 0),
    ("anim_time", ValueType::Number, 0),
    ("key_frame_lerp_time", ValueType::Number, 0),
    ("distance_from_camera", ValueType::Number, 0),
];

fn registry() -> QueryRegistry {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for (name, result, arguments) in ENTITY_QUERIES {
        registry.declare(QueryDeclaration {
            name: name.into(),
            arguments: if arguments == 0 {
                Arguments::None
            } else {
                Arguments::Literals(arguments)
            },
            result,
            rate: Rate::EntityFrame,
            scope: QueryScope::Entity,
            milestone: Milestone::One,
        });
    }
    registry
}

#[derive(Deserialize)]
struct Form {
    form: String,
}

#[derive(Deserialize)]
struct TextRecord {
    expr: String,
    file: String,
    pointer: String,
}

#[test]
#[ignore = "reads the corpus that MCRS_MOLANG_CORPUS names"]
fn every_text_expression_of_the_corpus_parses() {
    let Ok(path) = std::env::var("MCRS_MOLANG_CORPUS") else {
        eprintln!("MCRS_MOLANG_CORPUS is not set; the gate is skipped");
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let mut parsed = 0usize;
    let mut failures = Vec::new();
    let mut census: BTreeMap<String, usize> = BTreeMap::new();
    for line in text.lines() {
        let form: Form = serde_json::from_str(line).unwrap();
        if !TEXT_FORMS.contains(&form.form.as_str()) {
            continue;
        }
        let record: TextRecord = serde_json::from_str(line).unwrap();
        let expression = record.expr.as_str();
        match parse(expression) {
            Ok(_) => parsed += 1,
            Err(errors) => failures.push(format!("{} {}: {errors:?}", record.file, record.pointer)),
        }
        let options = CompileOptions {
            milestone: Milestone::Two,
            ..CompileOptions::default()
        };
        let outcome = match compile_text(
            expression,
            &mut VariableLayout::new(),
            &mut registry(),
            &options,
        ) {
            Ok(_) => "compiles".to_owned(),
            Err(errors) => format!("load error {}", errors[0].kind.name()),
        };
        *census.entry(outcome).or_default() += 1;
    }
    let total = parsed + failures.len();
    eprintln!(
        "parsed {parsed} of {total} text expressions ({:.2}%)",
        100.0 * parsed as f64 / total.max(1) as f64
    );
    for (outcome, count) in &census {
        eprintln!("  milestone 2 compile, {outcome}: {count}");
    }
    assert!(
        failures.is_empty(),
        "{} parse errors:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
