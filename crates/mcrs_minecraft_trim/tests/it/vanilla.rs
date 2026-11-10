use mcrs_minecraft_fixtures::assert_vanilla_round_trip;
use mcrs_minecraft_text::ItemHover;
use mcrs_minecraft_trim::TrimMaterial;
use mcrs_minecraft_trim::TrimPattern;

#[test]
fn every_vanilla_trim_file_writes_back_as_read() {
    assert_vanilla_round_trip::<TrimMaterial<ItemHover>>("trim_material");
    assert_vanilla_round_trip::<TrimPattern<ItemHover>>("trim_pattern");
}

#[test]
fn a_trim_pattern_without_decal_reads_as_no_decal_and_writes_it() {
    let json = r#"{"asset_id":"minecraft:coast","description":"Coast"}"#;
    let pattern: TrimPattern<ItemHover> = serde_json::from_str(json).unwrap();
    assert!(!pattern.decal, "an absent decal reads as false");
    assert_eq!(
        serde_json::to_string(&pattern).unwrap(),
        r#"{"asset_id":"minecraft:coast","decal":false,"description":"Coast"}"#
    );
}
