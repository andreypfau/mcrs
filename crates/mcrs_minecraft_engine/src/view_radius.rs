use mcrs_engine_muxer::TileRadius;

/// The tiles around its own tile in which a player sees the other players: 4 tiles are about 128
/// blocks, on X and Z only, since one tile spans the height of the world. A mob has its own
/// tracking range instead.
pub const VIEW_RADIUS: TileRadius = TileRadius::new(4, 0, 4);
