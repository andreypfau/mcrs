use bevy_ecs::component::Component;
use mcrs_minecraft_falling_block::Landing;
use mcrs_voxel::BlockPos;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landed {
    pub landing: Landing,
    pub target: BlockPos,
}
