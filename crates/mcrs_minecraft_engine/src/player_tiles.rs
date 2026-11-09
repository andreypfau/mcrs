use std::num::NonZeroU32;

use bevy_math::DVec3;
use mcrs_engine_muxer::{TileGrid, TileKey, TileRadius};
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_entity_key::TILE_WIDTH;

/// The tiles around its own tile in which a player sees the other players: 4 tiles are about 128
/// blocks, on X and Z only, since one tile spans the height of the world. A mob has its own
/// tracking range instead.
pub const VIEW_RADIUS: TileRadius = TileRadius::new(4, 0, 4);

/// The tile that holds a point of a world, from the height of the world.
#[derive(Clone, Copy, Debug)]
pub struct PlayerTiles {
    grid: TileGrid,
    min_y: i32,
    max_y: i32,
}

impl PlayerTiles {
    #[must_use]
    pub fn new(config: &DimensionTypeConfig) -> Self {
        let height = NonZeroU32::new(config.height).unwrap_or(NonZeroU32::MIN);
        Self {
            grid: TileGrid {
                x: TILE_WIDTH,
                y: height,
                z: TILE_WIDTH,
            },
            min_y: config.min_y,
            max_y: config
                .min_y
                .saturating_add_unsigned(height.get().saturating_sub(1)),
        }
    }

    /// The tile of `position`. A point above or below the world counts as inside it, so a
    /// player that flies over the build limit stays in sight, as with vanilla tracking.
    #[must_use]
    pub fn tile_at(&self, position: DVec3) -> TileKey {
        let y = (position.y.floor() as i32).clamp(self.min_y, self.max_y);
        self.grid.tile_at(
            position.x.floor() as i32,
            y.saturating_sub(self.min_y),
            position.z.floor() as i32,
        )
    }
}
