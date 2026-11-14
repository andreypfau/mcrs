use bevy_ecs::component::Component;
use mcrs_minecraft_falling_block::Start;

/// A start of a fall that no world write has applied yet. It exists from the read of its
/// [`FallRequest`](crate::FallRequest) to the end of
/// [`FallingBlockSet::StartReview`](crate::FallingBlockSet::StartReview). A system in that set
/// cancels the start by despawning the entity, or changes the start in place. After the set the
/// plugin sets the leftover at the start position and makes the entity a falling block.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PendingFall(pub Start);
