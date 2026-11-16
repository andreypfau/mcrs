use mcrs_minecraft_entity_key::EntityId;

/// The entity whose attribute instance an edit changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AttributeTarget {
    /// A mob row of the world.
    Row(EntityId),
}
