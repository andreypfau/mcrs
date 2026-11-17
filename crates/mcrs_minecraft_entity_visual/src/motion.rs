use bevy_ecs::component::Component;
use glam::DVec3;

use crate::interpolation::{Interpolation, Start};
use crate::path::PacketPath;
use crate::placement::{Placement, Rotation};

#[derive(Component, Clone, Debug, PartialEq)]
pub struct Motion {
    pub current: Placement,
    pub previous: Placement,
    pub delta_base: DVec3,
    pub on_ground: bool,
    pub in_water: bool,
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
            in_water: false,
            interpolation: Interpolation::default(),
        }
    }

    pub fn snap(&mut self, placement: Placement) {
        self.current = placement;
        self.previous = placement;
    }

    pub fn start(&mut self, path: Option<PacketPath>, rotation: Option<Rotation>, length: u32) {
        if let Start::Snap(placement) =
            self.interpolation
                .start(self.current, path, rotation, length)
        {
            self.snap(placement);
        }
    }

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

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Age(pub u32);
