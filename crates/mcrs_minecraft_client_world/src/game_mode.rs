use crate::abilities;
use crate::joined_game::JoinedGame;
use bevy::prelude::*;
use mcrs_minecraft_game_mode::{PlayerGameMode, PreviousGameMode};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityEvent, ClientboundGameEvent, ClientboundLogin, ClientboundRespawn,
};
use mcrs_minecraft_protocol_game::game_event::GameEventKind;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;

use crate::player::Player;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PermissionLevel(pub u8);

impl PermissionLevel {
    pub const GAMEMASTERS: Self = Self(2);

    const FIRST_EVENT: i8 = 24;
    const OWNERS: u8 = 4;

    pub fn from_entity_event(status: i8) -> Option<Self> {
        let level = u8::try_from(status.checked_sub(Self::FIRST_EVENT)?).ok()?;
        (level <= Self::OWNERS).then_some(Self(level))
    }
}

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_login);
        app.add_observer(on_respawn);
        app.add_observer(on_game_event);
        app.add_observer(on_entity_event);
        app.add_observer(abilities::on_login);
        app.add_observer(abilities::on_respawn);
        app.add_observer(abilities::on_game_event);
        app.add_observer(abilities::on_player_abilities);
    }
}

fn on_login(
    event: On<Received<ClientboundLogin>>,
    player: Query<Entity, With<Player>>,
    commands: Commands,
) {
    if let Some(login) = event.packet() {
        spawned(&login.player_spawn_info, player, commands);
    }
}

fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    player: Query<Entity, With<Player>>,
    commands: Commands,
) {
    if let Some(respawn) = event.packet() {
        spawned(&respawn.player_spawn_info, player, commands);
    }
}

fn spawned(info: &PlayerSpawnInfo, player: Query<Entity, With<Player>>, mut commands: Commands) {
    if let Ok(player) = player.single() {
        commands.entity(player).insert((
            PlayerGameMode(info.game_mode),
            PreviousGameMode(info.prev_game_mode.0),
            PermissionLevel::default(),
        ));
    }
}

fn on_game_event(
    event: On<Received<ClientboundGameEvent>>,
    mut player: Query<(&mut PlayerGameMode, &mut PreviousGameMode), With<Player>>,
) {
    if let Some(packet) = event.packet()
        && let GameEventKind::ChangeGameMode(changed) = packet.game_event
        && let Ok((mut mode, mut previous)) = player.single_mut()
        && mode.0 != changed
    {
        previous.set_if_neq(PreviousGameMode(Some(mode.0)));
        mode.0 = changed;
    }
}

fn on_entity_event(
    event: On<Received<ClientboundEntityEvent>>,
    connections: Query<&JoinedGame>,
    mut permission: Query<&mut PermissionLevel, With<Player>>,
) {
    if let Some(packet) = event.packet()
        && connections
            .get(event.entity)
            .is_ok_and(|joined| joined.player_id == packet.entity_id)
        && let Some(level) = PermissionLevel::from_entity_event(packet.entity_status)
        && let Ok(mut permission) = permission.single_mut()
    {
        permission.set_if_neq(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_game_mode::{GameMode, OptGameMode};
    use mcrs_minecraft_network::ConnectionState;
    use mcrs_minecraft_network::metrics::PacketDispatcher;
    use mcrs_minecraft_protocol::{Encode, Packet, VarInt};
    use mcrs_minecraft_resource::Id;

    const PLAYER_ID: i32 = 7;

    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(GameModePlugin);
        let connection = app
            .world_mut()
            .spawn((
                ConnectionState::Game,
                JoinedGame {
                    player_id: PLAYER_ID,
                    dimensions: Vec::new(),
                },
            ))
            .id();
        app.world_mut().spawn(Player);
        (app, connection)
    }

    fn receive<P: Packet + Encode>(app: &mut App, connection: Entity, packet: P) {
        let mut data = Vec::new();
        packet.encode(&mut data).unwrap();
        let trigger = PacketDispatcher::clientbound()
            .with_play(crate::game_packets::GAME_CLIENTBOUND)
            .dispatch(P::STATE, P::ID, &data)
            .unwrap()
            .unwrap();
        trigger.raise(app.world_mut().commands(), connection, data.into());
        app.world_mut().flush();
    }

    type ModeState = (
        Option<PlayerGameMode>,
        Option<PreviousGameMode>,
        Option<PermissionLevel>,
    );

    fn spawn_info(mode: GameMode, previous: Option<GameMode>) -> PlayerSpawnInfo {
        let overworld = Id::checked(0, 1).unwrap();
        PlayerSpawnInfo {
            prev_game_mode: OptGameMode(previous),
            ..PlayerSpawnInfo::new(
                overworld,
                mcrs_minecraft_dimension::dimension_keys::OVERWORLD.into(),
                mode,
            )
        }
    }

    fn login(mode: GameMode, previous: Option<GameMode>) -> ClientboundLogin {
        ClientboundLogin {
            player_id: PLAYER_ID,
            hardcore: false,
            dimensions: Vec::new(),
            max_players: VarInt(20),
            chunk_radius: VarInt(8),
            simulation_distance: VarInt(8),
            reduced_debug_info: false,
            show_death_screen: true,
            do_limited_crafting: false,
            player_spawn_info: spawn_info(mode, previous),
            online_mode: false,
            enforces_secure_chat: false,
        }
    }

    fn change(mode: GameMode) -> ClientboundGameEvent {
        ClientboundGameEvent {
            game_event: GameEventKind::ChangeGameMode(mode),
        }
    }

    fn state(app: &mut App) -> ModeState {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(
            Option<&PlayerGameMode>,
            Option<&PreviousGameMode>,
            Option<&PermissionLevel>,
        ), With<Player>>();
        let (mode, previous, level) = query.single(world).unwrap();
        (mode.copied(), previous.copied(), level.copied())
    }

    #[test]
    fn login_game_events_and_respawn_set_the_mode() {
        let (mut app, connection) = app();
        receive(&mut app, connection, change(GameMode::Creative));
        assert_eq!(state(&mut app), (None, None, None));

        receive(&mut app, connection, login(GameMode::Survival, None));
        assert_eq!(
            state(&mut app),
            (
                Some(PlayerGameMode(GameMode::Survival)),
                Some(PreviousGameMode(None)),
                Some(PermissionLevel(0))
            )
        );

        for _ in 0..2 {
            receive(&mut app, connection, change(GameMode::Creative));
            assert_eq!(
                state(&mut app),
                (
                    Some(PlayerGameMode(GameMode::Creative)),
                    Some(PreviousGameMode(Some(GameMode::Survival))),
                    Some(PermissionLevel(0))
                ),
                "a change to the same mode keeps the previous one"
            );
        }

        receive(
            &mut app,
            connection,
            ClientboundRespawn {
                player_spawn_info: spawn_info(GameMode::Adventure, Some(GameMode::Spectator)),
                data_to_keep: 0,
            },
        );
        assert_eq!(
            state(&mut app),
            (
                Some(PlayerGameMode(GameMode::Adventure)),
                Some(PreviousGameMode(Some(GameMode::Spectator))),
                Some(PermissionLevel(0))
            )
        );
    }

    #[test]
    fn only_the_local_players_entity_event_sets_the_permission() {
        let (mut app, connection) = app();
        receive(&mut app, connection, login(GameMode::Creative, None));
        receive(
            &mut app,
            connection,
            ClientboundEntityEvent {
                entity_id: PLAYER_ID + 1,
                entity_status: 28,
            },
        );
        assert_eq!(state(&mut app).2, Some(PermissionLevel(0)));

        for (status, level) in [
            (26, 2),
            (24, 0),
            (28, 4),
            (23, 4),
            (29, 4),
            (25, 1),
            (27, 3),
        ] {
            receive(
                &mut app,
                connection,
                ClientboundEntityEvent {
                    entity_id: PLAYER_ID,
                    entity_status: status,
                },
            );
            assert_eq!(
                state(&mut app).2,
                Some(PermissionLevel(level)),
                "status {status}"
            );
        }

        receive(
            &mut app,
            connection,
            ClientboundRespawn {
                player_spawn_info: spawn_info(GameMode::Creative, None),
                data_to_keep: 0,
            },
        );
        assert_eq!(state(&mut app).2, Some(PermissionLevel(0)));
    }
}
