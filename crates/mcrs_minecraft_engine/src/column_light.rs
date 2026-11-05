use bevy_ecs::schedule::SystemSet;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_voxel::ColumnPos;

/// Where the light stage of cold space hands its answers to the data layer, at the start of the
/// tick, after the answers of the data layer itself.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColdStageLandSet {
    Light,
}

/// The settled light of one column from the light stage of cold space.
#[derive(Clone, Debug)]
pub struct ColumnLight {
    pub column: ColumnPos,
    pub sections: Vec<LitSection>,
}

/// The light of one section of a [`ColumnLight`].
#[derive(Clone, Debug)]
pub struct LitSection {
    pub y: i32,
    pub block_light: BlockLight,
    pub sky_light: SkyLight,
}
