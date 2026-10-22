use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::{
    Declaration, Diagnostic, Received, Registries, Stage, load_received,
};

use crate::support::{
    DamageType, Packs, damage, declarations_with, names, report_errors, rl, tag, value,
};

fn synced_damage() -> Vec<Declaration> {
    declarations_with(
        Declaration::data::<DamageType>(rl("damage_type"), Stage::World)
            .synced(|damage, _| match damage.message_id.as_str() {
                "t:huge" => Err("the value is too large".to_owned()),
                id => Ok(NbtTag::String(id.to_owned())),
            })
            .into(),
    )
}

#[test]
fn an_entry_that_cannot_be_sent_fails_the_load_naming_it() {
    let packs = Packs::new().world(
        "a",
        &[
            ("data/t/damage_type/fine.json", &damage("t:fine")),
            ("data/t/damage_type/huge.json", &damage("t:huge")),
        ],
    );
    let errors = report_errors(&packs.load(&synced_damage()));
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
    let packs = Packs::new().world("a", &[("data/t/damage_type/fine.json", &damage("t:fine"))]);
    let (synced, _) = packs.load(&synced_damage()).unwrap();
    assert_eq!(synced.network().registry_data(false).count(), 1);
    let (unsynced, _) = packs
        .load(&declarations_with(
            Declaration::data::<DamageType>(rl("damage_type"), Stage::World).into(),
        ))
        .unwrap();
    assert_eq!(unsynced.network().registry_data(false).count(), 0);
}

fn compound(message_id: &str) -> NbtTag {
    mcrs_minecraft_nbt::to_nbt_tag(&crate::support::Damage {
        message_id: message_id.to_owned(),
        exhaustion: 0.5,
    })
    .unwrap()
}

fn receive(received: &Received) -> Result<Registries, Vec<String>> {
    load_received(&synced_damage(), received)
        .map(|(registries, _)| registries)
        .map_err(|report| report.iter().map(ToString::to_string).collect())
}

#[test]
fn a_client_numbers_received_entries_in_the_order_sent() {
    let received = Received {
        registries: vec![(
            rl("damage_type"),
            vec![(rl("t:z"), compound("t:z")), (rl("t:a"), compound("t:a"))],
        )],
        tags: vec![(rl("damage_type"), vec![(rl("t:hot"), vec![1, 0, 9])])],
    };
    let registries = receive(&received).unwrap();
    assert_eq!(names::<DamageType>(&registries), ["t:z", "t:a"]);
    assert_eq!(value::<DamageType>(&registries, "t:a").message_id, "t:a");
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
            vec![(rl("t:a"), compound("t:a")), (rl("t:a"), compound("t:a"))],
        )],
        tags: Vec::new(),
    };
    let errors = receive(&received).unwrap_err();
    assert!(
        errors.iter().any(|error| error.contains("sent twice")),
        "{errors:?}"
    );
}
