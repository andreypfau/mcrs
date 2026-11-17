use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_fixtures::fixture;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    default_attributes: Vec<Vec<Cell>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Cell {
    Type(String),
    Base(String, String),
}

#[test]
fn each_living_type_has_the_set_and_the_bases_of_the_game() {
    let text = fixture("attributes").text();
    let fixture: Fixture = serde_json::from_str(&text).unwrap();
    assert_eq!(fixture.default_attributes.len(), 94, "94 living types");
    let mut living: Vec<EntityType> = Vec::new();
    for row in &fixture.default_attributes {
        let [Cell::Type(id), pairs @ ..] = row.as_slice() else {
            panic!("a row starts with the id of its type");
        };
        let entity_type = EntityType::read(id).unwrap();
        let bases = mcrs_minecraft_entity::attribute_bases(entity_type)
            .unwrap_or_else(|| panic!("{id} has no bases"));
        let expected: Vec<(Attribute, String)> = pairs
            .iter()
            .map(|cell| match cell {
                Cell::Base(attribute, bits) => (Attribute::read(attribute).unwrap(), bits.clone()),
                Cell::Type(text) => panic!("{id}: {text} is not a pair"),
            })
            .collect();
        let actual: Vec<(Attribute, String)> = Attribute::ALL
            .iter()
            .filter_map(|attribute| {
                let base = bases.base(*attribute)?;
                Some((*attribute, format!("{:016x}", base.to_bits())))
            })
            .collect();
        assert_eq!(actual, expected, "{id}");
        let set: Vec<Attribute> = bases.attributes().collect();
        let expected_set: Vec<Attribute> =
            expected.iter().map(|(attribute, _)| *attribute).collect();
        assert_eq!(set, expected_set, "{id} set");
        living.push(entity_type);
    }
    for entity_type in EntityType::ALL {
        assert_eq!(
            mcrs_minecraft_entity::attribute_bases(*entity_type).is_some(),
            living.contains(entity_type),
            "{entity_type:?} is living exactly when the fixture lists it"
        );
    }
}
