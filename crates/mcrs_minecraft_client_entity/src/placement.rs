use glam::DVec3;

use crate::angle;

/// A position with a yaw and a pitch, in blocks and degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Placement {
    #[must_use]
    pub const fn new(position: DVec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
        }
    }

    /// The placement a fraction `t` of the way to `to`; the yaw follows the shorter arc.
    #[must_use]
    pub fn towards(&self, to: &Placement, t: f32) -> Placement {
        Placement {
            position: self.position.lerp(to.position, f64::from(t)),
            yaw: angle::alerp(t, self.yaw, to.yaw),
            pitch: angle::lerp(t, self.pitch, to.pitch),
        }
    }

    #[must_use]
    pub fn rotation(&self) -> Rotation {
        Rotation {
            yaw: self.yaw,
            pitch: self.pitch,
        }
    }
}

/// A yaw and a pitch in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotation {
    pub yaw: f32,
    pub pitch: f32,
}
