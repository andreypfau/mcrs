use bevy_ecs::component::Component;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::{AttributeBases, AttributeSet};

/// The attribute instances of a player, with the bases of the type `player`. Each player has it
/// from its spawn; only [`AttributeEdit`](crate::AttributeEdit) changes it.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Attributes {
    pub(crate) set: AttributeSet,
}

impl Attributes {
    /// The value of ATR-VAL-001, or `None` when the attribute is not in the set of a player.
    #[must_use]
    pub fn value(&self, attribute: Attribute) -> Option<f64> {
        self.set.value(player_bases()?, attribute)
    }

    /// The instances that differ from those of a new player.
    #[must_use]
    pub fn set(&self) -> &AttributeSet {
        &self.set
    }
}

pub(crate) const fn player_bases() -> Option<AttributeBases> {
    AttributeBases::of(EntityType::Player)
}
