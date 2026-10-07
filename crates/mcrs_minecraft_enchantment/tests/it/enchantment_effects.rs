use std::path::PathBuf;

use mcrs_minecraft_enchantment::effects::{EnchantmentEffects, EnchantmentValueEffect};
use mcrs_minecraft_entity::keys::Attribute;
use std::sync::LazyLock;

use mcrs_minecraft_item::damage_type::DamageType;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_worldgen_testing::{tagged_report, with_shipped};

/// The static registries, and the damage types an effect's damage source
/// predicate names by tag.
fn scope() -> &'static RegistrySet {
    static SET: LazyLock<RegistrySet> =
        LazyLock::new(|| with_shipped::<DamageType>(tagged_report().clone(), "damage_type"));
    &SET
}

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("assets")
}

fn enchantment_dir() -> PathBuf {
    assets().join("minecraft/enchantment")
}

fn effects_of(file: &str) -> EnchantmentEffects {
    let bytes = std::fs::read(enchantment_dir().join(file)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    scope().scope(|| serde_json::from_value(value["effects"].clone()).unwrap())
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
        let encoded = scope().scope(|| {
            let parsed: EnchantmentEffects = serde_json::from_value(effects.clone())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            serde_json::to_value(&parsed).unwrap()
        });
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
    assert!(
        err.to_string().contains(
            "Unknown registry key in ResourceKey[minecraft:root / \
             minecraft:enchantment_effect_component_type]: minecraft:nonsense"
        ),
        "{err}"
    );

    let attribute = r#"[{"effect":{"type":"minecraft:attribute","id":"minecraft:test","attribute":"minecraft:max_health","amount":1.0,"operation":"add_value"}}]"#;
    let location = format!(r#"{{"minecraft:location_changed":{attribute}}}"#);
    assert!(serde_json::from_str::<EnchantmentEffects>(&location).is_ok());
    let entity = format!(r#"{{"minecraft:tick":{attribute}}}"#);
    let err = serde_json::from_str::<EnchantmentEffects>(&entity).unwrap_err();
    assert!(
        err.to_string().contains(
            "Unknown registry key in ResourceKey[minecraft:root / \
             minecraft:enchantment_entity_effect_type]: minecraft:attribute"
        ),
        "{err}"
    );
}

#[test]
fn level_based_effects_scale_with_the_level() {
    let efficiency = effects_of("efficiency.json")
        .attributes
        .expect("efficiency has attributes");
    assert_eq!(efficiency[0].attribute, Attribute::MiningEfficiency);
    assert_eq!(efficiency[0].amount.calculate(3), 10.0);

    let charge_time = effects_of("quick_charge.json")
        .crossbow_charge_time
        .expect("quick charge sets it");
    let mut binomial = |value: f32, _chance: f32| value;
    assert_eq!(charge_time.process(1, 1.0, &mut binomial), 0.75);
    assert_eq!(charge_time.process(3, 1.0, &mut binomial), 0.25);
}

#[test]
fn an_exponential_value_effect_raises_the_base_to_the_exponent() {
    let effect: EnchantmentValueEffect = serde_json::from_str(
        r#"{"type":"minecraft:exponential","base":2.0,"exponent":{"type":"minecraft:linear","base":1.0,"per_level_above_first":1.0}}"#,
    )
    .unwrap();
    let mut binomial = |value: f32, _chance: f32| value;
    assert_eq!(effect.process(1, 3.0, &mut binomial), 6.0);
    assert_eq!(effect.process(3, 3.0, &mut binomial), 24.0);
}

#[test]
fn an_explode_effect_reads_its_block_particles() {
    let effects = r#"{"minecraft:hit_block":[{"effect":{"type":"minecraft:explode","block_interaction":"trigger","radius":3.5,"small_particle":{"type":"minecraft:gust_emitter_small"},"large_particle":{"type":"minecraft:gust_emitter_large"},"block_particles":[{"particle":{"type":"minecraft:poof"},"scaling":0.5,"weight":3},{"particle":{"type":"minecraft:smoke"},"weight":1}],"sound":"minecraft:entity.wind_charge.wind_burst"}}]}"#;
    let json: serde_json::Value = serde_json::from_str(effects).unwrap();
    let encoded = scope().scope(|| {
        let parsed: EnchantmentEffects = serde_json::from_value(json.clone()).unwrap();
        serde_json::to_value(&parsed).unwrap()
    });
    assert_eq!(as_f32(&encoded), as_f32(&json));
}

#[test]
fn a_malformed_registry_reference_is_a_load_error() {
    let explode = |sound: &str| {
        format!(
            r#"{{"minecraft:hit_block":[{{"effect":{{"type":"minecraft:explode","block_interaction":"trigger","radius":1.0,"small_particle":{{"type":"minecraft:poof"}},"large_particle":{{"type":"minecraft:poof"}},"sound":"{sound}"}}}}]}}"#
        )
    };
    let attribute = |id: &str, attribute: &str| {
        format!(
            r#"{{"minecraft:attributes":[{{"id":"{id}","attribute":"{attribute}","amount":1.0,"operation":"add_value"}}]}}"#
        )
    };
    let damage = |damage_type: &str| {
        format!(
            r#"{{"minecraft:hit_block":[{{"effect":{{"type":"minecraft:damage_entity","min_damage":1.0,"max_damage":2.0,"damage_type":"{damage_type}"}}}}]}}"#
        )
    };
    let valid = [
        explode("minecraft:entity.wind_charge.wind_burst"),
        attribute("minecraft:test", "minecraft:max_health"),
        damage("minecraft:magic"),
        r#"{"minecraft:crossbow_charging_sounds":[{"start":"minecraft:item.crossbow.quick_charge_1"}]}"#.to_owned(),
        r#"{"minecraft:trident_sound":["minecraft:item.trident.riptide_1"]}"#.to_owned(),
    ];
    let malformed = [
        explode("Not A Sound"),
        attribute("Not An Id", "minecraft:max_health"),
        attribute("minecraft:test", "minecraft:not_an_attribute"),
        damage("minecraft:not_a_damage_type"),
        r#"{"minecraft:crossbow_charging_sounds":[{"start":"Not A Sound"}]}"#.to_owned(),
        r#"{"minecraft:trident_sound":["Not A Sound"]}"#.to_owned(),
    ];
    scope().scope(|| {
        for json in valid {
            serde_json::from_str::<EnchantmentEffects>(&json)
                .unwrap_or_else(|error| panic!("{json}: {error}"));
        }
        for json in malformed {
            assert!(
                serde_json::from_str::<EnchantmentEffects>(&json).is_err(),
                "{json} loaded"
            );
        }
    });
}
