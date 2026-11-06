//! The move of a player from one dimension world to another.

use bevy_math::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::PositionFlags;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundPlayerLookAt, ClientboundPlayerPosition,
};

/// Where a player arrives: its transform, and what `player_position` tells the client, whose
/// relative axes carry the change and not the value.
#[derive(Clone, Debug, PartialEq)]
pub struct Arrival {
    /// The transform of the player in the new place.
    pub transform: Transform,
    /// The position `player_position` sends, a change on each relative axis.
    pub position: DVec3,
    /// The rotation `player_position` sends, a change on each relative angle.
    pub look: Look,
    /// The axes and angles `player_position` sends as a change.
    pub flags: PositionFlags,
    /// What the player turns to once it is there, sent after `player_position`.
    pub look_at: Option<ClientboundPlayerLookAt>,
}

impl Arrival {
    /// An arrival that tells the client the absolute position and rotation of `transform`.
    pub fn absolute(transform: Transform) -> Self {
        Self {
            transform,
            position: transform.translation,
            look: transform.rotation.into(),
            flags: PositionFlags::default(),
            look_at: None,
        }
    }

    /// The `player_position` packet of the arrival, with `teleport_id`.
    pub fn position_packet(&self, teleport_id: i32) -> ClientboundPlayerPosition {
        ClientboundPlayerPosition {
            teleport_id: VarInt(teleport_id),
            position: self.position,
            velocity: DVec3::ZERO,
            look: self.look,
            flags: self.flags,
        }
    }
}

/// Why a player did not change dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DimensionChangeError {
    /// No other running dimension is the one the player changes to.
    #[error("no other running dimension is the one the player changes to")]
    NoDimension,
    /// The old world holds no such player.
    #[error("the old world holds no such player")]
    NoPlayer,
    /// The player lacks the state its arrival sends.
    #[error("the player lacks the state its arrival sends")]
    NoArrivalState,
}
