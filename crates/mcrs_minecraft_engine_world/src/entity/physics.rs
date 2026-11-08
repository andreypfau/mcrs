use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::Component;
use bevy_math::*;
use mcrs_minecraft_core::Look;

/// Yaw and pitch in degrees. Pitch is clamped to `[-90, 90]`. Yaw keeps the raw value the client
/// sent: the client turns its input with the sine of the raw yaw in f32, and a wrapped yaw gives
/// another rounding, so the server copy of a player would drift from the client.
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
            yaw,
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

    /// Adds the turn of the mouse, already scaled, to the yaw and the pitch.
    #[inline]
    #[must_use]
    pub fn turn(self, yaw: f32, pitch: f32) -> Self {
        Self::new(self.yaw + yaw, self.pitch + pitch)
    }
}

impl From<Rotation> for Look {
    fn from(rotation: Rotation) -> Self {
        Self {
            yaw: rotation.yaw,
            pitch: rotation.pitch,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_is_clamped_to_a_quarter_turn_each_way() {
        assert_eq!(Rotation::new(0.0, 120.0).pitch(), 90.0);
        assert_eq!(Rotation::new(0.0, -120.0).pitch(), -90.0);
        assert_eq!(Rotation::new(0.0, 8.099984).pitch(), 8.099984);
        assert_eq!(
            Rotation::ZERO.turn(0.0, 500.0).turn(0.0, 500.0).pitch(),
            90.0
        );
    }

    #[test]
    fn a_yaw_of_720_survives_a_round_trip() {
        let rotation = Rotation::new(720.0, 0.0).turn(-0.5, 0.0);
        assert_eq!(rotation.yaw(), 719.5, "PHY-NUM-005: the yaw is raw");
        let look: Look = rotation.into();
        assert_eq!(Rotation::new(look.yaw, look.pitch), rotation);
    }
}
