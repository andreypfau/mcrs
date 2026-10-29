use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol::{Look, PositionFlags, VarInt};

/// Where a player arrives: its transform, and what `player_position` tells the client, whose
/// relative axes carry the change and not the value.
#[derive(Clone, Debug, PartialEq)]
pub struct Arrival {
    pub transform: Transform,
    pub position: DVec3,
    pub look: Look,
    pub flags: PositionFlags,
}

impl Arrival {
    /// An arrival that tells the client the absolute position and rotation of `transform`.
    pub fn absolute(transform: Transform) -> Self {
        Self {
            transform,
            position: transform.translation,
            look: Look {
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            },
            flags: PositionFlags::default(),
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
    #[error("the new world holds no dimension")]
    NoDimension,
    #[error("the old world holds no such player")]
    NoPlayer,
    #[error("the player did not leave its dimension")]
    NotLeft,
    #[error("the session of the player is no longer placed in the old dimension")]
    NotPlaced,
    #[error("the player lacks the state its arrival sends")]
    NoArrivalState,
}

/// How a dimension world moves one of its players, an entity of the first dimension world, into
/// the second as the same player, with the host world in reach. Gives the entity of the player
/// in the new world. The plugin that owns the players of a dimension world inserts it there.
#[derive(Resource, Clone, Copy)]
pub struct DimensionChanger(
    pub  fn(
        &mut World,
        &mut World,
        &mut World,
        Entity,
        Arrival,
    ) -> Result<Entity, DimensionChangeError>,
);
