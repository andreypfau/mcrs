use mcrs_minecraft_entity::AttributeBases;
use mcrs_minecraft_entity::AttributeError;
use mcrs_minecraft_entity::AttributeSet;
use mcrs_minecraft_entity::InstanceModifier;
use mcrs_minecraft_entity::ModifierKind;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Deserialize)]
struct Fixture {
    value_cases: Vec<ValueCase>,
}

#[derive(Deserialize)]
struct ValueCase(String, String, String, Vec<Step>, String);

/// `["add", id, amount bits, operation id]` or `["remove", id]`.
#[derive(Deserialize)]
#[serde(untagged)]
enum Step {
    Add(IgnoredAny, String, String, u8),
    Remove(IgnoredAny, String),
}

#[test]
fn each_value_has_the_bits_of_the_game() {
    let bits = |hex: &str| f64::from_bits(u64::from_str_radix(hex, 16).unwrap());
    let text = fixture("attributes").text();
    let fixture: Fixture = serde_json::from_str(&text).unwrap();
    assert_eq!(fixture.value_cases.len(), 68, "the fixture holds 68 values");
    for ValueCase(name, attribute, base, steps, expected) in &fixture.value_cases {
        let attribute = Attribute::read(attribute).unwrap();
        let bases = EntityType::ALL
            .iter()
            .filter_map(|entity_type| AttributeBases::of(*entity_type))
            .find(|bases| bases.base(attribute).is_some())
            .unwrap();
        let mut set = AttributeSet::new();
        let _ = set.set_base(bases, attribute, bits(base)).unwrap();
        for step in steps {
            match step {
                Step::Add(_, id, amount, operation) => {
                    let modifier = InstanceModifier::new(
                        ResourceLocationBuf::read(id).unwrap(),
                        bits(amount),
                        ModifierOperation::try_from(*operation).unwrap(),
                        ModifierKind::Permanent,
                    );
                    match set.add_modifier(bases, attribute, modifier) {
                        Ok(()) | Err(AttributeError::ModifierPresent) => {}
                        Err(error) => panic!("{name}: {error}"),
                    }
                }
                Step::Remove(_, id) => {
                    let id = ResourceLocation::read(id).unwrap();
                    let _ = set.remove_modifier(bases, attribute, id).unwrap();
                }
            }
        }
        let value = set.value(bases, attribute).unwrap();
        assert_eq!(
            format!("{:016x}", value.to_bits()),
            *expected,
            "{name}: {value}"
        );
    }
}
