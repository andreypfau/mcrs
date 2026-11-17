use bevy_ecs::resource::Resource;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeSet;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityId;
use rustc_hash::FxHashMap;

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RowAttributes {
    pub(crate) sets: FxHashMap<EntityId, AttributeSet>,
}

impl RowAttributes {
    #[must_use]
    pub fn value(
        &self,
        id: EntityId,
        entity_type: EntityType,
        attribute: Attribute,
    ) -> Option<f64> {
        let bases = mcrs_minecraft_entity::attribute_bases(entity_type)?;
        match self.sets.get(&id) {
            Some(set) => set.value(bases, attribute),
            None => AttributeSet::new().value(bases, attribute),
        }
    }

    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<&AttributeSet> {
        self.sets.get(&id)
    }
}
