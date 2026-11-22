use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

#[must_use]
pub fn mob_vitals(kind: &EntityTypeData) -> Vitals {
    let max_health = kind
        .attribute_bases()
        .and_then(|bases| bases.base(Attribute::MaxHealth))
        .unwrap_or_default();
    Vitals::new(max_health as f32)
}
