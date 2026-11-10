use mcrs_minecraft_fixtures::assert_vanilla_round_trip;
use mcrs_minecraft_painting_variant::PaintingVariant;
use mcrs_minecraft_text::ItemHover;

#[test]
fn every_vanilla_painting_variant_writes_back_as_its_file() {
    assert_vanilla_round_trip::<PaintingVariant<ItemHover>>("painting_variant");
}

#[test]
fn a_side_outside_one_to_sixteen_is_refused() {
    for side in [0, 17] {
        let json = format!(r#"{{"asset_id":"minecraft:kebab","height":1,"width":{side}}}"#);
        let read = serde_json::from_str::<PaintingVariant<ItemHover>>(&json);
        assert!(read.is_err(), "width {side} is outside 1 to 16");
    }
}
