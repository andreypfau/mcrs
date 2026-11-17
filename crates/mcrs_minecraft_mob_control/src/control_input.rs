use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlInput {
    pub tick: u64,
    pub position: DVec3,
    pub rotations: Rotations,
    pub body: BodyData,
    pub flags: BodyFlags,
    pub sideways: f32,
    pub speed: f32,
    pub movement_speed: f64,
    pub feet_block_top: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotations {
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub body_yaw: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyData {
    pub width: f32,
    pub eye_height: f32,
    pub step_height: f32,
    pub head_yaw_limit: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BodyFlags(u8);

impl BodyFlags {
    pub const NONE: Self = Self(0);
    pub const ON_GROUND: Self = Self(1);
    pub const IN_LIQUID: Self = Self(1 << 1);
    pub const PASSENGER: Self = Self(1 << 2);
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
