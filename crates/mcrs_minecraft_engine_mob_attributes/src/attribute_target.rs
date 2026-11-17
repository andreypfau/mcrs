use mcrs_minecraft_entity_key::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AttributeTarget {
    Row(EntityId),
}
