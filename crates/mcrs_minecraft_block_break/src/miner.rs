/// What a player brings to the rate of one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Miner {
    /// The speed of the main-hand item for the block. An empty hand has the speed 1.
    pub item_speed: f32,
    /// Whether the player can harvest the block.
    pub harvests: bool,
    /// The on-ground flag of the last movement of the player.
    pub on_ground: bool,
    /// Whether the eyes of the player are in water.
    pub eyes_in_water: bool,
}
