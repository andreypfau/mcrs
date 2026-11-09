//! A player that leaves its dimension world, as a value that crosses to the host and on to the
//! world it arrives in.

use bevy_ecs::message::Message;
use bevy_ecs::query::With;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::client_info::ClientInfo;
use crate::coordinator::clock::GameTime;
use crate::dimension_change::{Arrival, DimensionChangeError};
use crate::game_mode::ListedGameMode;
use crate::game_profile::GameProfile;
use crate::op_level::AnnouncedOpLevel;
use crate::persistence::{LoadedPlayerDat, UnreadablePlayerDat};
use crate::teleport::TeleportState;

/// A player that its dimension world let go at the end of a tick, to arrive in the dimension
/// `to`. The world hands it to the host after the packets of the same tick.
#[derive(Clone, Debug)]
pub struct Departure {
    pub(crate) session: PlayerSession,
    pub(crate) to: ResourceKeyBuf<DimensionKey>,
    pub(crate) arrival: Arrival,
    pub(crate) carried: CarriedPlayer,
}

/// What the host hands a dimension world for a player that changes dimension.
#[derive(Message, Clone, Debug)]
pub enum PlayerTransfer {
    /// The player arrives in this world from another one.
    Arrive(Departure),
    /// The player left this world but did not arrive in another one, and comes back where it
    /// was.
    Return(Departure),
}

/// The components of a player that go with it to another dimension world. A component of the
/// player that is not here stays behind and is despawned.
#[derive(Clone, Debug)]
pub(crate) struct CarriedPlayer {
    pub(crate) uuid: EntityUuid,
    pub(crate) transform: Transform,
    pub(crate) game_mode: PlayerGameMode,
    pub(crate) flying: PlayerFlying,
    pub(crate) op_level: PlayerOpLevel,
    pub(crate) teleport: TeleportState,
    /// The game time at which the player left, when the host sends its arrival teleport.
    pub(crate) departed_at: GameTime,
    profile: Option<GameProfile>,
    view_distance: Option<PlayerViewDistance>,
    client_info: Option<ClientInfo>,
    announced_op_level: Option<AnnouncedOpLevel>,
    listed_game_mode: Option<ListedGameMode>,
    loaded_dat: Option<LoadedPlayerDat>,
    unreadable_dat: Option<UnreadablePlayerDat>,
}

/// The players that [`CarriedPlayer::take`] takes from.
pub(crate) type Carriable = (
    With<Player>,
    With<EntityUuid>,
    With<Transform>,
    With<PlayerGameMode>,
    With<PlayerFlying>,
    With<PlayerOpLevel>,
    With<TeleportState>,
);

impl CarriedPlayer {
    /// Takes the carried components out of `player`. Takes nothing when `player` is no player
    /// or lacks the state its arrival sends.
    pub(crate) fn take(player: &mut EntityWorldMut<'_>) -> Result<Self, DimensionChangeError> {
        let (Some(&uuid), Some(&transform), true) = (
            player.get::<EntityUuid>(),
            player.get::<Transform>(),
            player.contains::<Player>(),
        ) else {
            return Err(DimensionChangeError::NoPlayer);
        };
        let (Some(&game_mode), Some(&flying), Some(&op_level), true) = (
            player.get::<PlayerGameMode>(),
            player.get::<PlayerFlying>(),
            player.get::<PlayerOpLevel>(),
            player.contains::<TeleportState>(),
        ) else {
            return Err(DimensionChangeError::NoArrivalState);
        };
        let Some(teleport) = player.take::<TeleportState>() else {
            return Err(DimensionChangeError::NoArrivalState);
        };
        let departed_at = player
            .world()
            .get_resource::<GameTime>()
            .copied()
            .unwrap_or_default();
        Ok(Self {
            uuid,
            transform,
            game_mode,
            flying,
            op_level,
            teleport,
            departed_at,
            profile: player.take(),
            view_distance: player.take(),
            client_info: player.take(),
            announced_op_level: player.take(),
            listed_game_mode: player.take(),
            loaded_dat: player.take(),
            unreadable_dat: player.take(),
        })
    }

    /// Inserts the carried components into `player`, except its transform and its uuid, which
    /// the spawn of `player` sets.
    pub(crate) fn insert_into(self, player: &mut EntityWorldMut<'_>) {
        player.insert((self.game_mode, self.flying, self.op_level, self.teleport));
        if let Some(profile) = self.profile {
            player.insert(profile);
        }
        if let Some(view_distance) = self.view_distance {
            player.insert(view_distance);
        }
        if let Some(client_info) = self.client_info {
            player.insert(client_info);
        }
        if let Some(announced_op_level) = self.announced_op_level {
            player.insert(announced_op_level);
        }
        if let Some(listed_game_mode) = self.listed_game_mode {
            player.insert(listed_game_mode);
        }
        if let Some(loaded_dat) = self.loaded_dat {
            player.insert(loaded_dat);
        }
        if let Some(unreadable_dat) = self.unreadable_dat {
            player.insert(unreadable_dat);
        }
    }
}
