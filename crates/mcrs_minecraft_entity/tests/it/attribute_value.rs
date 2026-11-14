use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::AttributeModifier;
use mcrs_minecraft_entity_attribute::Modifier;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_attribute::ValueRange;
use mcrs_minecraft_entity_attribute::fold;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_resource::ResourceLocation;
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
        let definition = Attribute::read(attribute).unwrap().definition();
        let range = ValueRange {
            min: definition.min,
            max: definition.max,
        };
        let mut sequence: Vec<AttributeModifier<'_>> = Vec::new();
        for step in steps {
            match step {
                Step::Add(_, id, amount, operation) => {
                    let id = ResourceLocation::read(id).unwrap();
                    if sequence.iter().all(|modifier| modifier.id != id) {
                        sequence.push(AttributeModifier {
                            id,
                            amount: bits(amount),
                            operation: ModifierOperation::try_from(*operation).unwrap(),
                        });
                    }
                }
                Step::Remove(_, id) => {
                    let id = ResourceLocation::read(id).unwrap();
                    sequence.retain(|modifier| modifier.id != id);
                }
            }
        }
        let value = range.clamp(fold(
            bits(base),
            sequence.iter().copied().map(Modifier::from),
        ));
        assert_eq!(
            format!("{:016x}", value.to_bits()),
            *expected,
            "{name}: {value}"
        );
    }
}
