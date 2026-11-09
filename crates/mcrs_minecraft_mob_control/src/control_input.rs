use glam::DVec3;

/// The state of the walker row that the controls read at the start of a tick, before its step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlInput {
    pub tick: u64,
    pub position: DVec3,
    pub rotations: Rotations,
    pub body: BodyData,
    pub flags: BodyFlags,
    /// The sideways input of the row before the decay of this tick.
    pub sideways: f32,
    /// The mob speed of the row (`Living::speed`).
    pub speed: f32,
    /// The value of the attribute `movement_speed`.
    pub movement_speed: f64,
    /// The top of the collision shape of the block at the block position of the mob, for the
    /// mob; `None` when the shape is empty or the block is in the tag `doors` or `fences`.
    pub feet_block_top: Option<f64>,
}

/// The four rotations of a mob in degrees: raw f32 values, never wrapped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotations {
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub body_yaw: f32,
}

/// The size of the mob and the limits of its type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyData {
    pub width: f32,
    pub eye_height: f32,
    pub step_height: f32,
    /// The largest difference of the head yaw from the body yaw; 75 for the animals.
    pub head_yaw_limit: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BodyFlags(u8);

impl BodyFlags {
    pub const NONE: Self = Self(0);
    pub const ON_GROUND: Self = Self(1);
    /// In a liquid that affects the mob.
    pub const IN_LIQUID: Self = Self(1 << 1);
    /// A passenger rides the mob; the follower updates its path also in the air.
    pub const PASSENGER: Self = Self(1 << 2);
    /// The first passenger of the mob is a mob, so the body rotation does not turn the body.
    pub const MOB_PASSENGER: Self = Self(1 << 3);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
