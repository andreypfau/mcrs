use std::num::NonZeroU32;

/// The limits of a [`Muxer`](crate::Muxer). All rates count the ticks of the tile deltas; a
/// second is 20 ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MuxerSettings {
    /// The width of a tile on X and Z, in the units of the game. The distance of a tile to a
    /// viewer is the distance on X and Z from the viewer to the nearest point of the tile.
    pub tile_width: NonZeroU32,
    /// The maximum number of subjects in the visible set of a viewer, K. The cap admits a tile
    /// while the sum is K or less, and removes it when the sum is more than 1.25 × K.
    pub cap: u32,
    /// The bytes per second that a viewer receives on average before the muxer lowers its
    /// tiers.
    pub budget: u32,
}

impl Default for MuxerSettings {
    fn default() -> Self {
        Self {
            tile_width: NonZeroU32::new(32).unwrap_or(NonZeroU32::MIN),
            cap: 200,
            budget: 32 * 1024,
        }
    }
}
