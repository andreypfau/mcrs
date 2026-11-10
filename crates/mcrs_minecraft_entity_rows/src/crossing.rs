use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

/// A row that the tile moves took from the tile `from` to the tile `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    pub id: EntityId,
    pub from: TileKey,
    pub to: TileKey,
}
