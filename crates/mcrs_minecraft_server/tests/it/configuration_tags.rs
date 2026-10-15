use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundUpdateTags;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_server::configuration::update_tags;
use mcrs_minecraft_world::packs::load_from_directory;
use mcrs_minecraft_world::registry_load::test_registries;

use crate::configuration_registries::ASSETS;

type SentTags = Vec<(String, BTreeMap<String, BTreeSet<String>>)>;

fn sent_tags(set: &RegistrySet, packet: &ClientboundUpdateTags) -> SentTags {
    packet
        .registries
        .iter()
        .map(|registry| {
            let name = registry.registry.as_str();
            let table = set
                .table(name)
                .unwrap_or_else(|| panic!("{name} has no table"));
            let tags = registry
                .tags
                .iter()
                .map(|tag| {
                    let members = tag
                        .entries
                        .iter()
                        .map(|member| {
                            let member = usize::try_from(member.0).unwrap();
                            table.name(member).unwrap().to_string()
                        })
                        .collect();
                    (tag.name.to_string(), members)
                })
                .collect();
            (name.to_owned(), tags)
        })
        .collect()
}

#[test]
fn the_tags_packet_names_the_tags_and_members_the_old_loader_sends() {
    let old = load_from_directory(Path::new(ASSETS)).unwrap_or_else(|report| panic!("{report}"));
    let new = test_registries();
    let (from_old, from_new) = (update_tags(&old), update_tags(new));
    let (old_tags, new_tags) = (sent_tags(&old, &from_old), sent_tags(new, &from_new));
    let registries = |tags: &SentTags| {
        tags.iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(registries(&old_tags), registries(&new_tags));
    for ((registry, old), (_, new)) in old_tags.iter().zip(&new_tags) {
        assert_eq!(old, new, "{registry}");
    }
}

#[test]
fn the_server_sends_tag_members_in_tag_order() {
    let set = test_registries();
    let packet = update_tags(set);

    let blocks = set.tags::<Block>().expect("the block tags are loaded");
    let sent_blocks = packet
        .registries
        .iter()
        .find(|registry| registry.registry.as_str() == "minecraft:block")
        .expect("block tags are sent");
    let mut out_of_id_order = Vec::new();
    for group in &sent_blocks.tags {
        let id = blocks
            .tag_ids()
            .find(|&id| blocks.name(id).as_str() == group.name.as_str())
            .expect("a sent block tag is in the set");
        let in_tag_order: Vec<u16> = blocks.members(id).map(|member| member.number()).collect();
        let sent: Vec<u16> = group.entries.iter().map(|member| member.0 as u16).collect();
        assert_eq!(sent, in_tag_order, "members of {}", group.name);
        if !sent.is_sorted() {
            out_of_id_order.push(group.name.as_str());
        }
    }
    assert!(
        !out_of_id_order.is_empty(),
        "no block tag lists its members out of id order, so the test cannot tell tag order from id order"
    );

    for registry in &packet.registries {
        let name = registry.registry.as_str();
        let table = set
            .tag_table(name)
            .unwrap_or_else(|| panic!("{name} has no tag table"));
        let sent: Vec<&str> = registry.tags.iter().map(|tag| tag.name.as_str()).collect();
        let declared: Vec<&str> = table.names().iter().map(|tag| tag.as_str()).collect();
        assert_eq!(sent, declared, "the tags {name} declares, in table order");
        for (index, group) in registry.tags.iter().enumerate() {
            let members: Vec<u16> = group.entries.iter().map(|member| member.0 as u16).collect();
            assert_eq!(
                members,
                table.members(index),
                "members of {name} {}",
                group.name
            );
        }
    }
}

#[test]
fn the_tags_packet_follows_the_static_registry_order() {
    let set = test_registries();
    let packet = update_tags(set);
    let synced: Vec<&str> = set
        .synced()
        .map(|(table, _)| table.registry().as_str())
        .collect();
    let statics_sent: Vec<&str> = packet
        .registries
        .iter()
        .map(|registry| registry.registry.as_str())
        .filter(|registry| !synced.contains(registry))
        .collect();

    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let positions: Vec<u64> = statics_sent
        .iter()
        .map(|registry| {
            report[*registry]["protocol_id"]
                .as_u64()
                .unwrap_or_else(|| panic!("{registry} has no protocol id in the report"))
        })
        .collect();
    assert!(
        positions.is_sorted_by(|a, b| a < b),
        "static registries follow the registry table's order: {statics_sent:?}"
    );
    assert!(
        !statics_sent.is_sorted(),
        "the table's order matches the name order, so the test cannot tell them apart"
    );
}

#[test]
fn every_static_and_synced_registry_with_tags_is_sent_and_no_other() {
    let set = test_registries();
    let packet = update_tags(set);
    let sent: Vec<&str> = packet
        .registries
        .iter()
        .map(|registry| registry.registry.as_str())
        .collect();

    let mut distinct = sent.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        sent.len(),
        "a registry is sent twice: {sent:?}"
    );

    let synced: Vec<&str> = set
        .synced()
        .map(|(table, _)| table.registry().as_str())
        .collect();
    let has_tags = |registry: &str| {
        set.tag_table(registry)
            .is_some_and(|table| !table.is_empty())
    };

    let synced_sent: Vec<&str> = sent
        .iter()
        .copied()
        .take_while(|registry| synced.contains(registry))
        .collect();
    let expected_synced: Vec<&str> = synced.iter().copied().filter(|r| has_tags(r)).collect();
    assert_eq!(
        synced_sent, expected_synced,
        "synced registries come first, in declaration order"
    );

    let statics_sent = &sent[synced_sent.len()..];
    for registry in statics_sent {
        assert!(
            !synced.contains(registry) && !set.is_world_registry(registry),
            "{registry} is neither synced nor static"
        );
    }

    for table in set.tables() {
        let registry = table.registry().as_str();
        let networked = synced.contains(&registry) || !set.is_world_registry(registry);
        assert_eq!(
            sent.contains(&registry),
            networked && has_tags(registry),
            "whether {registry} is sent"
        );
    }

    for registry in [
        "minecraft:fluid",
        "minecraft:game_event",
        "minecraft:potion",
    ] {
        assert!(
            sent.contains(&registry),
            "{registry} carries tags and is static"
        );
    }
    assert!(
        set.tag_table("minecraft:villager_trade")
            .is_some_and(|table| !table.is_empty())
            && !sent.contains(&"minecraft:villager_trade"),
        "a world registry the game does not synchronize is not sent even with tags"
    );

    let biomes = packet
        .registries
        .iter()
        .find(|registry| registry.registry.as_str() == "minecraft:worldgen/biome")
        .expect("biome tags are sent");
    assert!(!biomes.tags.is_empty());
    assert!(
        biomes.tags.iter().any(|tag| !tag.entries.is_empty()),
        "biome tags carry their members"
    );
}
