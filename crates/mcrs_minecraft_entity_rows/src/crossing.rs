use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    pub id: EntityId,
    pub from: TileKey,
    pub to: TileKey,
}
