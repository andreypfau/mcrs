//! The parse gate over the occurrences of the Bedrock vanilla packs (`docs/molang-corpus.md`,
//! section 12). The corpus lives outside the repository; `MCRS_MOLANG_CORPUS` names the file.

#![expect(
    clippy::disallowed_methods,
    reason = "the corpus is an artefact outside the repository, named by the environment"
)]

use std::collections::BTreeMap;

use serde::Deserialize;

use mcrs_molang::{CompileOptions, Milestone, VariableLayout, compile_text, parse};

#[expect(dead_code, reason = "this test does not read the GPU pose inputs")]
mod entity_queries;

use entity_queries::registry;

const TEXT_FORMS: [&str; 3] = ["string", "script_array", "versioned_object"];

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
