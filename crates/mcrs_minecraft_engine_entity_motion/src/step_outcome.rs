use bevy_ecs::component::Component;
use glam::DVec3;
use mcrs_minecraft_physics::TickOutcome;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct StepOutcome {
    pub start: DVec3,
    pub kernel: TickOutcome,
    pub(crate) stepped_at: Option<u64>,
}

impl StepOutcome {
    #[must_use]
    pub fn is_of(&self, tick: u64) -> bool {
        self.stepped_at == Some(tick)
    }
}
