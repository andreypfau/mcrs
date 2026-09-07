use mcrs_voxel_storage::VoxelId;

pub struct BetaCaveCarverConfig {
    pub air_state: VoxelId,
    pub lava_state: VoxelId,
    pub stone_state: VoxelId,
    pub dirt_state: VoxelId,
    pub grass_state: VoxelId,
    pub lava_level: i32,
    /// How far, in chunks, the source loop reaches around the target chunk.
    /// Distinct from [`Self::tunnel_length`]: the reference hardcodes this at 8
    /// in its source loop while its `getRange()` of 4 feeds only the length.
    pub source_radius: i32,
    /// Steps a tunnel walks before the length draw shortens it. Beta's
    /// `range * 16 - 16` at range 8 and the reference's
    /// `(getRange() * 2 - 1) * 16` at range 4 are both this number.
    pub tunnel_length: i32,
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
            lava_level: 10,
            source_radius: 8,
            tunnel_length: 112,
            horizontal_radius_multiplier: 1.0,
            vertical_radius_multiplier: 1.0,
        }
    }
}
