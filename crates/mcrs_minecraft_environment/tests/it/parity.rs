//! Parity with the environment artefact that the reference game wrote.

use std::sync::LazyLock;

use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::Named;
use serde_json::Value;

use mcrs_minecraft_environment::attribute::{
    Argument, AttributeType, AttributeValue, Interpolation, Operation, read_argument,
};
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::timeline::{Easing, NamedEasing, Timeline};
use mcrs_minecraft_environment::world_clock::{ClockTime, ClockTimeMarker};

static ARTEFACT: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_slice(&fixture("environment").bytes()).unwrap());

fn section(name: &str) -> &'static Value {
    &ARTEFACT[name]
}

fn entries(value: &Value) -> impl Iterator<Item = (&String, &Value)> {
    value.as_object().unwrap().iter()
}

fn rows(value: &Value) -> &Vec<Value> {
    value.as_array().unwrap()
}

/// The artefact as a tag: a number with a fraction stays a float, so `-0.0` keeps its sign.
fn tag(value: &Value) -> NbtTag {
    match value {
        Value::Bool(value) => NbtTag::Byte(i8::from(*value)),
        Value::Number(number) if number.is_f64() => NbtTag::Float(number.as_f64().unwrap() as f32),
        Value::Number(number) => NbtTag::Long(number.as_i64().unwrap()),
        Value::String(text) => NbtTag::String(text.clone()),
        Value::Array(items) => NbtTag::List(items.iter().map(tag).collect()),
        Value::Object(fields) => {
            let mut compound = NbtCompound::new();
            for (name, field) in fields {
                compound.put(name, tag(field));
            }
            NbtTag::Compound(compound)
        }
        Value::Null => NbtTag::End,
    }
}

/// The artefact writes an ambient particle by its type alone.
fn particle_tag(value: &Value) -> NbtTag {
    let mut list = Vec::new();
    for item in rows(value) {
        let mut particle = NbtCompound::new();
        particle.put("type", tag(&item["particle"]));
        let mut entry = NbtCompound::new();
        entry.put("particle", NbtTag::Compound(particle));
        entry.put("probability", tag(&item["probability"]));
        list.push(NbtTag::Compound(entry));
    }
    NbtTag::List(list)
}

fn kind(name: &str) -> AttributeType {
    match name {
        "float" => AttributeType::Float,
        "angle_degrees" => AttributeType::AngleDegrees,
        "integer" => AttributeType::Integer,
        "boolean" => AttributeType::Boolean,
        "rgb_color" => AttributeType::RgbColor,
        "argb_color" => AttributeType::ArgbColor,
        "moon_phase" => AttributeType::MoonPhase,
        "ambient_particles" => AttributeType::AmbientParticles,
        other => panic!("no attribute type {other}"),
    }
}

fn value(kind: AttributeType, value: &Value) -> AttributeValue {
    let tag = match kind {
        AttributeType::AmbientParticles => particle_tag(value),
        AttributeType::Integer => NbtTag::Int(value.as_i64().unwrap() as i32),
        _ => tag(value),
    };
    AttributeValue::read(kind, &tag).unwrap()
}

fn floats(value: &AttributeValue) -> Vec<f32> {
    match value {
        AttributeValue::Float(value) => vec![*value],
        AttributeValue::Rgb(channels) => channels.to_vec(),
        AttributeValue::Argb(channels) => channels.to_vec(),
        _ => Vec::new(),
    }
}

/// Floats agree bit for bit, the sign of zero too.
fn assert_exact(actual: &AttributeValue, expected: &AttributeValue, context: &str) {
    let bits = |value: &AttributeValue| -> Vec<u32> {
        floats(value).iter().map(|float| float.to_bits()).collect()
    };
    assert_eq!(actual, expected, "{context}");
    assert_eq!(bits(actual), bits(expected), "{context}");
}

#[test]
fn the_modifiers_agree_with_the_artefact() {
    let mut count = 0;
    for (type_name, modifiers) in entries(section("modifiers")) {
        let kind = kind(type_name);
        for (modifier_name, cases) in entries(modifiers) {
            let modifier: Operation = modifier_name.parse().unwrap();
            let argument_kind = modifier.argument_kind(kind).unwrap();
            for case in rows(cases) {
                let subject = value(kind, &case["subject"]);
                let argument = match argument_kind {
                    mcrs_minecraft_environment::attribute::ArgumentKind::Value(
                        AttributeType::Integer,
                    ) => Argument::Value(value(AttributeType::Integer, &case["argument"])),
                    other => Argument::read(other, &tag(&case["argument"])).unwrap(),
                };
                let expected = value(kind, &case["result"]);
                let actual = modifier.apply(&subject, &argument).unwrap();
                assert_exact(
                    &actual,
                    &expected,
                    &format!("{type_name} {modifier_name} {case}"),
                );
                count += 1;
            }
        }
    }
    assert!(count > 500, "only {count} modifier cases");
}

#[test]
fn the_interpolation_of_each_type_agrees_with_the_artefact() {
    for (type_name, places) in entries(section("type_lerps")) {
        let kind = kind(type_name);
        for (place_name, cases) in entries(places) {
            let place = match place_name.as_str() {
                "keyframe" => Interpolation::Keyframe,
                "weather" => Interpolation::Weather,
                "spatial" => Interpolation::Spatial,
                "partial_tick" => Interpolation::PartialTick,
                other => panic!("no interpolation {other}"),
            };
            for case in rows(cases) {
                let t = case["alpha"].as_f64().unwrap() as f32;
                let actual = AttributeValue::interpolate(
                    kind,
                    place,
                    t,
                    &value(kind, &case["from"]),
                    &value(kind, &case["to"]),
                );
                let expected = value(kind, &case["result"]);
                assert_exact(
                    &actual,
                    &expected,
                    &format!("{type_name} {place_name} {case}"),
                );
            }
        }
    }
}

#[test]
fn the_easings_agree_with_the_artefact() {
    let easings = section("easings");
    for (name, samples) in entries(&easings["named"]) {
        let easing = NamedEasing::ALL
            .into_iter()
            .find(|easing| easing.name() == name)
            .unwrap_or_else(|| panic!("no easing {name}"));
        let bound = if name.contains("sine") {
            2.0e-4
        } else {
            1.0e-5
        };
        for sample in rows(samples) {
            let x = sample[0].as_f64().unwrap() as f32;
            let y = sample[1].as_f64().unwrap() as f32;
            let eased = easing.ease(x);
            assert!((eased - y).abs() <= bound, "{name}({x}) = {eased}, not {y}");
        }
    }
    for curve in rows(&easings["cubic_bezier"]) {
        let controls: Vec<f32> = rows(&curve["controls"])
            .iter()
            .map(|control| control.as_f64().unwrap() as f32)
            .collect();
        let easing = Easing::CubicBezier(controls.clone().try_into().unwrap());
        for sample in rows(&curve["samples"]) {
            let x = sample[0].as_f64().unwrap() as f32;
            let y = sample[1].as_f64().unwrap() as f32;
            let eased = easing.ease(x);
            let (low, high) = bezier_y_within_tolerance(&controls, x);
            let slack = 1.0e-6;
            assert!(
                low - slack <= y && y <= high + slack,
                "{controls:?} at {x}: the artefact {y} is off the curve [{low}, {high}]"
            );
            assert!(
                low - slack <= eased && eased <= high + slack,
                "{controls:?} at {x}: {eased} is off the curve [{low}, {high}]"
            );
        }
    }
}

/// The span of Y over the parameters whose X lies within the solver tolerance of `x`. The
/// rule lets a solver stop at any such parameter, so a steep curve admits a span wider than
/// the stated precision.
fn bezier_y_within_tolerance(controls: &[f32], x: f32) -> (f32, f32) {
    let [x1, y1, x2, y2] = [0, 1, 2, 3].map(|at| f64::from(controls[at]));
    let axis = |p1: f64, p2: f64, u: f64| {
        let (a, b, c) = (3.0 * p1 - 3.0 * p2 + 1.0, -6.0 * p1 + 3.0 * p2, 3.0 * p1);
        ((a * u + b) * u + c) * u
    };
    let solve = |target: f64| {
        let (mut low, mut high) = (0.0, 1.0);
        for _ in 0..64 {
            let mid = (low + high) / 2.0;
            if axis(x1, x2, mid) < target {
                low = mid;
            } else {
                high = mid;
            }
        }
        low
    };
    let tolerance = 1.0e-5;
    let (from, to) = (
        solve(f64::from(x) - tolerance),
        solve(f64::from(x) + tolerance),
    );
    let ys: Vec<f64> = (0..=100)
        .map(|step| axis(y1, y2, from + (to - from) * f64::from(step) / 100.0))
        .collect();
    let low = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let high = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (low as f32, high as f32)
}

fn vanilla_timeline(name: &str) -> Timeline<Named> {
    let path = name.trim_start_matches("minecraft:");
    let bytes = std::fs::read(
        mcrs_minecraft_worldgen_testing::vanilla_dir()
            .join("timeline")
            .join(format!("{path}.json")),
    )
    .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// An eased sample agrees to 10^-4 of the track's largest argument: the solver tolerance of a
/// cubic Bézier curve moves the eased fraction, and the arguments' span scales that move.
fn assert_close(actual: &Argument, expected: &Argument, scale: f32, context: &str) {
    let (Argument::Value(actual), Argument::Value(expected)) = (actual, expected) else {
        assert_eq!(actual, expected, "{context}");
        return;
    };
    let (actual_floats, expected_floats) = (floats(actual), floats(expected));
    if expected_floats.is_empty() {
        assert_eq!(actual, expected, "{context}");
        return;
    }
    for (a, e) in actual_floats.iter().zip(&expected_floats) {
        assert!(
            (a - e).abs() <= 1.0e-4 * e.abs().max(scale).max(1.0),
            "{context}: {actual:?} is not {expected:?}"
        );
    }
}

#[test]
fn the_vanilla_timelines_sample_as_the_artefact() {
    for (name, expected) in entries(section("timelines")) {
        let timeline = vanilla_timeline(name);
        assert_eq!(
            timeline.period_ticks.map(i64::from),
            expected["period_ticks"].as_i64(),
            "{name}"
        );
        for (attribute_name, track) in entries(&expected["tracks"]) {
            let attribute = EnvironmentAttribute::read(attribute_name).unwrap();
            let actual = &timeline.tracks[&attribute];
            let modifier: Operation = track["modifier"].as_str().unwrap().parse().unwrap();
            assert_eq!(actual.modifier(), modifier, "{name} {attribute_name}");
            let eased = !matches!(
                actual.ease(),
                Easing::Named(NamedEasing::Linear | NamedEasing::Constant)
            );
            let scale = actual
                .keyframes()
                .iter()
                .filter_map(|keyframe| match &keyframe.argument {
                    Argument::Value(value) => Some(floats(value)),
                    _ => None,
                })
                .flatten()
                .fold(0.0f32, |largest, float| largest.max(float.abs()));
            for sample in rows(&track["samples"]) {
                let ticks = sample[0].as_i64().unwrap();
                let wanted = read_argument(attribute, modifier, &tag(&sample[1])).unwrap();
                let got = actual.sample(ticks).unwrap();
                let context = format!("{name} {attribute_name} at {ticks}");
                if eased {
                    assert_close(&got, &wanted, scale, &context);
                } else {
                    assert_eq!(got, wanted, "{context}");
                }
            }
        }
    }
}

#[test]
fn a_value_outside_its_range_becomes_the_nearer_bound() {
    for case in rows(section("range_clamp")) {
        let attribute = EnvironmentAttribute::read(case["attribute"].as_str().unwrap()).unwrap();
        let float = |name: &str| case[name].as_f64().unwrap() as f32;
        let sum = Operation::Add
            .apply(
                &AttributeValue::Float(float("value")),
                &Argument::Value(AttributeValue::Float(float("add"))),
            )
            .unwrap()
            .sanitized(attribute);
        assert_eq!(sum, AttributeValue::Float(float("result")), "{case}");
    }
}

#[test]
fn clock_parity() {
    for case in rows(section("server_clock")) {
        let mut clock = ClockTime {
            total_ticks: case["start_total_ticks"].as_i64().unwrap(),
            rate: case["rate"].as_f64().unwrap() as f32,
            ..ClockTime::default()
        };
        let mut ticks = 0;
        for step in rows(&case["steps"]) {
            let target = step[0].as_i64().unwrap();
            while ticks < target {
                clock.advance();
                ticks += 1;
            }
            assert_eq!(
                clock.total_ticks,
                step[1].as_i64().unwrap(),
                "{case} at {target}"
            );
            assert_eq!(
                clock.partial_tick,
                step[2].as_f64().unwrap() as f32,
                "{case} at {target}"
            );
        }
    }
    for case in rows(section("client_clock")) {
        let mut game_time = case["update_game_time"].as_i64().unwrap();
        let mut clock = ClockTime {
            total_ticks: case["update_total_ticks"].as_i64().unwrap(),
            partial_tick: case["update_partial_tick"].as_f64().unwrap() as f32,
            rate: case["rate"].as_f64().unwrap() as f32,
            paused: false,
        };
        for step in rows(&case["steps"]) {
            let now = step[0].as_i64().unwrap();
            clock.advance_by_game_ticks(now - game_time);
            game_time = now;
            assert_eq!(
                clock.total_ticks,
                step[1].as_i64().unwrap(),
                "{case} at {now}"
            );
            assert_eq!(
                clock.partial_tick,
                step[2].as_f64().unwrap() as f32,
                "{case} at {now}"
            );
        }
    }
    for case in rows(section("time_markers")) {
        let marker = ClockTimeMarker {
            ticks: case["ticks"].as_u64().unwrap() as u32,
            period_ticks: case["period_ticks"].as_u64().map(|period| period as u32),
            show_in_commands: false,
        };
        for sample in rows(&case["samples"]) {
            let at = sample[0].as_i64().unwrap();
            assert_eq!(
                marker.occurs_at(at),
                sample[1].as_bool().unwrap(),
                "{case} at {at}"
            );
            assert_eq!(
                marker.next_after(at),
                sample[2].as_i64().unwrap(),
                "{case} at {at}"
            );
        }
    }
}
