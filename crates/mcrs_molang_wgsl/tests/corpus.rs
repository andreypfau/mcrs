//! The GPU gate over the occurrences of the Bedrock vanilla packs (`docs/molang-corpus.md`): each
//! text expression that classifies as pure lowers to WGSL, and naga validates all of them in one
//! module. `MCRS_MOLANG_CORPUS` names the corpus file.

#![expect(
    clippy::disallowed_methods,
    reason = "the corpus is an artefact outside the repository, named by the environment"
)]

use std::collections::BTreeMap;

use mcrs_molang::{
    Arguments, ChannelClass, CompileOptions, InputRange, Milestone, QueryDeclaration,
    QueryRegistry, QueryScope, Rate, UseContext, ValueType, VariableLayout, compile_text,
};
use serde::Deserialize;

const TEXT_FORMS: [&str; 3] = ["string", "script_array", "versioned_object"];

/// How a query reaches a GPU channel: the column "Pose input" of `docs/spec/molang-queries.md`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pose {
    /// `time`: a value of the entity clock, never negative.
    Time,
    /// `tick` for a flag: `0` or `1`.
    Flag,
    /// Any other pose input kind.
    Other,
    /// `—`: not a pose input.
    None,
}

const ENTITY_QUERIES: [(&str, ValueType, u8, Rate, Pose); 37] = [
    (
        "life_time",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Time,
    ),
    (
        "delta_time",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Time,
    ),
    (
        "frame_alpha",
        ValueType::Number,
        0,
        Rate::Frame,
        Pose::Other,
    ),
    (
        "time_of_day",
        ValueType::Number,
        0,
        Rate::Frame,
        Pose::Other,
    ),
    (
        "modified_distance_moved",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "modified_move_speed",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "ground_speed",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Other,
    ),
    (
        "vertical_speed",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Other,
    ),
    (
        "is_moving",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_on_ground",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_in_water",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "target_x_rotation",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "target_y_rotation",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "body_y_rotation",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "head_x_rotation",
        ValueType::Number,
        1,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "head_y_rotation",
        ValueType::Number,
        1,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "is_baby",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_alive",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "hurt_time",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Other,
    ),
    (
        "death_ticks",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::Other,
    ),
    (
        "variant",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Other,
    ),
    (
        "variant_identifier",
        ValueType::String,
        0,
        Rate::EntityTick,
        Pose::None,
    ),
    (
        "model_scale",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Other,
    ),
    (
        "is_saddled",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    ("pose", ValueType::String, 0, Rate::EntityTick, Pose::None),
    (
        "is_on_fire",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_sneaking",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_sprinting",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_swimming",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_invisible",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "is_riding",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "has_rider",
        ValueType::Number,
        0,
        Rate::EntityTick,
        Pose::Flag,
    ),
    (
        "anim_time",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        Pose::Time,
    ),
    (
        "key_frame_lerp_time",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        Pose::Time,
    ),
    (
        "all_animations_finished",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        Pose::None,
    ),
    (
        "any_animation_finished",
        ValueType::Number,
        0,
        Rate::AnimationFrame,
        Pose::None,
    ),
    (
        "distance_from_camera",
        ValueType::Number,
        0,
        Rate::EntityFrame,
        Pose::None,
    ),
];

fn registry() -> QueryRegistry {
    let mut registry = QueryRegistry::new(UseContext::Entity);
    for (name, result, arguments, rate, _) in ENTITY_QUERIES {
        registry.declare(QueryDeclaration {
            name: name.into(),
            arguments: if arguments == 0 {
                Arguments::None
            } else {
                Arguments::Literals(arguments)
            },
            result,
            rate,
            scope: QueryScope::Entity,
            milestone: Milestone::One,
        });
    }
    registry
}

fn pose_range(registry: &QueryRegistry, slot: mcrs_molang::QuerySlot) -> Option<InputRange> {
    let (_, key) = registry.slots().find(|(known, _)| *known == slot)?;
    let name = &*registry.declaration(key.declaration)?.name;
    let (_, _, _, _, pose) = ENTITY_QUERIES.iter().find(|entry| entry.0 == name)?;
    match pose {
        Pose::Time => Some(InputRange::new(0.0, f32::MAX)),
        Pose::Flag => Some(InputRange::BOOLEAN),
        Pose::Other => Some(InputRange::FINITE),
        Pose::None => None,
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
