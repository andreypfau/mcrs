use bevy::prelude::*;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::JoinedGame;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::game_event::GameEventKind;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundEntityEvent, ClientboundGameEvent, ClientboundLogin, ClientboundRespawn,
};

use crate::player::Player;

/// The local player's game mode as the server last set it. Absent until the
/// login packet arrives.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalGameMode {
    pub current: GameMode,
    pub previous: Option<GameMode>,
}

impl LocalGameMode {
    fn spawned(info: &PlayerSpawnInfo) -> Self {
        Self {
            current: info.game_mode,
            previous: info.prev_game_mode.0,
        }
    }

    /// A change event only moves the old mode into `previous` when the mode
    /// actually differs.
    pub fn changed_to(self, mode: GameMode) -> Self {
        Self {
            current: mode,
            previous: if mode == self.current {
                self.previous
            } else {
                Some(self.current)
            },
        }
    }
}

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
        app.add_observer(receive_game_mode_packets);
    }
}

/// A login or respawn stands up a fresh local player, whose permissions start
/// empty until the server sends the level again.
fn receive_game_mode_packets(
    event: On<ReceivedPacketEvent>,
    connections: Query<(&ConnectionState, Option<&JoinedGame>)>,
    mut player: Query<
        (
            Entity,
            Option<&mut LocalGameMode>,
            Option<&mut PermissionLevel>,
        ),
        With<Player>,
    >,
    mut commands: Commands,
) {
    let Ok((ConnectionState::Game, joined)) = connections.get(event.entity) else {
        return;
    };
    let Ok((player, mode, permission)) = player.single_mut() else {
        return;
    };
    let spawn = if let Some(login) = event.decode::<ClientboundLogin>() {
        Some(LocalGameMode::spawned(&login.player_spawn_info))
    } else {
        event
            .decode::<ClientboundRespawn>()
            .map(|respawn| LocalGameMode::spawned(&respawn.player_spawn_info))
    };
    if let Some(spawned) = spawn {
        commands
            .entity(player)
            .insert((spawned, PermissionLevel::default()));
    } else if let Some(packet) = event.decode::<ClientboundGameEvent>() {
        if let GameEventKind::ChangeGameMode(changed) = packet.game_event
            && let Some(mut mode) = mode
        {
            let next = mode.changed_to(changed);
            mode.set_if_neq(next);
        }
    } else if let Some(packet) = event.decode::<ClientboundEntityEvent>()
        && joined.is_some_and(|joined| joined.player_id == packet.entity_id)
        && let Some(level) = PermissionLevel::from_entity_event(packet.entity_status)
        && let Some(mut permission) = permission
    {
        permission.set_if_neq(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::game_mode::OptGameMode;
    use mcrs_minecraft_protocol::{Encode, Packet, VarInt};

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
                    dimension: "minecraft:overworld".into(),
                },
            ))
            .id();
        app.world_mut().spawn(Player);
        (app, connection)
    }

    fn receive<P: Packet + Encode>(app: &mut App, connection: Entity, packet: P) {
        let mut data = Vec::new();
        packet.encode(&mut data).unwrap();
        app.world_mut().trigger(ReceivedPacketEvent {
            entity: connection,
            id: P::ID,
            data: data.into(),
            timestamp: mcrs_minecraft_network::Instant::now(),
        });
        app.world_mut().flush();
    }

    fn spawn_info(mode: GameMode, previous: Option<GameMode>) -> PlayerSpawnInfo<'static> {
        PlayerSpawnInfo {
            game_mode: mode,
            prev_game_mode: OptGameMode(previous),
            ..PlayerSpawnInfo::default()
        }
    }

    fn login(mode: GameMode, previous: Option<GameMode>) -> ClientboundLogin<'static> {
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

    fn state(app: &mut App) -> (Option<LocalGameMode>, Option<PermissionLevel>) {
        let world = app.world_mut();
        let mut query = world
            .query_filtered::<(Option<&LocalGameMode>, Option<&PermissionLevel>), With<Player>>();
        let (mode, level) = query.single(world).unwrap();
        (mode.copied(), level.copied())
    }

    #[test]
    fn statuses_24_to_28_are_the_five_permission_levels() {
        assert_eq!(PermissionLevel::from_entity_event(23), None);
        for level in 0..=4u8 {
            assert_eq!(
                PermissionLevel::from_entity_event(24 + level as i8),
                Some(PermissionLevel(level))
            );
        }
        assert_eq!(PermissionLevel::from_entity_event(29), None);
        assert_eq!(PermissionLevel::from_entity_event(-100), None);
    }

    #[test]
    fn a_change_to_the_same_mode_keeps_the_previous_one() {
        let mode = LocalGameMode {
            current: GameMode::Creative,
            previous: Some(GameMode::Adventure),
        };
        assert_eq!(mode.changed_to(GameMode::Creative), mode);
        assert_eq!(
            mode.changed_to(GameMode::Spectator),
            LocalGameMode {
                current: GameMode::Spectator,
                previous: Some(GameMode::Creative),
            }
        );
    }

    #[test]
    fn login_game_events_and_respawn_set_the_mode() {
        let (mut app, connection) = app();
        receive(&mut app, connection, change(GameMode::Creative));
        assert_eq!(state(&mut app), (None, None));

        receive(&mut app, connection, login(GameMode::Survival, None));
        assert_eq!(
            state(&mut app),
            (
                Some(LocalGameMode {
                    current: GameMode::Survival,
                    previous: None
                }),
                Some(PermissionLevel(0))
            )
        );

        receive(&mut app, connection, change(GameMode::Creative));
        assert_eq!(
            state(&mut app).0,
            Some(LocalGameMode {
                current: GameMode::Creative,
                previous: Some(GameMode::Survival)
            })
        );

        receive(
            &mut app,
            connection,
            ClientboundRespawn {
                player_spawn_info: spawn_info(GameMode::Adventure, Some(GameMode::Spectator)),
                data_to_keep: 0,
            },
        );
        assert_eq!(
            state(&mut app).0,
            Some(LocalGameMode {
                current: GameMode::Adventure,
                previous: Some(GameMode::Spectator)
            })
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
        assert_eq!(state(&mut app).1, Some(PermissionLevel(0)));

        receive(
            &mut app,
            connection,
            ClientboundEntityEvent {
                entity_id: PLAYER_ID,
                entity_status: 26,
            },
        );
        assert_eq!(state(&mut app).1, Some(PermissionLevel::GAMEMASTERS));

        receive(
            &mut app,
            connection,
            ClientboundRespawn {
                player_spawn_info: spawn_info(GameMode::Creative, None),
                data_to_keep: 0,
            },
        );
        assert_eq!(state(&mut app).1, Some(PermissionLevel(0)));
    }
}
