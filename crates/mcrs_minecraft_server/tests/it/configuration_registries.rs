use mcrs_minecraft_assets::packs::VANILLA_PACK;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_protocol::Encode;
use mcrs_minecraft_registry::RegistryLookup;
use mcrs_minecraft_server::configuration::{known_pack_offer, registry_data};
use mcrs_minecraft_world::registries::test_registries;
use std::collections::HashSet;

#[test]
fn the_registry_packets_carry_the_synced_registries_in_declared_order() {
    let set = test_registries();
    let no_known_packs = HashSet::new();
    let packets = registry_data(set, &no_known_packs);

    let sent: Vec<&str> = packets
        .iter()
        .map(|packet| packet.registry.as_str())
        .collect();
    let synced: Vec<&str> = set
        .synced()
        .map(|(table, _)| table.registry().as_str())
        .collect();
    assert_eq!(sent.len(), 32);
    assert_eq!(sent, synced);
    assert!(!sent.contains(&"minecraft:environment_attribute"));

    for packet in &packets {
        let table = set.table(packet.registry.as_str()).unwrap();
        let names: Vec<&str> = packet
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let in_id_order: Vec<&str> = table.names().iter().map(|name| name.as_str()).collect();
        assert_eq!(names, in_id_order, "entries of {}", packet.registry);
        assert!(
            packet.entries.iter().all(|entry| entry.data.is_some()),
            "an entry of {} is sent without data",
            packet.registry
        );
    }

    let mut bytes = 0;
    for packet in &packets {
        let mut buffer = Vec::new();
        packet.encode(&mut buffer).unwrap();
        bytes += buffer.len();
    }
    println!(
        "the {} registry packets encode to {bytes} bytes",
        packets.len()
    );
}

#[test]
fn an_entry_of_a_known_pack_is_sent_without_its_data() {
    let set = test_registries();
    let core = HashSet::from([("minecraft", "core")]);
    let packets = registry_data(set, &core);

    let mut from_other_packs = 0;
    for packet in &packets {
        let registry = packet.registry.as_str();
        for (id, entry) in packet.entries.iter().enumerate() {
            let vanilla = set.pack_of(registry, id) == Some(VANILLA_PACK);
            assert_eq!(
                entry.data.is_none(),
                vanilla,
                "{registry}/{}: data is sent exactly when the entry is not from the vanilla pack",
                entry.id
            );
            from_other_packs += usize::from(!vanilla);
        }
    }
    assert!(
        from_other_packs > 0,
        "no entry comes from another pack, so the test cannot tell the skip from sending nothing"
    );
}

#[test]
fn the_bridge_numbers_world_entries_as_the_registry_packets_do() {
    let set = test_registries();
    let packets = registry_data(set, &HashSet::new());
    assert!(!packets.is_empty());

    for packet in &packets {
        let path = packet.registry.path();
        for (position, entry) in packet.entries.iter().enumerate() {
            let name = ResourceLocation::read(entry.id.as_str()).unwrap();
            let number = u16::try_from(position).unwrap();
            assert_eq!(
                RegistryLookup::id(set, path, &name),
                Some(number),
                "{path}/{name}"
            );
            assert_eq!(
                RegistryLookup::name(set, path, number),
                Some(&name),
                "{path}/{number}"
            );
        }
    }
}

#[test]
fn the_known_pack_offer_follows_the_plugin() {
    let offered = known_pack_offer(true);
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].namespace, "minecraft");
    assert_eq!(offered[0].id, "core");
    assert_eq!(offered[0].version, VERSION.id);
    assert!(known_pack_offer(false).is_empty());
}
