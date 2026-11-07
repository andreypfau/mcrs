//! What a data pack may write, and what the reader makes of it.

use mcrs_minecraft_registries::Named;
use serde_json::json;

use crate::attribute::{Argument, AttributeValue, ModifierMap, Operation, argb_hex};
use crate::environment_attribute_keys::EnvironmentAttribute;
use crate::timeline::Timeline;

fn track_with_ticks(ticks: &[u32]) -> serde_json::Value {
    let keyframes: Vec<serde_json::Value> = ticks
        .iter()
        .map(|tick| json!({"ticks": tick, "value": 1.0}))
        .collect();
    json!({
        "clock": "minecraft:overworld",
        "tracks": {"minecraft:visual/cloud_height": {"keyframes": keyframes}},
    })
}

#[test]
fn a_track_refuses_a_run_of_keyframes_on_one_tick() {
    let loads = |ticks: &[u32]| serde_json::from_value::<Timeline<Named>>(track_with_ticks(ticks));
    assert!(loads(&[0, 50, 50, 50]).is_ok());
    assert!(loads(&[50, 50, 50]).is_err());
    assert!(loads(&[0, 50, 50, 50, 50]).is_err());
    assert!(loads(&[50, 0]).is_err());
    assert!(loads(&[]).is_err());
}

#[test]
fn a_keyframe_may_sit_on_the_period_but_a_time_marker_may_not() {
    let mut on_period = track_with_ticks(&[0, 24000]);
    on_period["period_ticks"] = json!(24000);
    assert!(serde_json::from_value::<Timeline<Named>>(on_period.clone()).is_ok());

    on_period["time_markers"] = json!({"test:end": 24000});
    assert!(serde_json::from_value::<Timeline<Named>>(on_period).is_err());
}

#[test]
fn an_override_object_reads_as_the_bare_value() {
    let map: ModifierMap = serde_json::from_value(json!({
        "minecraft:audio/background_music": {"argument": {}, "modifier": "override"},
        "minecraft:visual/water_fog_end_distance": {"argument": 0.85, "modifier": "multiply"},
    }))
    .unwrap();
    let music = map.get(EnvironmentAttribute::AudioBackgroundMusic).unwrap();
    assert_eq!(music.modifier, Operation::Override);
    assert_eq!(
        serde_json::to_string(&map).unwrap(),
        r#"{"minecraft:audio/background_music":{},"minecraft:visual/water_fog_end_distance":{"argument":0.85,"modifier":"multiply"}}"#
    );
}

#[test]
fn only_a_value_form_is_held_to_the_range_of_its_attribute() {
    let read = |entry: serde_json::Value| {
        serde_json::from_value::<ModifierMap>(json!({"minecraft:visual/star_brightness": entry}))
    };
    let refused = read(json!(1.5)).unwrap_err().to_string();
    assert!(refused.contains("1.5 is not in range [0; 1]"), "{refused}");
    assert!(read(json!({"argument": 1.5, "modifier": "override"})).is_err());
    assert!(read(json!({"argument": 1.5, "modifier": "add"})).is_ok());
    assert!(read(json!({"argument": true, "modifier": "and"})).is_err());
}

#[test]
fn a_colour_writes_back_as_the_hex_it_was_read_from() {
    for byte in 0..=255u32 {
        let packed = byte << 24 | byte << 16 | (255 - byte) << 8 | byte;
        let hex = format!("#{packed:08x}");
        let map: ModifierMap =
            serde_json::from_value(json!({"minecraft:visual/cloud_color": hex})).unwrap();
        let entry = map.get(EnvironmentAttribute::VisualCloudColor).unwrap();
        let Argument::Value(value @ AttributeValue::Argb(channels)) = &entry.argument else {
            panic!("{hex} is not an ARGB colour");
        };
        assert_eq!(argb_hex(*channels), hex);
        assert_eq!(value.packed_argb(), Some(packed));
    }
    assert_eq!(argb_hex([1.5, -0.5, 0.5, 1.0]), "#ffff007f");
}
