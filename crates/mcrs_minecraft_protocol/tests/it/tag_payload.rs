use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::configuration::clientbound::{RegistryTags, TagGroup};
use mcrs_minecraft_protocol::tags::{tags_from_payload, tags_payload, tags_payload_of};
use mcrs_minecraft_registry::Registered;
use mcrs_minecraft_registry::{Registry, TagId, Tags};
use std::borrow::Cow;
use std::sync::Arc;

struct TestRegistry;

impl Registered for TestRegistry {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:test_registry"));
}

type Name = ResourceLocation<Arc<str>>;

fn name(text: &str) -> Name {
    ResourceLocation::read(text).unwrap()
}

fn registry(count: usize) -> Registry<TestRegistry> {
    Registry::new(
        TestRegistry::REGISTRY,
        (0..count).map(|n| name(&format!("minecraft:e{n}"))),
    )
    .unwrap()
}

fn tag_id(tags: &Tags<TestRegistry>, tag: &str) -> TagId<TestRegistry> {
    tags.get(&TagKey::from_location(name(tag)))
        .unwrap_or_else(|| panic!("no tag {tag}"))
}

fn numbers(tags: &Tags<TestRegistry>, tag: &str) -> Vec<u16> {
    tags.members(tag_id(tags, tag))
        .map(|member| member.number())
        .collect()
}

fn contained(tags: &Tags<TestRegistry>, registry: &Registry<TestRegistry>, tag: &str) -> Vec<u16> {
    let id = tag_id(tags, tag);
    registry
        .ids()
        .filter(|&member| tags.contains(id, member))
        .map(|member| member.number())
        .collect()
}

#[test]
fn tags_round_trip_through_the_payload_in_tag_order() {
    let registry = registry(5);
    let ids = |numbers: &[u16]| -> Vec<_> {
        numbers
            .iter()
            .map(|&number| registry.id(number).unwrap())
            .collect()
    };
    let tags = Tags::from_members(
        &registry,
        vec![
            (name("minecraft:shuffled"), ids(&[3, 1, 4])),
            (name("minecraft:empty"), ids(&[])),
        ],
    );

    let payload = tags_payload_of(tags.table());
    let typed = tags_payload(&tags);
    assert_eq!(typed.registry.as_str(), payload.registry.as_str());
    assert_eq!(typed.tags.len(), payload.tags.len());
    for (typed, untyped) in typed.tags.iter().zip(&payload.tags) {
        assert_eq!(typed.name.as_str(), untyped.name.as_str());
        assert_eq!(typed.entries, untyped.entries);
    }
    assert_eq!(payload.registry.as_str(), "minecraft:test_registry");
    let sent: Vec<(&str, Vec<u16>)> = payload
        .tags
        .iter()
        .map(|group| {
            (
                group.name.as_str(),
                group.entries.iter().map(|id| id.0 as u16).collect(),
            )
        })
        .collect();
    assert_eq!(
        sent,
        [
            ("minecraft:shuffled", vec![3, 1, 4]),
            ("minecraft:empty", vec![])
        ]
    );

    let decoded = tags_from_payload(&registry, &payload);
    for tag in ["minecraft:shuffled", "minecraft:empty"] {
        assert_eq!(numbers(&decoded, tag), numbers(&tags, tag), "{tag} members");
        assert_eq!(
            contained(&decoded, &registry, tag),
            contained(&tags, &registry, tag),
            "{tag} bit set"
        );
    }
    assert_eq!(
        contained(&decoded, &registry, "minecraft:shuffled"),
        [1, 3, 4]
    );
}

#[test]
fn an_unknown_number_in_a_payload_is_dropped() {
    let registry = registry(3);
    let payload = RegistryTags {
        registry: rl!("minecraft:test_registry").into(),
        tags: vec![TagGroup {
            name: ResourceLocation::read_cow(Cow::Borrowed("minecraft:t")).unwrap(),
            entries: [2, 99, 0, 3, -1, 70_000].map(VarInt).to_vec(),
        }],
    };

    let tags = tags_from_payload(&registry, &payload);
    assert_eq!(numbers(&tags, "minecraft:t"), [2, 0]);
    assert_eq!(contained(&tags, &registry, "minecraft:t"), [0, 2]);
}
