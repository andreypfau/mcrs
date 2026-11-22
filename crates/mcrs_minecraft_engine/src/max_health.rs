use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

#[must_use]
pub fn max_health(attributes: Option<&AttributeInstances>) -> f32 {
    let max_health = attributes.map_or_else(
        || {
            mcrs_minecraft_entity::attribute_bases(EntityType::Player)
                .and_then(|bases| bases.base(Attribute::MaxHealth))
        },
        |instances| instances.value(Attribute::MaxHealth),
    );
    (max_health.unwrap_or_default() as f32).max(0.0)
}
