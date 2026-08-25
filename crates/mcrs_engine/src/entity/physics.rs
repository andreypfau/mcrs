use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::Component;
use bevy_math::*;

/// Minecraft yaw and pitch in degrees, with the invariants `Entity.turn`
/// enforces: pitch is clamped to `[-90, 90]`. Yaw is additionally wrapped to
/// `[-180, 180)`, which `Entity.turn` leaves to the wire encoder.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotation {
    yaw: f32,
    pitch: f32,
}

impl Rotation {
    pub const ZERO: Self = Self {
        yaw: 0.0,
        pitch: 0.0,
    };

    #[inline]
    pub fn new(yaw: f32, pitch: f32) -> Self {
        Self {
            yaw: wrap_degrees(yaw),
            pitch: pitch.clamp(-90.0, 90.0),
        }
    }

    #[inline]
    pub const fn yaw(self) -> f32 {
        self.yaw
    }

    #[inline]
    pub const fn pitch(self) -> f32 {
        self.pitch
    }

    /// `Entity.turn`, without the caller-side sensitivity scale.
    #[inline]
    #[must_use]
    pub fn turn(self, yaw: f32, pitch: f32) -> Self {
        Self::new(self.yaw + yaw, self.pitch + pitch)
    }
}

/// `Mth.wrapDegrees`, which leaves an already-in-range angle bit-identical.
fn wrap_degrees(degrees: f32) -> f32 {
    let wrapped = degrees % 360.0;
    if wrapped >= 180.0 {
        wrapped - 360.0
    } else if wrapped < -180.0 {
        wrapped + 360.0
    } else {
        wrapped
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct Transform {
    pub translation: DVec3,
    pub rotation: Rotation,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: Rotation::ZERO,
    };

    #[inline]
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self {
            translation: DVec3::new(x, y, z),
            rotation: Rotation::ZERO,
        }
    }

    #[inline]
    pub fn from_translation(translation: DVec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    #[inline]
    #[must_use]
    pub const fn with_translation(mut self, translation: DVec3) -> Self {
        self.translation = translation;
        self
    }

    #[inline]
    #[must_use]
    pub const fn with_rotation(mut self, rotation: Rotation) -> Self {
        self.rotation = rotation;
        self
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Copy, Clone, Debug, Component, Deref)]
pub struct OldTransform(pub Transform);

#[derive(Copy, Clone, Debug, Deref, DerefMut, Component)]
pub struct Velocity(pub DVec3);

#[derive(Copy, Clone, Debug, Deref, DerefMut, Component)]
pub struct OldVelocity(pub Velocity);

#[cfg(test)]
mod tests {
    use super::Rotation;

    #[test]
    fn pitch_is_clamped_to_the_vanilla_range() {
        assert_eq!(Rotation::new(0.0, 120.0).pitch(), 90.0);
        assert_eq!(Rotation::new(0.0, -120.0).pitch(), -90.0);
        assert_eq!(Rotation::new(0.0, 8.099984).pitch(), 8.099984);
        assert_eq!(
            Rotation::ZERO.turn(0.0, 500.0).turn(0.0, 500.0).pitch(),
            90.0
        );
    }

    #[test]
    fn yaw_wraps_past_half_a_turn() {
        assert_eq!(Rotation::new(190.0, 0.0).yaw(), -170.0);
        assert_eq!(Rotation::new(-190.0, 0.0).yaw(), 170.0);
        assert!((Rotation::new(720.0 - 139.9493, 0.0).yaw() + 139.9493).abs() < 1e-3);
        assert_eq!(Rotation::new(-139.9493, 0.0).yaw(), -139.9493);
        assert_eq!(Rotation::ZERO.turn(-200.0, 0.0).yaw(), 160.0);
    }
}
