use glam::DVec3;
use mcrs_minecraft_entity_key::BirthKey;
use mcrs_minecraft_entity_key::SourceKind;
use mcrs_voxel::BlockPos;

use crate::Body;
use crate::MotionAttributes;
use crate::MotionEffects;
use crate::MoveFlags;
use crate::Pose;
use crate::QueryEntity;

/// The state of a player at the start of a tick: the values that one tick reads and writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerState {
    pub position: DVec3,
    pub velocity: DVec3,
    pub pose: Pose,
    pub flags: MoveFlags,
    pub support: Option<BlockPos>,
    /// True when the last search for the supporting block found no block.
    pub support_missed: bool,
    pub fall_distance: f64,
    pub stuck: DVec3,
    pub jump_delay: u8,
    pub sprint_timer: u8,
    pub jump_timer: u8,
    /// The shift key of the previous tick.
    pub shift0: bool,
    /// The jump key of the previous tick, a press of auto-jump included.
    pub jump0: bool,
    /// The forward value of the input of the previous tick.
    pub forward0: f32,
    pub sprint0: bool,
    pub flight0: bool,
    pub crouching: bool,
    pub swimming: bool,
    pub item_use: ItemUse,
    pub attributes: MotionAttributes,
    pub effects: MotionEffects,
    pub may_fly: bool,
    pub flight_speed: f32,
    /// True when the player was inside powder snow in the previous tick.
    pub in_powder_snow0: bool,
}

/// The item use state of a player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemUse {
    pub using: bool,
    /// The speed multiplier of the use, `0.2F` by default.
    pub speed_multiplier: f32,
    /// True when the use permits the sprint state.
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
    /// True when the use slows the player: it uses an item that does not permit the sprint state.
    #[must_use]
    pub const fn slows(&self) -> bool {
        self.using && !self.permits_sprint
    }
}

/// The raw yaw and pitch of a player in degrees, never wrapped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerRotation {
    pub yaw: f32,
    pub pitch: f32,
}

impl PlayerState {
    /// A standing player at `position` with the defaults of a new player.
    #[must_use]
    pub fn new(position: DVec3) -> Self {
        Self {
            position,
            flight_speed: 0.05,
            ..Self::default()
        }
    }

    /// The player as the entity of a query of colliders.
    pub(crate) fn query_entity(&self) -> QueryEntity {
        QueryEntity {
            key: BirthKey::new(0, SourceKind::Player, 0, 0),
            position: self.position,
            fall_distance: self.fall_distance,
            shift: self.shift0,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        }
    }

    pub(crate) fn body(&self) -> Body {
        Body {
            position: self.position,
            velocity: self.velocity,
            flags: self.flags,
            support: self.support,
            support_missed: self.support_missed,
            fall_distance: self.fall_distance,
            stuck: self.stuck,
        }
    }

    pub(crate) fn set_body(&mut self, body: &Body) {
        self.position = body.position;
        self.velocity = body.velocity;
        self.flags = body.flags;
        self.support = body.support;
        self.support_missed = body.support_missed;
        self.fall_distance = body.fall_distance;
        self.stuck = body.stuck;
    }
}
