use glam::DVec3;
use mcrs_minecraft_entity_key::EntityUuid;

use crate::Body;
use crate::MotionAttributes;
use crate::MotionEffects;
use crate::QueryEntity;
use crate::SprintSpeed;
use mcrs_minecraft_pose::Pose;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerState {
    pub key: EntityUuid,
    pub body: Body,
    pub pose: Pose,
    pub jump_delay: u8,
    pub sprint_timer: u8,
    pub jump_timer: u8,
    pub shift0: bool,
    pub jump0: bool,
    pub forward0: f32,
    pub sprint0: bool,
    pub flight0: bool,
    pub crouching: bool,
    pub swimming: bool,
    pub item_use: ItemUse,
    pub attributes: MotionAttributes,
    pub sprint_speed: SprintSpeed,
    pub effects: MotionEffects,
    pub may_fly: bool,
    pub flight_speed: f32,
    pub in_powder_snow0: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemUse {
    pub using: bool,
    pub speed_multiplier: f32,
    pub permits_sprint: bool,
}

impl Default for ItemUse {
    fn default() -> Self {
        Self {
            using: false,
            speed_multiplier: 0.2,
            permits_sprint: false,
        }
    }
}

impl ItemUse {
    #[must_use]
    pub const fn slows(&self) -> bool {
        self.using && !self.permits_sprint
    }
}

impl PlayerState {
    #[must_use]
    pub fn new(position: DVec3) -> Self {
        Self {
            body: Body {
                position,
                ..Body::default()
            },
            flight_speed: 0.05,
            ..Self::default()
        }
    }

    pub(crate) fn query_entity(&self) -> QueryEntity {
        QueryEntity {
            key: self.key,
            position: self.body.position,
            fall_distance: self.body.fall_distance,
            shift: self.shift0,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        }
    }
}
