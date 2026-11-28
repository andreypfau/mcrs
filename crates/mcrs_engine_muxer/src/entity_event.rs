use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum EntityEvent<D> {
    Spawn(D),
    Move(D),
    Data(D),
    Enter { from: TileKey },
    Leave { to: TileKey },
    Despawn,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubjectEvent<D> {
    pub subject: EntityId,
    pub event: EntityEvent<D>,
}
