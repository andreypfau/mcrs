use mcrs_minecraft_instrument::Instrument;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_text::ItemHover;

fn read(use_duration: &str, range: &str, durability_damage: &str) -> serde_json::Result<()> {
    let entry = format!(
        r#"{{"sound_event":"minecraft:item.goat_horn.sound.0","use_duration":{use_duration},"range":{range},"description":"x","durability_damage":{durability_damage}}}"#
    );
    serde_json::from_str::<Instrument<ItemHover, Named>>(&entry).map(|_| ())
}

#[test]
fn a_value_out_of_its_range_is_an_error_of_the_entry() {
    assert!(read("0.0", "256.0", "0").is_ok(), "the base entry is valid");

    let refused: [(&str, &str, &str); 5] = [
        ("-0.0", "256.0", "0"),
        ("3.5e38", "256.0", "0"),
        ("0.0", "0.0", "0"),
        ("0.0", "3.5e38", "0"),
        ("0.0", "256.0", "-1"),
    ];
    for (use_duration, range, durability_damage) in refused {
        assert!(
            read(use_duration, range, durability_damage).is_err(),
            "{use_duration} {range} {durability_damage}"
        );
    }
}
