use bevy_ecs::component::Component;
use glam::DVec3;
use mcrs_minecraft_physics::MoveFlags;
use mcrs_minecraft_physics::TickOutcome;

/// What the last step of an entity did: the pass writes it in each tick that it steps the entity,
/// and marks it changed only when it differs from the step before.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct StepOutcome {
    /// The position at the start of the step.
    pub start: DVec3,
    /// The collision flags at the end of the step.
    pub flags: MoveFlags,
    /// The outcome of the kernel, such as the landing of a falling block.
    pub kernel: TickOutcome,
}
