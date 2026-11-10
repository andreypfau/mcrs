//! The GPU gate over the occurrences of the Bedrock vanilla packs (`docs/molang-corpus.md`): each
//! text expression that classifies as pure lowers to WGSL, and naga validates all of them in one
//! module. `MCRS_MOLANG_CORPUS` names the corpus file.

#![expect(
    clippy::disallowed_methods,
    reason = "the corpus is an artefact outside the repository, named by the environment"
)]

use std::collections::BTreeMap;

use mcrs_molang::{
    ChannelClass, CompileOptions, InputRange, Milestone, QueryRegistry, VariableLayout,
    compile_text,
};
use serde::Deserialize;

#[path = "../../mcrs_molang/tests/entity_queries/mod.rs"]
mod entity_queries;

use entity_queries::{ENTITY_QUERIES, PoseInput, registry};

const TEXT_FORMS: [&str; 3] = ["string", "script_array", "versioned_object"];

fn pose_range(registry: &QueryRegistry, slot: mcrs_molang::QuerySlot) -> Option<InputRange> {
    let (_, key) = registry.slots().find(|(known, _)| *known == slot)?;
    let name = &*registry.declaration(key.declaration)?.name;
    let entry = ENTITY_QUERIES.iter().find(|entry| entry.name == name)?;
    match entry.pose {
        PoseInput::Time => Some(InputRange::new(0.0, f32::MAX)),
        PoseInput::Flag => Some(InputRange::BOOLEAN),
        PoseInput::Other => Some(InputRange::FINITE),
        PoseInput::None => None,
    }
}

#[derive(Deserialize)]
struct Record {
    form: String,
    #[serde(default)]
    expr: Option<String>,
    kind: String,
    field: String,
}

#[derive(Default)]
struct Tally {
    total: usize,
    pure: usize,
    classes: BTreeMap<String, usize>,
}

impl Tally {
    fn add(&mut self, class: &str) {
        self.total += 1;
        if class == "pure" {
            self.pure += 1;
        }
        *self.classes.entry(class.to_owned()).or_default() += 1;
    }

    fn report(&self, title: &str) {
        eprintln!(
            "{title}: {} of {} pure ({:.1}%)",
            self.pure,
            self.total,
            100.0 * self.pure as f64 / self.total.max(1) as f64
        );
        for (class, count) in &self.classes {
            eprintln!("  {class}: {count}");
        }
    }
}

#[test]
#[ignore = "reads the corpus that MCRS_MOLANG_CORPUS names"]
fn every_pure_expression_of_the_corpus_lowers_to_valid_wgsl() {
    let Ok(path) = std::env::var("MCRS_MOLANG_CORPUS") else {
        eprintln!("MCRS_MOLANG_CORPUS is not set; the gate is skipped");
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let options = CompileOptions {
        milestone: Milestone::Two,
        ..CompileOptions::default()
    };
    let mut all = Tally::default();
    let mut channels = Tally::default();
    let mut source = String::from(mcrs_molang_wgsl::LIBRARY);
    for line in text.lines() {
        let record: Record = serde_json::from_str(line).unwrap();
        let Some(expression) = record
            .expr
            .filter(|_| TEXT_FORMS.contains(&record.form.as_str()))
        else {
            continue;
        };
        let mut registry = registry();
        let class = match compile_text(
            &expression,
            &mut VariableLayout::new(),
            &mut registry,
            &options,
        ) {
            Err(errors) => format!("load error {}", errors[0].kind.name()),
            Ok(compiled) => match compiled
                .expression
                .classify(|slot| pose_range(&registry, slot))
            {
                ChannelClass::Constant => "constant".to_owned(),
                ChannelClass::Pure(program) => {
                    let name = format!("channel_{}", all.pure);
                    source.push_str(&mcrs_molang_wgsl::lower(&program, &name));
                    "pure".to_owned()
                }
                ChannelClass::State(reason) => {
                    let reason = format!("{reason:?}");
                    format!("state {}", reason.split('(').next().unwrap_or_default())
                }
            },
        };
        if record.kind == "animation" && record.field.starts_with("bones.") {
            channels.add(&class);
        }
        all.add(&class);
    }
    all.report("text expressions");
    channels.report("bone channel expressions");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    eprintln!("naga validated {} functions", all.pure);
}
