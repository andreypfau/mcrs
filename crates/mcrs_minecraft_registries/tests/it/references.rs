use mcrs_minecraft_registries::{
    Diagnostic, Diagnostics, IdSet, Missing, Ref, Referrer, Registries,
};

use crate::support::{
    DamageType, Enchant, Enchantment, Item, Packs, damage, declarations, get, report_errors, rl,
    value,
};

fn load_enchantment(body: &str, extra: &[(&str, &str)]) -> Result<Registries, Diagnostics> {
    let files = [&[("data/t/enchantment/e.json", body)][..], extra].concat();
    Packs::new()
        .world("a", &files)
        .load(&declarations())
        .map(|(registries, _)| registries)
}

fn e(registries: &Registries) -> &Enchant {
    value::<Enchantment>(registries, "t:e")
}

fn fails_e(result: &Result<Registries, Diagnostics>) -> bool {
    report_errors(result)
        .iter()
        .any(|d| matches!(d, Diagnostic::Entry { entry, .. } if *entry == rl("t:e")))
}

fn from_e(name: &str, tag: bool) -> Missing {
    Missing {
        name: rl(name),
        tag,
        referrer: Referrer::Entry {
            registry: rl("enchantment"),
            entry: rl("t:e"),
        },
    }
}

#[test]
fn a_tag_of_the_same_stage_resolves_whatever_the_file_order() {
    for tag_path in [
        "data/t/tags/damage_type/my_damage_tag.json",
        "data/a/tags/damage_type/my_damage_tag.json",
    ] {
        let namespace = tag_path.split('/').nth(1).unwrap();
        let body = format!(r##"{{"damage_types":"#{namespace}:my_damage_tag"}}"##);
        let damage_a = damage("a");
        let registries = load_enchantment(
            &body,
            &[
                ("data/t/damage_type/a.json", &damage_a),
                (tag_path, r#"{"values":["t:a"]}"#),
            ],
        )
        .unwrap();
        let tags = registries.tags::<DamageType>().unwrap();
        let tag = tags
            .by_name(&rl(&format!("{namespace}:my_damage_tag")))
            .unwrap();
        let set = e(&registries).damage_types.as_ref().unwrap();
        assert_eq!(set, &IdSet::Tag(tag));
        assert!(set.contains(&tags, get::<DamageType>(&registries, "t:a")));
    }
}

#[test]
fn a_missing_reference_of_the_same_stage_is_reported_under_its_registry() {
    let cases = [
        (
            r##"{"damage_types":"#t:my_damage_tag"}"##,
            Diagnostic::Unresolved {
                registry: rl("damage_type"),
                missing: vec![from_e("t:my_damage_tag", true)],
            },
        ),
        (
            r#"{"exclusive_set":["t:e","t:missing"]}"#,
            Diagnostic::Unresolved {
                registry: rl("enchantment"),
                missing: vec![from_e("t:missing", false)],
            },
        ),
    ];
    for (body, expected) in cases {
        assert_eq!(
            report_errors(&load_enchantment(body, &[])),
            [expected],
            "{body}"
        );
    }
}

#[test]
fn a_missing_reference_to_an_earlier_stage_fails_the_entry() {
    for body in [
        r##"{"supported_items":"#t:none"}"##,
        r#"{"supported_items":"t:none"}"#,
    ] {
        let errors = report_errors(&load_enchantment(body, &[]));
        assert!(
            matches!(
                errors.as_slice(),
                [Diagnostic::Entry { registry, entry, message, .. }]
                    if *registry == rl("enchantment") && *entry == rl("t:e") && message.contains("t:none")
            ),
            "{body}: {errors:?}"
        );
    }
}

#[test]
fn entry_sets_accept_a_tag_a_key_or_a_list_without_tags() {
    let registries = load_enchantment(
        r#"{"supported_items":["minecraft:dirt","stone"],"exclusive_set":"t:e"}"#,
        &[],
    )
    .unwrap();
    let value = e(&registries);
    let items = value.supported_items.as_ref().unwrap();
    let tags = registries.tags::<Item>().unwrap();
    assert_eq!(
        items.ids(&tags).collect::<Vec<_>>(),
        [
            get::<Item>(&registries, "dirt"),
            get::<Item>(&registries, "stone")
        ]
    );
    assert_eq!(
        value.exclusive_set,
        Some(IdSet::List(Box::new([get::<Enchantment>(
            &registries,
            "t:e"
        )])))
    );

    assert!(fails_e(&load_enchantment(
        r##"{"supported_items":["stone","#minecraft:x"]}"##,
        &[]
    )));
}

#[test]
fn a_reference_to_a_later_stage_accepts_only_an_inline_value() {
    let inline = load_enchantment(r#"{"merchant_predicate":{"condition":"x"}}"#, &[]).unwrap();
    assert!(matches!(
        e(&inline).merchant_predicate,
        Some(Ref::Inline(_))
    ));

    assert!(fails_e(&load_enchantment(
        r#"{"merchant_predicate":"t:p"}"#,
        &[("data/t/predicate/p.json", "{}")],
    )));
}

#[test]
fn a_reference_by_key_to_a_built_stage_resolves_to_its_id() {
    let damage_a = damage("a");
    let registries = load_enchantment(
        r#"{"damage":"t:a"}"#,
        &[("data/t/damage_type/a.json", &damage_a)],
    )
    .unwrap();
    let target = get::<DamageType>(&registries, "t:a");
    assert!(matches!(e(&registries).damage, Some(Ref::Id(id)) if id == target));
}

#[test]
fn a_reference_miss_the_value_recovers_from_is_not_an_error() {
    let registries = load_enchantment(r#"{"preferred":"t:missing"}"#, &[]).unwrap();
    assert_eq!(e(&registries).preferred, None);

    let errors = report_errors(&load_enchantment(
        r#"{"preferred":"t:missing","supported_items":5}"#,
        &[],
    ));
    assert!(
        matches!(
            errors.as_slice(),
            [Diagnostic::Entry { entry, field, .. }]
                if *entry == rl("t:e") && field.as_deref() == Some("supported_items")
        ),
        "{errors:?}"
    );

    let result = load_enchantment(
        r#"{"preferred":"t:missing","exclusive_set":["t:other"]}"#,
        &[],
    );
    assert_eq!(
        report_errors(&result),
        [Diagnostic::Unresolved {
            registry: rl("enchantment"),
            missing: vec![from_e("t:other", false)],
        }]
    );
}

#[test]
fn an_unknown_key_of_an_earlier_stage_names_the_field_that_holds_it() {
    let errors = report_errors(&load_enchantment(
        r#"{"supported_items":["stone","t:nope"]}"#,
        &[],
    ));
    assert!(
        matches!(
            errors.as_slice(),
            [Diagnostic::Entry { entry, field, .. }]
                if *entry == rl("t:e") && field.as_deref() == Some("supported_items[1]")
        ),
        "{errors:?}"
    );
}
