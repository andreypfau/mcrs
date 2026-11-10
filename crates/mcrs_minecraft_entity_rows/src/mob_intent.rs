use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_physics::Steering;

/// What the AI sets for one mob before its next hot step: the steering of the travel and the
/// rotations. A field that is `None` keeps the value of the row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobIntent {
    pub target: EntityId,
    pub steering: Option<Steering>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub head_yaw: Option<f32>,
    pub body_yaw: Option<f32>,
}

impl MobIntent {
    /// An intent for `target` that changes nothing.
    #[must_use]
    pub const fn new(target: EntityId) -> Self {
        Self {
            target,
            steering: None,
            yaw: None,
            pitch: None,
            head_yaw: None,
            body_yaw: None,
        }
    }
}
