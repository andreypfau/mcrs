use bevy_ecs::component::Component;

use crate::body_turn::BodyTurn;
use crate::head_turn::HeadTurn;
use crate::walk::Walk;

/// The ticks of red tint after a hurt.
pub const HURT_TICKS: u32 = 10;

/// The state that only a living client entity has: its head and body, its walk, and its hurt
/// and death timers.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Living {
    pub head_yaw: f32,
    pub previous_head_yaw: f32,
    pub body_yaw: f32,
    pub previous_body_yaw: f32,
    pub head_turn: HeadTurn,
    pub body_turn: BodyTurn,
    pub walk: Walk,
    /// The ticks of red tint that remain.
    pub hurt_time: u32,
    /// The ticks since the entity began to die; 0 while it is alive.
    pub death_time: u32,
}

impl Living {
    /// A living entity that the server adds with the head yaw `head_yaw`: its body faces the
    /// same way.
    #[must_use]
    pub fn new(head_yaw: f32) -> Self {
        Self {
            head_yaw,
            previous_head_yaw: head_yaw,
            body_yaw: head_yaw,
            previous_body_yaw: head_yaw,
            ..Self::default()
        }
    }

    pub fn hurt(&mut self) {
        self.hurt_time = HURT_TICKS;
    }
}
