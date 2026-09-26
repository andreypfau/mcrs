use mcrs_minecraft_light_color::asset::{BlockStateRef, LightColor, LightColorFile};

fn json(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}

#[test]
fn a_colour_round_trips_as_lowercase_hex() {
    let colour: LightColor = serde_json::from_str(&json("#36d9e6")).unwrap();
    assert_eq!(colour, LightColor([0x36, 0xd9, 0xe6]));
    assert_eq!(serde_json::to_string(&colour).unwrap(), json("#36d9e6"));
    let upper: LightColor = serde_json::from_str(&json("#36D9E6")).unwrap();
    assert_eq!(serde_json::to_string(&upper).unwrap(), json("#36d9e6"));
}

#[test]
fn malformed_colours_are_rejected() {
    for text in [
        "36d9e6", "#36d9e", "#gggggg", "#+6d9e6", "#36d9e6f", "#-6d9e6", "",
    ] {
        assert!(
            serde_json::from_str::<LightColor>(&json(text)).is_err(),
            "`{text}` was accepted"
        );
    }
}

#[test]
fn state_references_round_trip_unchanged() {
    for text in [
        "minecraft:candle",
        "#minecraft:candles",
        "minecraft:candle[lit=true]",
        "minecraft:candle[candles=4,lit=true]",
        "#minecraft:candles[lit=true]",
    ] {
        let parsed: BlockStateRef = serde_json::from_str(&json(text)).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json(text));
    }
}

#[test]
fn malformed_state_references_are_rejected() {
    for text in [
        "",
        "#",
        "minecraft:candle[",
        "minecraft:candle]",
        "minecraft:candle[]",
        "minecraft:candle[lit]",
        "minecraft:candle[lit=true,lit=false]",
        "minecraft:candle[=true]",
        "minecraft:candle[lit=]",
        "minecraft:candle[lit=true]x",
        "minecraft:candle[lit=true]]",
        "minecraft:candle[[lit=true]",
        "Minecraft:Candle",
    ] {
        assert!(
            serde_json::from_str::<BlockStateRef>(&json(text)).is_err(),
            "`{text}` was accepted"
        );
    }
}

#[test]
fn unknown_fields_are_rejected() {
    let text = r##"{ "color": "#36d9e6", "blocks": [], "priority": 1 }"##;
    assert!(serde_json::from_str::<LightColorFile>(text).is_err());
    let text = r##"{ "color": "#36d9e6", "blocks": [] }"##;
    assert!(serde_json::from_str::<LightColorFile>(text).is_ok());
}
