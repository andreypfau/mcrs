use std::collections::BTreeSet;

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::{
    Declaration, Diagnostic, NetworkSnapshot, Received, Registries, Stage, load_received,
};

use crate::support::{
    Damage, DamageType, Packs, damage, declarations_with, names, report_errors, rl, tag, value,
};

fn synced_damage() -> Vec<Declaration> {
    declarations_with(
        Declaration::data::<Damage>()
            .synced_as(|damage, _| match damage.message_id.as_str() {
                "t:huge" => Err("the value is too large".to_owned()),
                id => Ok(NbtTag::String(id.to_owned())),
            })
            .into(),
    )
}

#[test]
fn an_entry_that_cannot_be_sent_fails_the_snapshot_naming_it() {
    let packs = Packs::new().world(
        "a",
        &[
            ("data/t/damage_type/fine.json", &damage("t:fine")),
            ("data/t/damage_type/huge.json", &damage("t:huge")),
        ],
    );
    let declarations = synced_damage();
    let (registries, _) = packs.load(&declarations).unwrap();
    let errors = report_errors(&NetworkSnapshot::new(&declarations, &registries));
    assert!(
        matches!(
            &errors[..],
            [Diagnostic::Entry { registry, entry, message, .. }]
                if *registry == rl("damage_type")
                    && *entry == rl("t:huge")
                    && message.contains("too large")
        ),
        "{errors:?}"
    );
}

#[test]
fn only_synced_registries_have_registry_data() {
    let packs = || Packs::new().world("a", &[("data/t/damage_type/fine.json", &damage("t:fine"))]);
    let registry_data = |declarations: &[Declaration]| {
        let (registries, _) = packs().load(declarations).unwrap();
        let snapshot = NetworkSnapshot::new(declarations, &registries).unwrap();
        snapshot.registry_data(false).count()
    };
    assert_eq!(registry_data(&synced_damage()), 1);
    assert_eq!(
        registry_data(&declarations_with(Declaration::data::<Damage>().into())),
        0
    );
}

fn compound(message_id: &str) -> NbtTag {
    mcrs_minecraft_nbt::to_nbt_tag(&crate::support::Damage {
        message_id: message_id.to_owned(),
        exhaustion: 0.5,
    })
    .unwrap()
}

fn receive(received: &Received) -> Result<Registries, Vec<String>> {
    load_received(&synced_damage(), received, None)
        .map(|(registries, _)| registries)
        .map_err(|report| report.iter().map(ToString::to_string).collect())
}

#[test]
fn a_client_numbers_received_entries_in_the_order_sent() {
    let received = Received {
        registries: vec![(
            rl("damage_type"),
            vec![
                (rl("t:z"), Some(compound("t:z"))),
                (rl("t:a"), Some(compound("t:a"))),
            ],
        )],
        tags: vec![(rl("damage_type"), vec![(rl("t:hot"), vec![1, 0, 9])])],
    };
    let registries = receive(&received).unwrap();
    assert_eq!(names::<DamageType>(&registries), ["t:z", "t:a"]);
    assert_eq!(value::<Damage>(&registries, "t:a").message_id, "t:a");
    assert_eq!(
        tag::<DamageType>(&registries, "t:hot"),
        Some(vec!["t:a".into(), "t:z".into()])
    );
}

#[test]
fn an_entry_sent_twice_fails_the_load() {
    let received = Received {
        registries: vec![(
            rl("damage_type"),
            vec![
                (rl("t:a"), Some(compound("t:a"))),
                (rl("t:a"), Some(compound("t:a"))),
            ],
        )],
        tags: Vec::new(),
    };
    let errors = receive(&received).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("sent twice")),
        "{errors:?}"
    );
}

#[test]
fn an_entry_sent_without_data_takes_the_data_of_the_known_entry() {
    let declarations = declarations_with(Declaration::data::<Damage>().synced().into());
    let (known, _) = Packs::new()
        .world(
            "a",
            &[
                ("data/t/damage_type/a.json", &damage("t:known_a")),
                ("data/t/damage_type/b.json", &damage("t:known_b")),
            ],
        )
        .load(&declarations)
        .unwrap();
    let received = Received {
        registries: vec![(
            rl("damage_type"),
            vec![(rl("t:b"), None), (rl("t:a"), Some(compound("t:sent_a")))],
        )],
        tags: Vec::new(),
    };

    let (client, _) = load_received(&declarations, &received, Some(&known)).unwrap();
    assert_eq!(names::<DamageType>(&client), ["t:b", "t:a"]);
    assert_eq!(value::<Damage>(&client, "t:b").message_id, "t:known_b");
    assert_eq!(value::<Damage>(&client, "t:a").message_id, "t:sent_a");

    let errors = report_errors(&load_received(&declarations, &received, None));
    assert!(
        matches!(&errors[..], [Diagnostic::Entry { entry, .. }] if *entry == rl("t:b")),
        "{errors:?}"
    );
}

#[test]
fn a_registry_read_by_its_names_holds_the_names_and_tags_sent() {
    let mut declarations = synced_damage();
    declarations.push(Declaration::names_only(rl("t:names"), Stage::World));
    let received = Received {
        registries: vec![(rl("t:names"), vec![(rl("t:x"), None), (rl("t:y"), None)])],
        tags: vec![(rl("t:names"), vec![(rl("t:all"), vec![1, 0])])],
    };

    let (registries, _) = load_received(&declarations, &received, None).unwrap();
    assert_eq!(
        registries.names_of(&rl("t:names")),
        Some(&[rl("t:x"), rl("t:y")][..])
    );
    let tags: Vec<_> = registries
        .tags_of(&rl("t:names"))
        .unwrap()
        .map(|(name, members)| (name.clone(), members.iter().copied().collect()))
        .collect();
    assert_eq!(tags, [(rl("t:all"), BTreeSet::from([0, 1]))]);
}
