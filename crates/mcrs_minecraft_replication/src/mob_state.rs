use glam::DVec3;

/// The state of a mob that its viewers see, at the end of a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MobState {
    pub position: DVec3,
    pub velocity: DVec3,
    /// The yaw of the body in degrees.
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub on_ground: bool,
    pub health: f32,
    /// Whether the mob is in the dying state, whose pose is `dying`.
    pub dying: bool,
    /// Whether the mob is drawn on fire.
    pub on_fire: bool,
}
