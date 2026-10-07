#[cfg(feature = "bevy")]
use bevy_ecs::prelude::Component;

const SPRINT_SPEED_MULTIPLIER: f64 = 2.0;

#[derive(Default, Debug, Clone, Copy)]
#[cfg_attr(feature = "bevy", derive(Component), component(storage = "SparseSet"))]
pub struct InstantBuild;

/// Membership rather than a flag: `Abilities.flying` flips a handful of times a
/// session, so the archetype split costs less than a boolean that every
/// consumer of every entity has to read past.
#[derive(Default, Debug, Clone, Copy)]
#[cfg_attr(feature = "bevy", derive(Component), component(storage = "SparseSet"))]
pub struct Flying;

/// `Abilities.flyingSpeed`, at its `Abilities.java` default.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "bevy", derive(Component))]
pub struct FlyingSpeed(pub f64);

impl Default for FlyingSpeed {
    fn default() -> Self {
        Self(0.05)
    }
}

impl FlyingSpeed {
    /// `Player.getFlyingSpeed`.
    pub fn with_sprint(self, sprinting: bool) -> f64 {
        if sprinting {
            self.0 * SPRINT_SPEED_MULTIPLIER
        } else {
            self.0
        }
    }
}
