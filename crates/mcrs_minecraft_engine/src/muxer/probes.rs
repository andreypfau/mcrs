//! Layer 1 of the movement checks for one connection: the probes that the writer puts
//! around the predicted changes once the order on the wire is final, and the client ticks of
//! the serverbound stream.

use std::time::Duration;

use bevy_ecs::component::Component;
use mcrs_minecraft_move_validate::MoveGuard;
use mcrs_minecraft_network::Instant;

/// The layer 1 state of a connection in the play state, with the origin of its times.
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
