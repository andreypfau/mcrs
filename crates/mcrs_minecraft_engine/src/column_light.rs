use bevy_ecs::schedule::SystemSet;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_voxel::ColumnPos;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColdStageLandSet;

#[derive(Clone, Debug)]
pub struct ColumnLight {
    pub column: ColumnPos,
    pub sections: Vec<LitSection>,
}

#[derive(Clone, Debug)]
pub struct LitSection {
    pub y: i32,
    pub block_light: BlockLight,
    pub sky_light: SkyLight,
}
