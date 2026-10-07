use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::{RegistrySet, Tags};
use mcrs_minecraft_worldgen_feature::proto::{Feature, PlacedFeature};
use mcrs_minecraft_worldgen_testing::corpus_set;
use std::sync::Arc;

fn with_tag(members: &[&str]) -> RegistrySet {
    let names = corpus_set().registry::<PlacedFeature>().unwrap();
    let members = members
        .iter()
        .map(|name| names.by_name(name).unwrap())
        .collect();
    let tags = Tags::from_members(
        &names,
        vec![(ResourceLocation::read("test:trees").unwrap(), members)],
    );
    corpus_set().clone().with_tags(Arc::clone(tags.table()))
}

#[test]
fn a_placed_feature_tag_reads_as_its_members_and_writes_back_as_the_tag() {
    let members = ["minecraft:oak", "minecraft:birch_bees_0002"];
    with_tag(&members).scope(|| {
        let json = r##"{"type":"minecraft:simple_random_selector","features":"#test:trees"}"##;
        let read: Feature = serde_json::from_str(json).unwrap();
        let mut visited = Vec::new();
        read.visit_placed_features(&mut |holder| visited.push(holder.clone()));
        assert_eq!(
            visited,
            members.map(|name| Holder::Reference(ResourceLocation::read(name).unwrap()))
        );
        assert_eq!(serde_json::to_string(&read).unwrap(), json);

        let error = serde_json::from_str::<Feature>(
            r##"{"type":"minecraft:sequence","features":"#test:no_such_tag"}"##,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("test:no_such_tag"), "{error}");
    });
}
