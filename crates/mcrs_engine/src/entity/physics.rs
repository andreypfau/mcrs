use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::Component;
use bevy_math::*;

#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct Transform {
    pub translation: DVec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    #[inline]
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self {
            translation: DVec3::new(x, y, z),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
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
    pub const fn with_rotation(mut self, rotation: Quat) -> Self {
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
