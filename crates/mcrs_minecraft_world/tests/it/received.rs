use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_world::registry_load::{
    KnownEntries, load_received, load_vanilla_registries, sent_by, test_packs, test_registries,
};

const REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

type Contents = BTreeMap<String, (Vec<String>, Vec<(String, Vec<u16>)>)>;

fn contents(set: &Registries, registries: &[String]) -> Contents {
    registries
        .iter()
        .map(|registry| {
            let key = ResourceLocation::read(registry).unwrap();
            let names = set
                .names_of(&key)
                .map(|names| names.iter().map(ToString::to_string).collect())
                .unwrap_or_default();
            let tags = set
                .tags_of(&key)
                .map(|tags| {
                    tags.map(|(name, members)| (name.to_string(), members.to_vec()))
                        .collect()
                })
                .unwrap_or_default();
            (registry.clone(), (names, tags))
        })
        .collect()
}

#[test]
fn a_client_holds_the_synced_registries_and_tags_the_server_holds() {
    let vanilla = load_vanilla_registries(&test_packs()).unwrap();
    let local = KnownEntries::of(&vanilla.registries).unwrap();
    for (known, local) in [(true, Some(&local)), (false, None)] {
        let received = sent_by(test_registries(), known);
        let sent: Vec<String> = received
            .0
            .iter()
            .map(|(key, _)| key.to_string())
            .chain(received.1.iter().map(|(key, _)| key.to_string()))
            .collect();
        let client = load_received(REPORT, local, received)
            .unwrap_or_else(|error| panic!("known {known}: {error}"));
        assert_eq!(
            contents(&client.registries, &sent),
            contents(test_registries(), &sent),
            "known packs negotiated: {known}"
        );
    }
}
