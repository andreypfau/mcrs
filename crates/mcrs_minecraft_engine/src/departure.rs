//! A player that leaves its dimension world, as a value that crosses to the host and on to the
//! world it arrives in.

use bevy_ecs::message::Message;
use bevy_ecs::query::With;
use bevy_ecs::world::{EntityWorldMut, World};
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_resource::ResourceKeyBuf;
use tracing::error;

use crate::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::coordinator::clock::GameTime;
use crate::dimension_change::{Arrival, DimensionChangeError};
use crate::persisted_registry::{PersistedRegistry, PersistedSnapshot};
use crate::persistence::UnreadablePlayerDat;
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

/// The components of a player that go with it to another dimension world, captured through the
/// persisted-component registry. A component of the player that is not registered stays behind and
/// is despawned.
#[derive(Clone, Debug)]
pub(crate) struct CarriedPlayer {
    snapshot: PersistedSnapshot,
    /// The player file could not be read, so no later save may replace it.
    unreadable: bool,
    /// The game time at which the player left, when the host sends its arrival teleport.
    pub(crate) departed_at: GameTime,
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
    /// Takes the registered components out of `player`. Takes nothing when `player` is no player
    /// or lacks the state its arrival sends.
    pub(crate) fn take(player: &mut EntityWorldMut<'_>) -> Result<Self, DimensionChangeError> {
        if !(player.contains::<Player>()
            && player.contains::<EntityUuid>()
            && player.contains::<Transform>()
            && player.contains::<PlayerOpLevel>())
        {
            return Err(DimensionChangeError::NoPlayer);
        }
        if !(player.contains::<PlayerGameMode>()
            && player.contains::<PlayerFlying>()
            && player.contains::<TeleportState>())
        {
            return Err(DimensionChangeError::NoArrivalState);
        }
        let id = player.id();
        let unreadable = player.contains::<UnreadablePlayerDat>();
        let (snapshot, departed_at) = player.world_scope(|world| {
            let Some(registry) = world.remove_resource::<PersistedRegistry>() else {
                error!("player transfer has no persisted-component registry");
                return Err(DimensionChangeError::NoArrivalState);
            };
            let snapshot = registry.capture(world, id);
            world.insert_resource(registry);
            let snapshot = snapshot.map_err(|err| {
                error!(?id, "player transfer state not captured: {err}");
                DimensionChangeError::NoArrivalState
            })?;
            let departed_at = world
                .get_resource::<GameTime>()
                .copied()
                .unwrap_or_default();
            Ok((snapshot, departed_at))
        })?;
        Ok(Self {
            snapshot,
            unreadable,
            departed_at,
        })
    }

    pub(crate) fn transform(&self) -> Transform {
        self.snapshot.transform().unwrap_or_default()
    }

    pub(crate) fn uuid(&self) -> EntityUuid {
        EntityUuid(
            self.snapshot
                .profile()
                .map_or(Uuid::nil(), |profile| profile.id),
        )
    }

    pub(crate) fn game_mode(&self) -> GameMode {
        self.snapshot.game_mode().0
    }

    pub(crate) fn flying(&self) -> PlayerFlying {
        self.snapshot.flying()
    }

    pub(crate) fn op_level(&self) -> PlayerOpLevel {
        self.snapshot.op_level()
    }

    /// Sends the player a teleport to `destination` as of its departure and returns its id.
    pub(crate) fn send_teleport(&mut self, destination: Transform) -> i32 {
        self.snapshot.send_teleport(destination, self.departed_at)
    }

    /// Inserts the carried components into `player`, except its transform and its uuid, which
    /// the spawn of `player` sets.
    pub(crate) fn insert_into(self, world: &mut World, player: bevy_ecs::entity::Entity) {
        let mut snapshot = self.snapshot;
        snapshot.forget_placement();
        if self.unreadable {
            world.entity_mut(player).insert(UnreadablePlayerDat);
        }
        let Some(registry) = world.remove_resource::<PersistedRegistry>() else {
            error!("player transfer has no persisted-component registry");
            return;
        };
        if let Err(err) = registry.apply(world, player, &snapshot) {
            error!(?player, "player transfer state not applied: {err}");
            world.entity_mut(player).insert(UnreadablePlayerDat);
        }
        world.insert_resource(registry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_profile::GameProfile;
    use crate::persisted_registry::{PersistScope, PersistedAppExt};
    use crate::persistence::tests::{app, profile, temp_world};
    use crate::persistence::{PlayerPersistencePlugin, load_player, write_player};
    use bevy_app::App;
    use bevy_ecs::component::Component;
    use mcrs_minecraft_protocol::uuid::Uuid;
    use mcrs_minecraft_save::{player_dat_path, read_player_dat};
    use serde::{Deserialize, Serialize};

    #[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct CarriedMarker(u32);

    fn player_bundle() -> impl bevy_ecs::bundle::Bundle {
        (
            Player,
            EntityUuid(Uuid::nil()),
            Transform::default(),
            PlayerGameMode::default(),
            PlayerFlying(false),
            PlayerOpLevel(1),
            TeleportState::default(),
        )
    }

    #[test]
    fn a_registered_player_component_crosses_a_dimension() {
        let mut app = App::new();
        app.add_plugins(PlayerPersistencePlugin);
        app.register_persisted::<CarriedMarker>(&["TestCarried"], PersistScope::TravelOnly);
        app.insert_resource(GameTime(42));
        let world = app.world_mut();
        let old = world
            .spawn((player_bundle(), profile(), CarriedMarker(9)))
            .id();

        let carried = {
            let mut leaving = world.entity_mut(old);
            CarriedPlayer::take(&mut leaving).unwrap()
        };
        world.despawn(old);
        let arrived = world
            .spawn((Player, EntityUuid(Uuid::nil()), Transform::default()))
            .id();
        carried.insert_into(world, arrived);

        assert_eq!(world.get::<CarriedMarker>(arrived), Some(&CarriedMarker(9)));
        assert_eq!(world.get::<PlayerOpLevel>(arrived), Some(&PlayerOpLevel(1)));
        assert_eq!(
            world.get::<GameProfile>(arrived).map(|profile| profile.id),
            Some(profile().id)
        );
    }

    #[test]
    fn an_unreadable_player_file_survives_a_dimension_change_and_a_save() {
        let dir = temp_world();
        let path = player_dat_path(&dir, profile().id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not a player file").unwrap();
        let mut app = app(&dir);
        let world = app.world_mut();
        let old = world.spawn((player_bundle(), profile())).id();
        load_player(world, old);
        assert!(world.get::<UnreadablePlayerDat>(old).is_some());

        let carried = CarriedPlayer::take(&mut world.entity_mut(old)).unwrap();
        world.despawn(old);
        let arrived = world
            .spawn((Player, EntityUuid(Uuid::nil()), Transform::default()))
            .id();
        carried.insert_into(world, arrived);
        write_player(world, arrived);

        assert_eq!(std::fs::read(&path).unwrap(), b"not a player file");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_arrival_apply_keeps_the_player_file_from_being_overwritten() {
        let dir = temp_world();
        let path = player_dat_path(&dir, profile().id);
        let mut source = app(&dir);
        source.register_persisted::<CarriedMarker>(&["TestCarried"], PersistScope::TravelOnly);
        let world = source.world_mut();
        let old = world
            .spawn((player_bundle(), profile(), CarriedMarker(9)))
            .id();
        let carried = CarriedPlayer::take(&mut world.entity_mut(old)).unwrap();

        let mut destination = app(&dir);
        let world = destination.world_mut();
        let arrived = world
            .spawn((Player, EntityUuid(Uuid::nil()), Transform::default()))
            .id();
        carried.insert_into(world, arrived);
        assert!(world.get::<UnreadablePlayerDat>(arrived).is_some());
        write_player(world, arrived);

        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_readable_player_file_is_written_after_a_dimension_change() {
        let dir = temp_world();
        let mut app = app(&dir);
        let world = app.world_mut();
        let old = world.spawn((player_bundle(), profile())).id();
        let carried = CarriedPlayer::take(&mut world.entity_mut(old)).unwrap();
        world.despawn(old);
        let arrived = world
            .spawn((Player, EntityUuid(Uuid::nil()), Transform::default()))
            .id();
        carried.insert_into(world, arrived);
        assert!(world.get::<UnreadablePlayerDat>(arrived).is_none());
        write_player(world, arrived);

        assert!(read_player_dat(&dir, profile().id).unwrap().is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
