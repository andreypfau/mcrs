use glam::DVec3;
use mcrs_minecraft_entity_key::TileKey;

/// The tile of a point, as [`TileKey::at`].
#[must_use]
pub fn tile_at(position: DVec3) -> TileKey {
    TileKey::at(position)
}
