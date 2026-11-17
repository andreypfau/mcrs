use bevy_ecs::component::Component;
use glam::DVec3;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct TickVelocity {
    pub current: DVec3,
    pub previous: DVec3,
}

impl TickVelocity {
    #[must_use]
    pub const fn new(velocity: DVec3) -> Self {
        Self {
            current: velocity,
            previous: velocity,
        }
    }

    pub fn advance(&mut self, velocity: DVec3) {
        self.previous = self.current;
        self.current = velocity;
    }

    #[must_use]
    pub fn frame(&self, partial_tick: f32) -> DVec3 {
        self.previous.lerp(self.current, f64::from(partial_tick))
    }
}
