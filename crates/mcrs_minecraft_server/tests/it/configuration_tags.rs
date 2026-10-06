use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_server::configuration::update_tags;
use mcrs_minecraft_world::registries::test_registries;

const REGISTRIES_WITH_TAGS: &[&str] = &[
    "minecraft:block",
    "minecraft:item",
    "minecraft:enchantment",
    "minecraft:entity_type",
    "minecraft:damage_type",
    "minecraft:dialog",
    "minecraft:timeline",
    "minecraft:banner_pattern",
    "minecraft:instrument",
    "minecraft:painting_variant",
    "minecraft:cat_variant",
    "minecraft:wolf_variant",
    "minecraft:trim_material",
    "minecraft:trim_pattern",
    "minecraft:jukebox_song",
];

const REGISTRIES_WITHOUT_TAGS: &[&str] = &[
    "minecraft:fluid",
    "minecraft:game_event",
    "minecraft:worldgen/biome",
];

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
        let sent: Vec<u16> = group.entries.iter().map(|member| member.0).collect();
        assert_eq!(sent, in_tag_order, "members of {}", group.name);
        if !sent.is_sorted() {
            out_of_id_order.push(group.name.as_str());
        }
    }
    assert!(
        !out_of_id_order.is_empty(),
        "no block tag lists its members out of id order, so the test cannot tell tag order from id order"
    );

    let mut expected: Vec<&str> = REGISTRIES_WITH_TAGS
        .iter()
        .copied()
        .filter(|registry| {
            set.tag_table(registry)
                .is_some_and(|table| !table.is_empty())
        })
        .chain(REGISTRIES_WITHOUT_TAGS.iter().copied())
        .collect();
    expected.sort_unstable();
    let mut sent: Vec<&str> = packet
        .registries
        .iter()
        .map(|registry| registry.registry.as_str())
        .collect();
    sent.sort_unstable();
    assert_eq!(sent, expected, "the registries the packet carries");

    for registry in &packet.registries {
        let name = registry.registry.as_str();
        let mut names: Vec<&str> = registry.tags.iter().map(|tag| tag.name.as_str()).collect();
        names.sort_unstable();
        let mut declared: Vec<&str> = if REGISTRIES_WITHOUT_TAGS.contains(&name) {
            Vec::new()
        } else {
            set.tag_table(name)
                .unwrap_or_else(|| panic!("{name} has no tag table"))
                .names()
                .iter()
                .map(|tag| tag.as_str())
                .collect()
        };
        declared.sort_unstable();
        assert_eq!(names, declared, "the tags {name} declares");
    }
}
