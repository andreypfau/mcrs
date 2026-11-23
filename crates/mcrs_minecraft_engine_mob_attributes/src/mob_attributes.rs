use bevy_ecs::component::Component;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeSet;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct MobAttributes(pub(crate) AttributeSet);

impl MobAttributes {
    #[must_use]
    pub fn value(&self, entity_type: EntityType, attribute: Attribute) -> Option<f64> {
        let bases = mcrs_minecraft_entity::attribute_bases(entity_type)?;
        self.0.value(bases, attribute)
    }

    #[must_use]
    pub fn set(&self) -> &AttributeSet {
        &self.0
    }
}
