//! The move of a player from one dimension world to another.

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol::{PositionFlags, VarInt};

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
}

impl Arrival {
    /// An arrival that tells the client the absolute position and rotation of `transform`.
    pub fn absolute(transform: Transform) -> Self {
        Self {
            transform,
            position: transform.translation,
            look: transform.rotation.into(),
            flags: PositionFlags::default(),
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

/// Why a [`DimensionChanger`] did not move a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DimensionChangeError {
    /// The new world holds no dimension.
    #[error("the new world holds no dimension")]
    NoDimension,
    /// The old world holds no such player.
    #[error("the old world holds no such player")]
    NoPlayer,
    /// The player did not leave its dimension.
    #[error("the player did not leave its dimension")]
    NotLeft,
    /// The session of the player is no longer placed in the old dimension.
    #[error("the session of the player is no longer placed in the old dimension")]
    NotPlaced,
    /// The player lacks the state its arrival sends.
    #[error("the player lacks the state its arrival sends")]
    NoArrivalState,
}

/// How a dimension world moves one of its players, an entity of the first dimension world, into
/// the second as the same player, with the host world in reach. Gives the entity of the player
/// in the new world.
///
/// The embedder must insert one in each dimension world, from the plugin that owns the players
/// there. A world without one keeps its players: a move to another dimension, such as a
/// teleport there, fails and the player stays where it is.
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
