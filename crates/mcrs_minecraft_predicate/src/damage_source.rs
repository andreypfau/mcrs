use mcrs_minecraft_entity::damage_type::DamageType;
use mcrs_minecraft_resource::Id;

/// The damage source that a damage source predicate tests. `E` is the entity handle of the
/// [`crate::EntityView`] of the match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageSource<E> {
    /// The id in the damage type registry of the registry set of the bake.
    pub damage_type: Id<DamageType>,
    /// The entity that the damage came from, such as an arrow; `None` for a source without one.
    pub direct_entity: Option<E>,
    /// The entity that caused the damage, such as the skeleton that shot the arrow.
    pub causing_entity: Option<E>,
}

impl<E: PartialEq> DamageSource<E> {
    /// `true` when the direct entity is the causing entity, also when the source has neither.
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.direct_entity == self.causing_entity
    }
}
