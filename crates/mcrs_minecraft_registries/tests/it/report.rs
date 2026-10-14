use mcrs_minecraft_registries::{Declaration, Diagnostic, Missing, Referrer, Stage};
use serde::de::DeserializeSeed;
use serde::de::value::{self, StrDeserializer};

use crate::support::{
    Block, DamageType, LootTable, Packs, damage, declarations, report, report_errors, rl,
};

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

fn validated() -> Declaration {
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
}

fn resolving() -> Declaration {
    Declaration::data::<DamageType>(rl("damage_type"), Stage::World)
        .validate(|_, ctx| {
            let seed = ctx.id::<DamageType>();
            seed.deserialize(StrDeserializer::<value::Error>::new("t:missing"))
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .into()
}

#[test]
fn registry_wide_checks_run_only_after_a_stage_without_entry_errors() {
    let non_empty = || -> Declaration {
        Declaration::data::<DamageType>(rl("damage_type"), Stage::World)
            .non_empty()
            .into()
    };
    let dup = damage("dup");
    let ok = damage("ok");
    enum Expected {
        Loads,
        EntryErrorOnly,
        ValidationOnly,
        Only(Diagnostic),
    }
    type Files<'a> = &'a [(&'a str, &'a str)];
    let cases: [(&str, Declaration, Files<'_>, Expected); 6] = [
        (
            "a validator resolving a missing name",
            resolving(),
            &[("data/t/damage_type/x.json", &ok)],
            Expected::ValidationOnly,
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
            "empty with an unresolved tag member",
            non_empty(),
            &[("data/t/tags/damage_type/t.json", r#"{"values":["t:nope"]}"#)],
            Expected::Only(Diagnostic::Unresolved {
                registry: rl("damage_type"),
                missing: vec![Missing {
                    name: rl("t:nope"),
                    tag: false,
                    referrer: Referrer::TagFile {
                        tag: rl("t:t"),
                        pack: "file/a".to_owned(),
                    },
                }],
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
        let errors = report_errors(&result);
        match expected {
            Expected::Loads => assert!(result.is_ok(), "{name}: {errors:?}"),
            Expected::Only(expected) => assert_eq!(errors, [expected], "{name}"),
            Expected::EntryErrorOnly => assert!(
                matches!(errors.as_slice(), [Diagnostic::Entry { .. }]),
                "{name}: {errors:?}"
            ),
            Expected::ValidationOnly => assert!(
                matches!(errors.as_slice(), [Diagnostic::Validation { registry, .. }] if *registry == rl("damage_type")),
                "{name}: {errors:?}"
            ),
        }
    }
}

#[test]
fn a_registry_or_type_declared_twice_is_refused() {
    let cases: [(&str, Vec<Declaration>, &str); 4] = [
        (
            "one type under two keys",
            vec![
                Declaration::data::<DamageType>(rl("damage_type"), Stage::World).into(),
                Declaration::data::<DamageType>(rl("other"), Stage::World).into(),
            ],
            "minecraft:root/minecraft:other: the registry or its type is declared twice",
        ),
        (
            "one key for two types",
            vec![
                Declaration::data::<DamageType>(rl("damage_type"), Stage::World).into(),
                Declaration::data::<LootTable>(rl("damage_type"), Stage::Reloadable).into(),
            ],
            "minecraft:root/minecraft:damage_type: the registry or its type is declared twice",
        ),
        (
            "a fixed registry naming an entry twice",
            vec![Declaration::fixed::<Block>(
                rl("block"),
                vec![rl("stone"), rl("dirt"), rl("stone")],
            )],
            "minecraft:root/minecraft:block: an entry name is listed twice",
        ),
        (
            "two keys sharing a path",
            vec![
                Declaration::data::<DamageType>(rl("damage_type"), Stage::World).into(),
                Declaration::fixed::<Block>(rl("t:damage_type"), vec![]),
            ],
            "minecraft:root/t:damage_type: another registry is declared with the same path",
        ),
    ];
    for (name, declarations, expected) in cases {
        let report_of = Packs::new().load(&declarations).unwrap_err();
        assert_eq!(report(&report_of), [expected], "{name}");
    }
}
