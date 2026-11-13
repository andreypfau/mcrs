use bevy_ecs::component::Component;
use glam::{DVec3, Vec3};

/// World position of an entity for combat and drops.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct CombatPosition(pub DVec3);

impl CombatPosition {
    #[must_use]
    pub const fn new(pos: DVec3) -> Self {
        Self(pos)
    }

    #[must_use]
    pub const fn from_coords(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }

    #[must_use]
    pub fn to_vec3(self) -> Vec3 {
        self.0.as_vec3()
    }
}
