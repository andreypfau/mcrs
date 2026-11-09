use glam::DVec3;
use mcrs_minecraft_physics::EntityState;

/// The state of a living row that its viewers see.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VisibleState {
    pub position: DVec3,
    pub velocity: DVec3,
    /// The yaw of the body in degrees.
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub on_ground: bool,
    pub health: f32,
    /// Whether the entity is in the dying state.
    pub dying: bool,
}

impl From<&EntityState> for VisibleState {
    fn from(state: &EntityState) -> Self {
        let living = state.living.unwrap_or_default();
        Self {
            position: state.body.position,
            velocity: state.body.velocity,
            yaw: living.yaw,
            pitch: living.pitch,
            head_yaw: living.head_yaw,
            on_ground: state.body.flags.on_ground,
            health: living.health,
            dying: !living.is_alive(),
        }
    }
}
