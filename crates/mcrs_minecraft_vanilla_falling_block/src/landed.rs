use bevy_ecs::component::Component;
use mcrs_minecraft_falling_block::Landing;
use mcrs_voxel::BlockPos;

/// The landing of a falling block in this tick, before the world writes (FBK-LND-001). It exists
/// from the decision after the motion pass to the end of
/// [`FallingBlockSet::LandingReview`](crate::FallingBlockSet::LandingReview). A system in that set
/// replaces the landing in place, or cancels it by removing the component: the falling block then
/// keeps falling and can land again in a later tick. After the set the plugin applies the landing
/// and removes the falling block, but for [`LandingOutcome::Wait`](mcrs_minecraft_falling_block::LandingOutcome::Wait).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landed {
    pub landing: Landing,
    /// The target cell (FBK-TCK-003).
    pub target: BlockPos,
}
