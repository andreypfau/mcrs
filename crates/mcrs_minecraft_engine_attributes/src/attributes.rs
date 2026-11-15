use bevy_ecs::component::Component;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{AttributeBases, AttributeInstances, AttributeSet};

/// The attribute instances of a player, with the bases of the type `player`. Each player has it
/// from its spawn. In a dimension world only [`AttributeEdit`](crate::AttributeEdit) changes it;
/// a player that arrives from another world takes the set it left with.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Attributes {
    pub(crate) instances: AttributeInstances,
}

impl Attributes {
    /// The instances of `set`, as a player carries them into another world. The change sends no
    /// [`AttributeChanged`](crate::AttributeChanged).
    #[must_use]
    pub fn new(set: AttributeSet) -> Self {
        Self {
            instances: AttributeInstances::with_set(PLAYER_BASES, set),
        }
    }

    /// The value of ATR-VAL-001, or `None` when the attribute is not in the set of a player.
    #[must_use]
    pub fn value(&self, attribute: Attribute) -> Option<f64> {
        self.instances.value(attribute)
    }

    /// The instances that differ from those of a new player.
    #[must_use]
    pub fn set(&self) -> &AttributeSet {
        self.instances.set()
    }
}

impl Default for Attributes {
    fn default() -> Self {
        Self::new(AttributeSet::new())
    }
}

const PLAYER_BASES: AttributeBases =
    match mcrs_minecraft_entity::attribute_bases(EntityType::Player) {
        Some(bases) => bases,
        None => AttributeBases::new(&[], &[]),
    };

const _: () = assert!(
    mcrs_minecraft_entity::attribute_bases(EntityType::Player).is_some(),
    "a player is a living entity and has bases"
);
