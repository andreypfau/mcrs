use mcrs_minecraft_registries::{Diagnostic, IdSet, Missing, Ref, Referrer};

use crate::support::{
    DamageType, Enchantment, Item, Packs, damage, declarations, get, report_errors, rl,
};

fn enchantment(body: &str) -> (&'static str, String) {
    ("data/t/enchantment/e.json", body.to_owned())
}

fn load_enchantment(
    body: &str,
    extra: &[(&str, &str)],
) -> Result<mcrs_minecraft_registries::Registries, mcrs_minecraft_registries::Diagnostics> {
    let (path, body) = enchantment(body);
    let mut files: Vec<(&str, &str)> = vec![(path, &body)];
    files.extend_from_slice(extra);
    Packs::new()
        .world("a", &files)
        .load(&declarations())
        .map(|(registries, _)| registries)
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
        let registry = registries.registry::<Enchantment>().unwrap();
        let set = registry
            .get(get::<Enchantment>(&registries, "t:e"))
            .unwrap()
            .damage_types
            .as_ref()
            .unwrap();
        assert_eq!(set, &IdSet::Tag(tag));
        assert!(set.contains(tags, get::<DamageType>(&registries, "t:a")));
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
        let report = load_enchantment(body, &[]).unwrap_err();
        assert_eq!(report_errors(&report), [expected], "{body}");
    }
}

#[test]
fn a_missing_reference_to_an_earlier_stage_fails_the_entry() {
    for body in [
        r##"{"supported_items":"#t:none"}"##,
        r#"{"supported_items":"t:none"}"#,
    ] {
        let report = load_enchantment(body, &[]).unwrap_err();
        let errors = report_errors(&report);
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
    let value = registries
        .registry::<Enchantment>()
        .unwrap()
        .get(get::<Enchantment>(&registries, "t:e"))
        .unwrap();
    let items = value.supported_items.as_ref().unwrap();
    let tags = registries.tags::<Item>().unwrap();
    assert_eq!(
        items.ids(tags).collect::<Vec<_>>(),
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

    let report =
        load_enchantment(r##"{"supported_items":["stone","#minecraft:x"]}"##, &[]).unwrap_err();
    assert!(
        report_errors(&report)
            .iter()
            .any(|d| matches!(d, Diagnostic::Entry { entry, .. } if *entry == rl("t:e")))
    );
}

#[test]
fn a_reference_to_a_later_stage_accepts_only_an_inline_value() {
    let inline = load_enchantment(r#"{"merchant_predicate":{"condition":"x"}}"#, &[]).unwrap();
    let value = inline
        .registry::<Enchantment>()
        .unwrap()
        .get(get::<Enchantment>(&inline, "t:e"))
        .unwrap();
    assert!(matches!(value.merchant_predicate, Some(Ref::Inline(_))));

    let report = load_enchantment(
        r#"{"merchant_predicate":"t:p"}"#,
        &[("data/t/predicate/p.json", "{}")],
    )
    .unwrap_err();
    assert!(
        report_errors(&report)
            .iter()
            .any(|d| matches!(d, Diagnostic::Entry { entry, .. } if *entry == rl("t:e")))
    );
}

#[test]
fn a_reference_miss_the_value_recovers_from_is_not_an_error() {
    let registries = load_enchantment(r#"{"preferred":"t:missing"}"#, &[]).unwrap();
    let value = registries
        .registry::<Enchantment>()
        .unwrap()
        .get(get::<Enchantment>(&registries, "t:e"))
        .unwrap();
    assert_eq!(value.preferred, None);

    let report =
        load_enchantment(r#"{"preferred":"t:missing","supported_items":5}"#, &[]).unwrap_err();
    let errors = report_errors(&report);
    assert!(
        matches!(
            errors.as_slice(),
            [Diagnostic::Entry { entry, field, .. }]
                if *entry == rl("t:e") && field.as_deref() == Some("supported_items")
        ),
        "{errors:?}"
    );

    let report = load_enchantment(
        r#"{"preferred":"t:missing","exclusive_set":["t:other"]}"#,
        &[],
    )
    .unwrap_err();
    assert_eq!(
        report_errors(&report),
        [Diagnostic::Unresolved {
            registry: rl("enchantment"),
            missing: vec![from_e("t:other", false)],
        }]
    );
}
