use std::collections::BTreeMap;

use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::packets::configuration::{
    ClientboundRegistryData, ClientboundUpdateTags,
};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::registry_load::{
    Collected, KnownEntries, load_received, load_vanilla_registries, test_packs, test_registries,
    test_registries_arc,
};

const REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

/// What a client collects from the bodies the server's load encoded.
fn received(known: bool) -> Collected {
    let network = test_registries_arc().network();
    let registries = network
        .registry_data(known)
        .map(|body| {
            let packet = ClientboundRegistryData::decode(&mut &body[..]).unwrap();
            let entries = packet
                .entries
                .into_iter()
                .map(|entry| (entry.id.into(), entry.data.map(|data| data.into_owned())))
                .collect();
            (packet.registry.into(), entries)
        })
        .collect();
    let tags = ClientboundUpdateTags::decode(&mut network.tags())
        .unwrap()
        .registries
        .into_iter()
        .map(|registry| {
            let tags = registry
                .tags
                .into_iter()
                .map(|tag| (tag.name.into(), tag.entries.iter().map(|id| id.0).collect()))
                .collect();
            (registry.registry.into(), tags)
        })
        .collect();
    (registries, tags)
}

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
        let received = received(known);
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
