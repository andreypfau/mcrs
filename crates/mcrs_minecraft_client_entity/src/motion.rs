use bevy_ecs::component::Component;
use glam::DVec3;

use crate::interpolation::{Interpolation, Start};
use crate::path::PacketPath;
use crate::placement::{Placement, Rotation};

/// Where a client entity is, where it was at the start of the latest tick, the base of the
/// relative moves of the server, and the path that it still travels.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Motion {
    pub current: Placement,
    pub previous: Placement,
    /// The position that the next relative move of the server starts from.
    pub delta_base: DVec3,
    pub on_ground: bool,
    pub interpolation: Interpolation,
}

impl Motion {
    #[must_use]
    pub fn new(placement: Placement, delta_base: DVec3) -> Self {
        Self {
            current: placement,
            previous: placement,
            delta_base,
            on_ground: false,
            interpolation: Interpolation::default(),
        }
    }

    /// Puts the entity at `placement` with no movement between frames. An active path stays,
    /// and its next tick moves the rest of it with the entity.
    pub fn snap(&mut self, placement: Placement) {
        self.current = placement;
        self.previous = placement;
    }

    /// Adds the movement of one packet to the path of the entity.
    pub fn start(&mut self, path: Option<PacketPath>, rotation: Option<Rotation>, length: u32) {
        if let Start::Snap(placement) =
            self.interpolation
                .start(self.current, path, rotation, length)
        {
            self.snap(placement);
        }
    }

    /// The change of the position in the latest tick.
    #[must_use]
    pub fn movement(&self) -> DVec3 {
        self.current.position - self.previous.position
    }

    #[must_use]
    pub fn frame_position(&self, partial_tick: f32) -> DVec3 {
        self.previous
            .position
            .lerp(self.current.position, f64::from(partial_tick))
    }
}

/// The ticks that a client entity ran since the server added it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Age(pub u32);
