use mcrs_minecraft_registries::{Declaration, Diagnostic, Stage};
use serde::de::DeserializeSeed;
use serde::de::value::{self, StrDeserializer};

use crate::support::{DamageType, Packs, damage, declarations, report, report_errors, rl};

#[test]
fn a_failed_load_lists_errors_by_registry_then_entry_path_then_namespace() {
    let missing = r#"{"exhaustion":0.1}"#;
    let report_of = Packs::new()
        .world(
            "a",
            &[
                ("data/a/damage_type/b.json", missing),
                ("data/z/damage_type/a.json", missing),
                ("data/a/damage_type/a.json", missing),
                ("data/b/damage_type/a.json", missing),
                (
                    "data/t/enchantment/e.json",
                    r#"{"exclusive_set":["t:missing"]}"#,
                ),
                (
                    "data/t/enchantment/f.json",
                    r#"{"supported_items":"t:none"}"#,
                ),
            ],
        )
        .load(&declarations())
        .unwrap_err();
    let keys: Vec<(String, String)> = report_of
        .iter()
        .filter_map(Diagnostic::report_key)
        .map(|(registry, key)| (registry.to_string(), key.to_string()))
        .collect();
    let expected = [
        ("minecraft:damage_type", "a:a"),
        ("minecraft:damage_type", "b:a"),
        ("minecraft:damage_type", "z:a"),
        ("minecraft:damage_type", "a:b"),
        ("minecraft:enchantment", "t:f"),
        ("minecraft:root", "minecraft:enchantment"),
    ];
    assert_eq!(
        keys,
        expected
            .iter()
            .map(|(r, k)| (r.to_string(), k.to_string()))
            .collect::<Vec<_>>()
    );
    let lines = report(&report_of);
    let first = lines.iter().find(|l| l.contains("/a:a")).unwrap();
    assert!(
        first.starts_with("minecraft:damage_type/a:a: ")
            && first.contains("data/a/damage_type/a.json")
            && first.contains("file/a"),
        "{first}"
    );
}

#[test]
fn registry_wide_checks_run_only_after_a_stage_without_entry_errors() {
    let non_empty = || -> Declaration {
        Declaration::data::<DamageType>(rl("damage_type"), Stage::World)
            .non_empty()
            .into()
    };
    let validated = || -> Declaration {
        Declaration::data::<DamageType>(rl("damage_type"), Stage::World)
            .validate(|registry, ctx| {
                let seen = ctx.registry::<DamageType>().map(|r| r.len());
                if seen != Some(registry.len()) {
                    return Err(format!("the context sees {seen:?} entries"));
                }
                match registry
                    .iter()
                    .any(|(_, _, value)| value.message_id == "dup")
                {
                    true => Err("a message id is used twice".to_owned()),
                    false => Ok(()),
                }
            })
            .into()
    };
    let resolving = || -> Declaration {
        Declaration::data::<DamageType>(rl("damage_type"), Stage::World)
            .validate(|_, ctx| {
                let seed = ctx.id::<DamageType>();
                seed.deserialize(StrDeserializer::<value::Error>::new("t:missing"))
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })
            .into()
    };
    let dup = damage("dup");
    let ok = damage("ok");
    enum Expected {
        Loads,
        EntryErrorOnly,
        Only(Diagnostic),
    }
    type Files<'a> = &'a [(&'a str, &'a str)];
    let cases: [(&str, Declaration, Files<'_>, Expected); 5] = [
        (
            "a validator resolving a missing name",
            resolving(),
            &[("data/t/damage_type/x.json", &ok)],
            Expected::Only(Diagnostic::Validation {
                registry: rl("damage_type"),
                message: "unknown entry t:missing of minecraft:damage_type".into(),
            }),
        ),
        (
            "empty",
            non_empty(),
            &[],
            Expected::Only(Diagnostic::EmptyRegistry {
                registry: rl("damage_type"),
            }),
        ),
        (
            "empty with an entry error",
            non_empty(),
            &[("data/t/damage_type/x.json", "{")],
            Expected::EntryErrorOnly,
        ),
        (
            "validation fails",
            validated(),
            &[("data/t/damage_type/x.json", &dup)],
            Expected::Only(Diagnostic::Validation {
                registry: rl("damage_type"),
                message: "a message id is used twice".into(),
            }),
        ),
        (
            "validation passes",
            validated(),
            &[("data/t/damage_type/x.json", &ok)],
            Expected::Loads,
        ),
    ];
    for (name, declaration, files, expected) in cases {
        let result = Packs::new().world("a", files).load(&[declaration]);
        let errors: Vec<Diagnostic> = match &result {
            Ok(_) => Vec::new(),
            Err(report) => report_errors(report),
        };
        match expected {
            Expected::Loads => assert!(result.is_ok(), "{name}: {errors:?}"),
            Expected::Only(expected) => assert_eq!(errors, [expected], "{name}"),
            Expected::EntryErrorOnly => assert!(
                matches!(errors.as_slice(), [Diagnostic::Entry { .. }]),
                "{name}: {errors:?}"
            ),
        }
    }
}

#[test]
fn a_registry_or_type_declared_twice_is_refused() {
    let declarations = [
        Declaration::data::<DamageType>(rl("damage_type"), Stage::World).into(),
        Declaration::data::<DamageType>(rl("other"), Stage::World).into(),
    ];
    let report_of = Packs::new().load(&declarations).unwrap_err();
    assert!(matches!(
        report_of.iter().collect::<Vec<_>>().as_slice(),
        [Diagnostic::Declaration { registry, .. }] if *registry == rl("other")
    ));
}
