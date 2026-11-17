use std::time::Duration;

use bevy_ecs::component::Component;
use mcrs_minecraft_move_validate::MoveGuard;
use mcrs_minecraft_network::Instant;

#[derive(Component, Debug)]
pub struct Probes {
    pub guard: MoveGuard,
    origin: Instant,
}

impl Default for Probes {
    fn default() -> Self {
        Self {
            guard: MoveGuard::new(),
            origin: Instant::now(),
        }
    }
}

impl Probes {
    #[must_use]
    pub fn since_origin(&self, at: Instant) -> Duration {
        at.saturating_duration_since(self.origin)
    }
}
