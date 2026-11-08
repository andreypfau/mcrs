use glam::DVec3;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine::player_tiles::TILE_WIDTH;
use mcrs_minecraft_physics::floor_block;

/// The tile of a point. A tile spans the height of the world, so `y` is always 0, as for the
/// tiles of the players.
#[must_use]
pub fn tile_at(position: DVec3) -> TileKey {
    let width = f64::from(TILE_WIDTH.get());
    TileKey::new(
        floor_block(position.x / width),
        0,
        floor_block(position.z / width),
    )
}
