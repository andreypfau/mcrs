use std::collections::BTreeMap;

use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::registry_load::{
    KnownEntries, load_received, load_vanilla_registries, sent_by, test_packs, test_registries,
    test_registries_arc,
};

const REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

type Contents = BTreeMap<String, (Vec<String>, Vec<(String, Vec<u16>)>)>;

fn contents(set: &RegistrySet, registries: &[String]) -> Contents {
    registries
        .iter()
        .map(|registry| {
            let names = set
                .table(registry)
                .map(|table| table.names().iter().map(ToString::to_string).collect())
                .unwrap_or_default();
            let tags = set
                .tag_table(registry)
                .map(|table| {
                    (0..table.names().len())
                        .map(|tag| (table.names()[tag].to_string(), table.members(tag).to_vec()))
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
        let received = sent_by(test_registries_arc(), known);
        let sent: Vec<String> = received
            .0
            .iter()
            .map(|(key, _)| key.to_string())
            .chain(received.1.iter().map(|(key, _)| key.to_string()))
            .collect();
        let client = load_received(REPORT, local, received)
            .unwrap_or_else(|error| panic!("known {known}: {error}"));
        assert_eq!(
            contents(&client.set, &sent),
            contents(test_registries(), &sent),
            "known packs negotiated: {known}"
        );
    }
}
