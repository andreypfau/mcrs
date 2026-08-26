use mcrs_palette::VoxelId;

pub struct BetaCaveCarverConfig {
    pub air_state: VoxelId,
    pub lava_state: VoxelId,
    pub stone_state: VoxelId,
    pub dirt_state: VoxelId,
    pub grass_state: VoxelId,
    /// Water block state IDs for the water-abort scan.
    /// Both flowing (water) and stationary forms should be provided.
    pub water_state: VoxelId,
    pub stationary_water_state: VoxelId,
    pub lava_level: i32,
    pub range: i32,
    pub horizontal_radius_multiplier: f32,
    pub vertical_radius_multiplier: f32,
}

impl BetaCaveCarverConfig {
    pub fn beta() -> Self {
        BetaCaveCarverConfig {
            air_state: VoxelId(0),
            lava_state: VoxelId(0),
            stone_state: VoxelId(0),
            dirt_state: VoxelId(0),
            grass_state: VoxelId(0),
            water_state: VoxelId(0),
            stationary_water_state: VoxelId(0),
            lava_level: 10,
            range: 8,
            horizontal_radius_multiplier: 1.0,
            vertical_radius_multiplier: 1.0,
        }
    }
}
