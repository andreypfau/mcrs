use mcrs_minecraft_block_predicate::provider::Holder;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen_feature::compile::LoadedFeatures;
use mcrs_minecraft_worldgen_feature::proto::Feature;

fn id(name: &str) -> ResourceLocationBuf {
    ResourceLocationBuf::read(name).unwrap()
}

fn features_of(json: &str) -> (Feature, String) {
    let read: Feature = serde_json::from_str(json).unwrap();
    let written = serde_json::to_string(&read).unwrap();
    (read, written)
}

#[test]
fn a_placed_feature_tag_writes_back_as_the_tag_and_expands_at_compile() {
    let members = ["minecraft:oak", "minecraft:birch_bees_0002"];
    let mut loaded = LoadedFeatures::default();
    loaded
        .placed_feature_tags
        .insert(id("test:trees"), members.map(id).to_vec());

    let json = r##"{"type":"minecraft:simple_random_selector","features":"#test:trees"}"##;
    let (read, written) = features_of(json);
    assert_eq!(written, json);
    let Feature::SimpleRandomSelector { features } = &read else {
        panic!("{read:?}");
    };
    assert_eq!(
        loaded.placed_set(features).unwrap().as_ref(),
        members.map(|name| Holder::Reference(id(name)))
    );

    let (read, _) =
        features_of(r##"{"type":"minecraft:sequence","features":"#test:no_such_tag"}"##);
    let Feature::Sequence { features } = &read else {
        panic!("{read:?}");
    };
    assert_eq!(
        loaded.placed_set(features).unwrap_err().to_string(),
        "unknown placed feature tag: #test:no_such_tag"
    );
}
