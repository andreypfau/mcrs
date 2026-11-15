use bevy_ecs::component::Component;
use glam::DVec3;
use mcrs_minecraft_physics::TickOutcome;

/// What the last step of an entity did: the pass writes it in each tick that it steps the entity,
/// and marks it changed only when the start or the kernel outcome differs from the step before.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct StepOutcome {
    /// The position at the start of the step.
    pub start: DVec3,
    /// The outcome of the kernel, such as the landing of a falling block.
    pub kernel: TickOutcome,
    /// The tick that the host gave to the last step; `None` before the first step.
    pub(crate) stepped_at: Option<u64>,
}

impl StepOutcome {
    /// True when the pass stepped the entity in `tick`, the tick that the host gives to
    /// [`MotionStep::run`](crate::MotionStep::run). After the pass, false means
    /// that the pass skipped the entity in this tick, because it holds a
    /// [`MotionHold`](crate::MotionHold) or its section is not loaded: the other fields are then of
    /// an earlier step, or the default before the first one.
    #[must_use]
    pub fn is_of(&self, tick: u64) -> bool {
        self.stepped_at == Some(tick)
    }
}
