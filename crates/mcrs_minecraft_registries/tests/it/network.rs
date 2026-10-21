use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::{Declaration, Diagnostic, Stage};

use crate::support::{DamageType, Packs, damage, declarations_with, report_errors, rl};

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
