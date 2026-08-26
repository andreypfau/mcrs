use std::path::PathBuf;

use mcrs_minecraft_world::enchantment::effects::{EnchantmentEffects, EnchantmentValueEffect};
use mcrs_minecraft_world::enchantment::value::LevelBasedValue;

fn enchantment_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("assets/minecraft/enchantment")
}

fn effects_of(file: &str) -> EnchantmentEffects {
    let bytes = std::fs::read(enchantment_dir().join(file)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    serde_json::from_value(value["effects"].clone()).unwrap()
}

/// Vanilla's codecs read these numbers as `Codec.FLOAT`, so the typed fields are
/// `f32` and re-encoding widens them back. Comparing at `f32` is comparing the
/// value the codec carries rather than the text `serde_json` chose for it.
fn as_f32(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(n) => match n.as_f64() {
            Some(f) if !n.is_i64() && !n.is_u64() => {
                serde_json::json!(f as f32 as f64)
            }
            _ => value.clone(),
        },
        serde_json::Value::Array(items) => items.iter().map(as_f32).collect(),
        serde_json::Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| (key.clone(), as_f32(value)))
            .collect::<serde_json::Map<_, _>>()
            .into(),
        _ => value.clone(),
    }
}

/// Every effect key every shipped enchantment states parses into a typed field
/// and re-encodes to the same JSON. The codec is symmetric, so the registry
/// snapshot the client receives is what the pack shipped.
#[test]
fn every_shipped_enchantment_effect_round_trips() {
    let mut checked = 0;
    let mut keys = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(enchantment_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let file: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let Some(effects) = file.get("effects") else {
            continue;
        };
        for key in effects.as_object().unwrap().keys() {
            keys.insert(key.clone());
        }
        let parsed: EnchantmentEffects = serde_json::from_value(effects.clone())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let encoded = serde_json::to_value(&parsed).unwrap();
        assert_eq!(
            as_f32(&encoded),
            as_f32(effects),
            "{} does not round-trip",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, 42, "shipped enchantments carrying effects");
    assert_eq!(keys.len(), 30, "distinct effect keys: {keys:?}");
}

#[test]
fn unknown_effect_key_is_an_error_naming_it() {
    let err =
        serde_json::from_str::<EnchantmentEffects>(r#"{"minecraft:nonsense": []}"#).unwrap_err();
    assert!(err.to_string().contains("`minecraft:nonsense`"), "{err}");
}

#[test]
fn silk_touch_sets_block_experience_to_zero() {
    let effects = effects_of("silk_touch.json");
    let block_experience = effects.block_experience.expect("silk touch sets it");
    assert_eq!(block_experience.len(), 1);
    assert!(block_experience[0].requirements.is_none());
    assert_eq!(
        block_experience[0].effect,
        EnchantmentValueEffect::Set {
            value: LevelBasedValue::Constant(0.0)
        }
    );
}

#[test]
fn efficiency_attribute_amount_is_levels_squared() {
    let effects = effects_of("efficiency.json");
    let attributes = effects.attributes.expect("efficiency has attributes");
    assert_eq!(attributes[0].attribute, "minecraft:mining_efficiency");
    assert_eq!(attributes[0].amount.calculate(3), 10.0);
}

#[test]
fn quick_charge_crossbow_charge_time_scales_linearly() {
    let effects = effects_of("quick_charge.json");
    let charge_time = effects.crossbow_charge_time.expect("quick charge sets it");
    let mut binomial = |value: f32, _chance: f32| value;
    assert_eq!(charge_time.process(1, 1.0, &mut binomial), 0.75);
    assert_eq!(charge_time.process(3, 1.0, &mut binomial), 0.25);
}
