use glam::DVec3;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine::player_tiles::TILE_WIDTH;

/// The tile of a point. A tile spans the height of the world, so `y` is always 0, as for the
/// tiles of the players.
#[must_use]
pub fn tile_at(position: DVec3) -> TileKey {
    let width = f64::from(TILE_WIDTH.get());
    TileKey::new(
        (position.x / width).floor() as i32,
        0,
        (position.z / width).floor() as i32,
    )
}
