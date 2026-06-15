use mcrs_protocol::BlockStateId;

pub struct BetaCaveCarverConfig {
    pub air_state: BlockStateId,
    pub lava_state: BlockStateId,
    pub stone_state: BlockStateId,
    pub dirt_state: BlockStateId,
    pub grass_state: BlockStateId,
    pub lava_level: i32,
    pub range: i32,
    pub horizontal_radius_multiplier: f32,
    pub vertical_radius_multiplier: f32,
}

impl BetaCaveCarverConfig {
    pub fn beta() -> Self {
        BetaCaveCarverConfig {
            air_state: BlockStateId(0),
            lava_state: BlockStateId(0),
            stone_state: BlockStateId(0),
            dirt_state: BlockStateId(0),
            grass_state: BlockStateId(0),
            lava_level: 10,
            range: 8,
            horizontal_radius_multiplier: 1.0,
            vertical_radius_multiplier: 1.0,
        }
    }
}
