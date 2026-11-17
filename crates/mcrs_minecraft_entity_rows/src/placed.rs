use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub tile: TileKey,
    pub id: EntityId,
}
