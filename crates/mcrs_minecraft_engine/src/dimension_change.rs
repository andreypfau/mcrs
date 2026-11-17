use bevy_math::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::PositionFlags;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundPlayerLookAt, ClientboundPlayerPosition,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Arrival {
    pub transform: Transform,
    pub position: DVec3,
    pub look: Look,
    pub flags: PositionFlags,
    pub look_at: Option<ClientboundPlayerLookAt>,
}

impl Arrival {
    pub fn absolute(transform: Transform) -> Self {
        Self {
            transform,
            position: transform.translation,
            look: transform.rotation.into(),
            flags: PositionFlags::default(),
            look_at: None,
        }
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DimensionChangeError {
    #[error("no other running dimension is the one the player changes to")]
    NoDimension,
    #[error("the old world holds no such player")]
    NoPlayer,
    #[error("the player lacks the state its arrival sends")]
    NoArrivalState,
}
