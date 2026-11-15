use bevy_ecs::resource::Resource;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeSet;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_key::EntityId;
use rustc_hash::FxHashMap;

/// The attribute instances of the mob rows that differ from those of a new mob of their type.
/// A row has an entry only while an edit left one of its instances changed; the entry leaves
/// when the row leaves the world.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RowAttributes {
    pub(crate) sets: FxHashMap<EntityId, AttributeSet>,
}

impl RowAttributes {
    /// The value of ATR-VAL-001 for the row `id` of the type `entity_type`, the value of a new
    /// mob of the type when the row has no entry; `None` when the attribute is not in the set of
    /// the type.
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

    /// The instances of the row `id`; `None` when they are those of a new mob of its type.
    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<&AttributeSet> {
        self.sets.get(&id)
    }
}
